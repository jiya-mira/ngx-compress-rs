#!/bin/sh
# Clang ASan/UBSan integration smoke. Rust code is ASan-instrumented when the
# caller selects a nightly toolchain; C NGINX and native codec builds always use
# Clang sanitizers here.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost
export UBSAN_OPTIONS="${UBSAN_OPTIONS:-halt_on_error=1:print_stacktrace=1}"
export ASAN_SYMBOLIZER_PATH="${ASAN_SYMBOLIZER_PATH:-/usr/bin/llvm-symbolizer-14}"

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/nginx-1.30.4}
SRC=/tmp/ngx-sanitizer
RUN=/tmp/ngx-sanitizer-run
WWW=/tmp/ngx-sanitizer-www
PORT=8085
DIAGNOSTIC_DIR=${DIAGNOSTIC_DIR:-/tmp/ngx-sanitizer-diagnostics}
mkdir -p "$DIAGNOSTIC_DIR"

capture_diagnostics() {
    for file in /tmp/cfg-sanitizer.log /tmp/make-sanitizer.log "$RUN/logs/error.log"; do
        if [ -f "$file" ]; then
            cp "$file" "$DIAGNOSTIC_DIR/$(basename "$file")"
        fi
    done
}
trap capture_diagnostics EXIT

export ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=1:halt_on_error=1:abort_on_error=1:log_path=$DIAGNOSTIC_DIR/asan}"
# NGINX intentionally uses memcpy(NULL, NULL, 0) for empty ngx_str values.
# Clang's nonnull-attribute check diagnoses that established upstream idiom, so
# suppress only that check while retaining all other undefined checks.
SAN_FLAGS='-O1 -g -fsanitize=address,undefined -fno-sanitize=nonnull-attribute -fno-omit-frame-pointer'
export CC=clang
export CFLAGS="$SAN_FLAGS"

rm -rf "$SRC" "$RUN" "$WWW"
cp -a "$NGINX_SRC" "$SRC"
mkdir -p "$RUN/logs" "$WWW"
: > "$WWW/body.txt"
i=0
while [ "$i" -lt 20000 ]; do
    printf 'sanitizer payload %06d abcdefghijklmnopqrstuvwxyz\n' "$i" >> "$WWW/body.txt"
    i=$((i + 1))
done
printf 'INCLUDED SANITIZER SUBREQUEST\n' > "$WWW/inc.txt"
printf 'HEAD\n<!--#include virtual="/inc.txt" -->\nTAIL\n' > "$WWW/page.shtml"

cd "$SRC"
./configure \
    --with-compat \
    --with-http_v2_module \
    --with-cc-opt="$SAN_FLAGS" \
    --with-ld-opt='-fsanitize=address,undefined' \
    --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-sanitizer.log 2>&1 \
    || { tail -60 /tmp/cfg-sanitizer.log; exit 1; }

# `-Zsanitizer` is enabled by CI through RUSTUP_TOOLCHAIN=nightly-YYYY-MM-DD.
# Stable local runs still instrument NGINX and native libraries.
if rustc -Z help >/dev/null 2>&1; then
    RUSTFLAGS='-Zsanitizer=address -Cforce-frame-pointers=yes' make \
        >/tmp/make-sanitizer.log 2>&1 || { tail -100 /tmp/make-sanitizer.log; exit 1; }
else
    make >/tmp/make-sanitizer.log 2>&1 || { tail -100 /tmp/make-sanitizer.log; exit 1; }
fi

cat > "$RUN/nginx.conf" <<EOF
load_module $SRC/objs/ngx_http_compress_module.so;
daemon off;
master_process off;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 256; }
http {
    access_log off;
    default_type text/plain;
    server {
        listen $PORT;
        root $WWW;
        location / { compress on; compress_gzip on; compress_min_length 20; }
        location = /page.shtml {
            default_type text/html;
            ssi on;
            compress on;
            compress_gzip on;
            compress_min_length 1;
        }
    }
}
EOF

"$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
ngx_pid=$!
i=0
while [ "$i" -lt 100 ]; do
    curl -s --noproxy '*' "http://127.0.0.1:$PORT/body.txt" >/dev/null 2>&1 && break
    i=$((i + 1)); sleep 0.05
done

curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -o "$RUN/body.gz" \
    "http://127.0.0.1:$PORT/body.txt"
gzip -dc < "$RUN/body.gz" | cmp -s - "$WWW/body.txt"
curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -o "$RUN/ssi.gz" \
    "http://127.0.0.1:$PORT/page.shtml"
gzip -dc < "$RUN/ssi.gz" | grep -q 'INCLUDED SANITIZER SUBREQUEST'

curl -s --noproxy '*' --limit-rate 100k -H 'Accept-Encoding: gzip' \
    "http://127.0.0.1:$PORT/body.txt" -o /dev/null &
client_pid=$!
sleep 0.1
kill -9 "$client_pid" 2>/dev/null || true
wait "$client_pid" 2>/dev/null || true
curl -sf --noproxy '*' "http://127.0.0.1:$PORT/body.txt" >/dev/null

kill -QUIT "$ngx_pid"
wait "$ngx_pid"
if grep -iE 'AddressSanitizer|UndefinedBehaviorSanitizer|runtime error:|LeakSanitizer' \
    "$RUN/logs/error.log" >/dev/null 2>&1; then
    cat "$RUN/logs/error.log"
    exit 1
fi
echo 'PASS [sanitizer]: compression, SSI and disconnect paths are clean'
