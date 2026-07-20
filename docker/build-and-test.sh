#!/bin/sh
# Build the ngx-compress module against the pinned NGINX source in BOTH link
# modes (dynamic and static) and verify end-to-end compression: request each
# coding, confirm Content-Encoding, decode the body with a reference tool, and
# compare against the original. Runs inside the Docker image with the repo at
# /repo. Each mode builds in its own NGINX tree so the Rust staticlib is rebuilt
# against that configure's exact ABI.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
RUN_DIR=/tmp/ngx-run
WWW=/tmp/www
PORT=8080

log() { printf '\n=== %s ===\n' "$1"; }

setup_www() {
    mkdir -p "$WWW"
    : > "$WWW/index.txt"
    i=0
    while [ "$i" -lt 220 ]; do
        printf 'The quick brown fox jumps over the lazy dog 0123456789\n' >> "$WWW/index.txt"
        i=$((i + 1))
    done
}

write_conf() {
    mkdir -p "$RUN_DIR/logs"
    cat > "$RUN_DIR/nginx.conf" <<EOF
${1}
daemon off;
master_process off;
error_log $RUN_DIR/logs/error.log info;
pid $RUN_DIR/nginx.pid;
events { worker_connections 64; }
http {
    default_type text/plain;
    access_log off;
    server {
        listen $PORT;
        root $WWW;
        location / {
            compress on;
            compress_gzip on;
            compress_deflate on;
            compress_brotli on;
            compress_zstd on;
            compress_min_length 20;
            compress_buffers 16 8k;
            compress_types text/plain application/json;
        }
    }
}
EOF
}

decode() {
    # $1 = coding; reads $RUN_DIR/c.bin, writes $RUN_DIR/d.txt
    case "$1" in
        gzip) gzip -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null ;;
        br) brotli -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null ;;
        zstd) zstd -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null ;;
        deflate) python3 -c "import zlib,sys;open('$RUN_DIR/d.txt','wb').write(zlib.decompress(open('$RUN_DIR/c.bin','rb').read()))" ;;
    esac
}

check() {
    # $1 = mode label, $2 = coding
    curl -sf --noproxy '*' -H "Accept-Encoding: $2" \
        -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/index.txt" || { echo "FAIL [$1 $2]: request failed"; return 1; }
    grep -qi "^content-encoding: *$2" "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 $2]: Content-Encoding missing"; cat "$RUN_DIR/h.txt"; return 1; }
    decode "$2"
    cmp -s "$RUN_DIR/d.txt" "$WWW/index.txt" \
        || { echo "FAIL [$1 $2]: decoded body != original"; return 1; }
    echo "PASS [$1 $2]: $(wc -c < "$RUN_DIR/c.bin") compressed / $(wc -c < "$WWW/index.txt") original"
}

check_identity() {
    # $1 = mode label; no Accept-Encoding must yield no Content-Encoding
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/index.txt" || { echo "FAIL [$1 identity]: request failed"; return 1; }
    if grep -qi '^content-encoding:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 identity]: unexpected Content-Encoding"; return 1
    fi
    cmp -s "$RUN_DIR/c.bin" "$WWW/index.txt" \
        || { echo "FAIL [$1 identity]: body altered"; return 1; }
    echo "PASS [$1 identity]: served uncompressed intact"
}

smoke() {
    # $1 = nginx binary, $2 = mode label, $3 = load_module line
    [ -x "$1" ] || { echo "FAIL [$2]: nginx binary $1 not found"; return 1; }
    write_conf "$3"
    "$1" -p "$RUN_DIR" -c "$RUN_DIR/nginx.conf" &
    ngx_pid=$!
    i=0
    while [ "$i" -lt 50 ]; do
        curl -s --noproxy '*' "http://127.0.0.1:$PORT/index.txt" >/dev/null 2>&1 && break
        i=$((i + 1)); sleep 0.1
    done

    rc=0
    for coding in gzip deflate br zstd; do
        check "$2" "$coding" || rc=1
    done
    check_identity "$2" || rc=1

    kill "$ngx_pid" 2>/dev/null || true
    wait "$ngx_pid" 2>/dev/null || true
    return "$rc"
}

setup_www

log "toolchain"
cargo --version
rustc --version

log "DYNAMIC build (--add-dynamic-module, --with-compat)"
DYN_SRC=/tmp/ngx-dynamic
rm -rf "$DYN_SRC"; cp -a /opt/nginx-1.28.0 "$DYN_SRC"
cd "$DYN_SRC"
./configure --with-compat --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-dyn.log 2>&1 \
    || { echo "configure (dynamic) failed"; tail -40 /tmp/cfg-dyn.log; exit 1; }
make >/tmp/make-dyn.log 2>&1 \
    || { echo "make (dynamic) failed"; tail -60 /tmp/make-dyn.log; exit 1; }
ls -l "$DYN_SRC/objs/ngx_http_compress_module.so"
smoke "$DYN_SRC/objs/nginx" "dynamic" "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" \
    || exit 1

log "STATIC build (--add-module)"
STATIC_SRC=/tmp/ngx-static
rm -rf "$STATIC_SRC"; cp -a /opt/nginx-1.28.0 "$STATIC_SRC"
cd "$STATIC_SRC"
./configure --add-module="$MODULE_DIR" >/tmp/cfg-static.log 2>&1 \
    || { echo "configure (static) failed"; tail -40 /tmp/cfg-static.log; exit 1; }
make >/tmp/make-static.log 2>&1 \
    || { echo "make (static) failed"; tail -60 /tmp/make-static.log; exit 1; }
smoke "$STATIC_SRC/objs/nginx" "static" "" || exit 1

log "ALL COMPRESSION TESTS PASSED"
