#!/bin/sh
# Verify both build backends produce a working module:
#   - vendored (default): codecs self-compiled and statically embedded
#   - system-libs:        flate2->libz, zstd->libzstd linked as shared objects
# Cross with dynamic/static linking and smoke every result. For system builds,
# also assert via ldd that libz/libzstd/libbrotli are shared dependencies.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/nginx-1.30.4}
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
    # $1 = nginx binary, $2 = optional .so path, $3 = label
    run=/tmp/run-$3
    mkdir -p "$run/logs"
    if [ -n "$2" ]; then
        load="load_module $2;"
    else
        load=
    fi
    cat > "$run/n.conf" <<EOF
$load
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
        if cmp -s "$run/d" "$WWW/index.txt"; then
            echo "PASS [$3 $enc]"
        else
            echo "FAIL [$3 $enc] decode"
            rc=1
        fi
    done
    kill "$ngx_pid" 2>/dev/null || true
    wait "$ngx_pid" 2>/dev/null || true
    return "$rc"
}

build() {
    # $1 = backend (vendored|system), $2 = link (dynamic|static), $3 = label
    src=/tmp/ngx-$3
    rm -rf "$src"; cp -a "$NGINX_SRC" "$src"; cd "$src"
    if [ "$2" = dynamic ]; then
        add="--add-dynamic-module=$MODULE_DIR"
    else
        add="--add-module=$MODULE_DIR"
    fi
    NGX_COMPRESS_BACKEND=$1 ./configure --with-compat "$add" \
        >"/tmp/cfg-$3.log" 2>&1 \
        || { echo "configure ($3) failed"; tail -30 "/tmp/cfg-$3.log"; exit 1; }
    NGX_COMPRESS_BACKEND=$1 make >"/tmp/make-$3.log" 2>&1 \
        || { echo "make ($3) failed"; tail -50 "/tmp/make-$3.log"; exit 1; }
    echo "$src"
}

for backend in vendored system; do
    for link in dynamic static; do
        label=$backend-$link
        printf '\n=== %s ===\n' "$label"
        src=$(build "$backend" "$link" "$label")
        if [ "$link" = dynamic ]; then
            module="$src/objs/ngx_http_compress_module.so"
            ls -l "$module"
            linked=$module
        else
            module=
            linked="$src/objs/nginx"
        fi
        if [ "$backend" = system ]; then
            echo "shared codec libraries:"
            ldd "$linked" | grep -iE 'libz\.|libzstd|libbrotli' \
                || { echo "FAIL: shared codec libs not linked"; exit 1; }
            for lib in 'libz\.' libzstd libbrotlienc; do
                ldd "$linked" | grep -qiE "$lib" \
                    || { echo "FAIL: $lib not a shared dependency"; exit 1; }
            done
        fi
        smoke "$src/objs/nginx" "$module" "$label" || exit 1
    done
done

printf '\n=== ALL LINK/BACKEND COMBINATIONS OK ===\n'
