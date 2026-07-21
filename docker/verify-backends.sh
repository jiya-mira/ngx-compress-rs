#!/bin/sh
# Verify both build backends produce a working module:
#   - vendored (default): codecs self-compiled and statically embedded
#   - system-libs:        flate2->libz, zstd->libzstd linked as shared objects
# For each, build a dynamic module and smoke gzip + zstd end-to-end. For the
# system build also assert via ldd that libz/libzstd are shared dependencies.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
WWW=/tmp/www
PORT=8080

mkdir -p "$WWW"
: > "$WWW/index.txt"
i=0
while [ "$i" -lt 220 ]; do
    printf 'The quick brown fox jumps over the lazy dog 0123456789\n' >> "$WWW/index.txt"
    i=$((i + 1))
done

smoke() {
    # $1 = nginx binary, $2 = .so path, $3 = label
    run=/tmp/run-$3
    mkdir -p "$run/logs"
    cat > "$run/n.conf" <<EOF
load_module $2;
daemon off;
master_process off;
error_log $run/logs/e.log info;
pid $run/n.pid;
events { worker_connections 64; }
http {
    default_type text/plain;
    server { listen $PORT; root $WWW;
        location / { compress on; compress_gzip on; compress_brotli on; compress_zstd on; compress_min_length 20; } }
}
EOF
    "$1" -p "$run" -c "$run/n.conf" &
    ngx_pid=$!
    i=0
    while [ "$i" -lt 50 ]; do
        curl -s --noproxy '*' "http://127.0.0.1:$PORT/index.txt" >/dev/null 2>&1 && break
        i=$((i + 1)); sleep 0.1
    done
    rc=0
    for enc in gzip zstd br; do
        curl -sf --noproxy '*' -H "Accept-Encoding: $enc" -D "$run/h" -o "$run/c" \
            "http://127.0.0.1:$PORT/index.txt" || { echo "FAIL [$3 $enc] request"; rc=1; continue; }
        grep -qi "^content-encoding: *$enc" "$run/h" || { echo "FAIL [$3 $enc] no CE"; rc=1; continue; }
        case "$enc" in
            gzip) gzip -dc < "$run/c" > "$run/d" ;;
            zstd) zstd -dc < "$run/c" > "$run/d" ;;
            br) brotli -dc < "$run/c" > "$run/d" ;;
        esac
        cmp -s "$run/d" "$WWW/index.txt" && echo "PASS [$3 $enc]" || { echo "FAIL [$3 $enc] decode"; rc=1; }
    done
    kill "$ngx_pid" 2>/dev/null || true
    wait "$ngx_pid" 2>/dev/null || true
    return "$rc"
}

build() {
    # $1 = backend (vendored|system), $2 = label
    src=/tmp/ngx-$2
    rm -rf "$src"; cp -a /opt/nginx-1.28.0 "$src"; cd "$src"
    NGX_COMPRESS_BACKEND=$1 ./configure --with-compat --add-dynamic-module="$MODULE_DIR" \
        >/tmp/cfg-$2.log 2>&1 || { echo "configure ($2) failed"; tail -30 /tmp/cfg-$2.log; exit 1; }
    NGX_COMPRESS_BACKEND=$1 make >/tmp/make-$2.log 2>&1 \
        || { echo "make ($2) failed"; tail -50 /tmp/make-$2.log; exit 1; }
    echo "$src/objs/ngx_http_compress_module.so"
}

printf '\n=== VENDORED backend ===\n'
so=$(build vendored vendored)
ls -l "$so"
smoke /tmp/ngx-vendored/objs/nginx "$so" vendored || exit 1

printf '\n=== SYSTEM-LIBS backend ===\n'
so=$(build system system)
ls -l "$so"
echo "shared codec libraries:"
ldd "$so" | grep -iE 'libz\.|libzstd|libbrotli' || { echo "FAIL: shared codec libs not linked"; exit 1; }
for lib in 'libz\.' libzstd libbrotlienc; do
    ldd "$so" | grep -qiE "$lib" || { echo "FAIL: $lib not a shared dependency"; exit 1; }
done
smoke /tmp/ngx-system/objs/nginx "$so" system || exit 1

printf '\n=== BOTH BACKENDS OK ===\n'
