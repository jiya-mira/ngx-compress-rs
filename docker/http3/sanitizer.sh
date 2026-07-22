#!/bin/sh
# HTTP/3-specific Clang ASan/UBSan smoke. CI selects pinned Rust nightly so the
# Rust FFI boundary is ASan-instrumented as well as NGINX/OpenSSL/native code.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost
export ASAN_OPTIONS=${ASAN_OPTIONS:-detect_leaks=1:halt_on_error=1:abort_on_error=1}
export UBSAN_OPTIONS=${UBSAN_OPTIONS:-halt_on_error=1:print_stacktrace=1}
export ASAN_SYMBOLIZER_PATH=${ASAN_SYMBOLIZER_PATH:-/usr/bin/llvm-symbolizer-14}

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/src/nginx-1.30.4}
OPENSSL_SRC=${OPENSSL_SRC:-/opt/src/openssl-3.5.7}
CURL=${HTTP3_CURL:-/opt/curl/bin/curl}
SRC=/tmp/ngx-h3-sanitizer
TLS=/tmp/openssl-h3-sanitizer
RUN=/tmp/ngx-h3-sanitizer-run
WWW=/tmp/ngx-h3-sanitizer-www
PORT=8443
SAN_FLAGS='-O1 -g -fsanitize=address,undefined -fno-sanitize=nonnull-attribute -fno-omit-frame-pointer'
# NGINX's ARM HTTP/3 Huffman encoder deliberately emits unaligned u64 stores.
# The release target is x86_64, where the full alignment check remains enabled.
if [ "$(uname -m)" = aarch64 ]; then
    SAN_FLAGS="$SAN_FLAGS -fno-sanitize=alignment"
fi
export CC=clang
export CFLAGS=$SAN_FLAGS

rm -rf "$SRC" "$TLS" "$RUN" "$WWW"
cp -a "$NGINX_SRC" "$SRC"
cp -a "$OPENSSL_SRC" "$TLS"
mkdir -p "$RUN/logs" "$WWW"
: > "$WWW/body.txt"
i=0
while [ "$i" -lt 100000 ]; do
    printf 'HTTP/3 sanitizer payload %06d abcdefghijklmnopqrstuvwxyz\n' "$i" >> "$WWW/body.txt"
    i=$((i + 1))
done
OPENSSL_CONF=/etc/ssl/openssl.cnf /opt/http3/bin/openssl req \
    -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=localhost' \
    -keyout "$RUN/key.pem" -out "$RUN/cert.pem" >/dev/null 2>&1

cd "$SRC"
./configure \
    --with-compat \
    --with-http_ssl_module \
    --with-http_v3_module \
    --with-openssl="$TLS" \
    --with-openssl-opt=no-tests \
    --with-cc-opt="$SAN_FLAGS" \
    --with-ld-opt='-fsanitize=address,undefined' \
    --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-h3-sanitizer.log 2>&1 \
    || { tail -80 /tmp/cfg-h3-sanitizer.log; exit 1; }
if rustc -Z help >/dev/null 2>&1; then
    RUSTFLAGS='-Zsanitizer=address -Cforce-frame-pointers=yes' make \
        >/tmp/make-h3-sanitizer.log 2>&1 || { tail -120 /tmp/make-h3-sanitizer.log; exit 1; }
else
    make >/tmp/make-h3-sanitizer.log 2>&1 || { tail -120 /tmp/make-h3-sanitizer.log; exit 1; }
fi

cat > "$RUN/nginx.conf" <<EOF
load_module $SRC/objs/ngx_http_compress_module.so;
daemon off;
worker_processes 1;
worker_shutdown_timeout 5s;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 256; }
http {
    access_log off;
    default_type text/plain;
    server {
        listen $PORT quic reuseport;
        ssl_certificate $RUN/cert.pem;
        ssl_certificate_key $RUN/key.pem;
        ssl_protocols TLSv1.3;
        root $WWW;
        location / { compress on; compress_gzip on; compress_min_length 20; }
    }
}
EOF

"$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" >"$RUN/sanitizer.log" 2>&1 &
ngx_pid=$!
sleep 0.5
version=$($CURL -sk --http3-only -H 'Accept-Encoding: gzip' \
    -o "$RUN/body.gz" -w '%{http_version}' "https://127.0.0.1:$PORT/body.txt")
[ "$version" = 3 ]
gzip -dc < "$RUN/body.gz" | cmp -s - "$WWW/body.txt"
$CURL -sk --http3-only --limit-rate 64k --max-time 0.2 \
    -H 'Accept-Encoding: gzip' "https://127.0.0.1:$PORT/body.txt" -o /dev/null || true
"$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" -s reload
version=$($CURL -sk --http3-only -o /dev/null -w '%{http_version}' \
    "https://127.0.0.1:$PORT/body.txt")
[ "$version" = 3 ]
kill -QUIT "$(cat "$RUN/nginx.pid")"
wait "$ngx_pid"

if grep -iE 'AddressSanitizer|UndefinedBehaviorSanitizer|runtime error:|LeakSanitizer' \
    "$RUN/sanitizer.log" "$RUN/logs/error.log" >/dev/null 2>&1; then
    cat "$RUN/sanitizer.log" "$RUN/logs/error.log"
    exit 1
fi
echo 'PASS [h3 sanitizer]: QUIC compression, disconnect and reload are clean'
