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
NGINX_SRC=${NGINX_SRC:-/opt/nginx-1.30.4}
RUN_DIR=/tmp/ngx-run
WWW=/tmp/www
PORT=8080

log() { printf '\n=== %s ===\n' "$1"; }

module_line() {
    grep -n "&$2," "$1" | head -1 | cut -d: -f1
}

assert_static_order() {
    modules=$1
    anchor=$2
    anchor_line=$(module_line "$modules" "$anchor")
    compress_line=$(module_line "$modules" ngx_http_compress_module)
    postpone_line=$(module_line "$modules" ngx_http_postpone_filter_module)

    if [ -z "$anchor_line" ] || [ -z "$compress_line" ] || [ -z "$postpone_line" ] \
        || [ "$anchor_line" -ge "$compress_line" ] \
        || [ "$compress_line" -ge "$postpone_line" ]; then
        echo "FAIL [static order]: expected $anchor -> compress -> postpone"
        grep -nE '&ngx_http_(range_header_filter|gzip_filter|compress|postpone_filter|ssi_filter)_module,' "$modules" || true
        return 1
    fi

    echo "PASS [static order]: $anchor -> compress -> postpone"
}

assert_dynamic_order() {
    order_file=$1
    first=$(grep -A20 'char \*ngx_module_order' "$order_file" | grep '"ngx_http_' | sed -n '1p')
    second=$(grep -A20 'char \*ngx_module_order' "$order_file" | grep '"ngx_http_' | sed -n '2p')
    if ! echo "$first" | grep -q 'ngx_http_compress_module' \
        || ! echo "$second" | grep -q 'ngx_http_postpone_filter_module'; then
        echo "FAIL [dynamic order]: compress must immediately precede postpone"
        grep -A20 'char \*ngx_module_order' "$order_file" || true
        return 1
    fi
    echo "PASS [dynamic order]: compress -> postpone"
}

setup_www() {
    mkdir -p "$WWW"
    # ~1 MB so the codecs stream across many buffers (multi-step buffering),
    # not just a single output buffer.
    : > "$WWW/index.txt"
    i=0
    while [ "$i" -lt 19000 ]; do
        printf 'The quick brown fox jumps over the lazy dog %06d\n' "$i" >> "$WWW/index.txt"
        i=$((i + 1))
    done
    # Subrequest fixtures: an SSI page including inc.txt. Compressing before the
    # include is spliced (wrong filter position) corrupts the assembled output.
    : > "$WWW/inc.txt"
    i=0
    while [ "$i" -lt 120 ]; do
        printf 'INCLUDED SUBREQUEST CONTENT LINE %03d 0123456789\n' "$i" >> "$WWW/inc.txt"
        i=$((i + 1))
    done
    printf 'HEAD\n<!--#include virtual="/inc.txt" -->\nTAIL\n' > "$WWW/page.shtml"

    # Precompressed-sidecar fixtures. The sidecars decode to payloads DISTINCT
    # from the original file, so a matching decoded body proves we served the
    # sidecar file (not a runtime re-compress of the original).
    mkdir -p "$WWW/static" "$WWW/astatic"
    : > "$WWW/static/asset.txt"
    i=0
    while [ "$i" -lt 400 ]; do
        printf 'ORIGINAL PLAINTEXT ASSET LINE %04d\n' "$i" >> "$WWW/static/asset.txt"
        i=$((i + 1))
    done
    : > /tmp/sgz.txt; : > /tmp/sbr.txt
    i=0
    while [ "$i" -lt 300 ]; do
        printf 'GZIP SIDECAR DISTINCT PAYLOAD %04d\n' "$i" >> /tmp/sgz.txt
        printf 'BROTLI SIDECAR DISTINCT PAYLOAD %04d\n' "$i" >> /tmp/sbr.txt
        i=$((i + 1))
    done
    gzip -c /tmp/sgz.txt > "$WWW/static/asset.txt.gz"
    brotli -c /tmp/sbr.txt > "$WWW/static/asset.txt.br"
    # A second copy for the `always` location.
    cp "$WWW/static/asset.txt" "$WWW/astatic/asset.txt"
    cp "$WWW/static/asset.txt.gz" "$WWW/astatic/asset.txt.gz"
    cp "$WWW/static/asset.txt.br" "$WWW/astatic/asset.txt.br"
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
            compress_types text/plain application/json text/html;
        }
        location = /page.shtml {
            default_type text/html;
            ssi on;
            compress on;
            compress_gzip on;
            compress_min_length 20;
        }
        # Sidecar serving with runtime compression OFF, so only compress_static
        # can add a Content-Encoding.
        location /static/ {
            compress off;
            compress_static on;
        }
        location /astatic/ {
            compress off;
            compress_static always;
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

check_ssi() {
    # $1 = mode label. The compressed SSI page must decode to exactly the
    # uncompressed (identity) SSI output — i.e. the include was spliced before
    # compression. A wrong filter position corrupts or truncates the stream.
    curl -sf --noproxy '*' -o "$RUN_DIR/ref.html" \
        "http://127.0.0.1:$PORT/page.shtml" || { echo "FAIL [$1 ssi]: reference request failed"; return 1; }
    grep -q INCLUDED "$RUN_DIR/ref.html" \
        || { echo "FAIL [$1 ssi]: include not resolved in reference"; return 1; }
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/page.shtml" || { echo "FAIL [$1 ssi]: compressed request failed"; return 1; }
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 ssi]: Content-Encoding missing"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/got.html" 2>/dev/null \
        || { echo "FAIL [$1 ssi]: gzip decode failed"; return 1; }
    cmp -s "$RUN_DIR/got.html" "$RUN_DIR/ref.html" \
        || { echo "FAIL [$1 ssi]: subrequest body corrupted under compression"; return 1; }
    echo "PASS [$1 ssi]: subrequest assembled then compressed correctly"
}

check_static() {
    # $1 = mode label. Verifies precompressed-sidecar serving. compress is OFF in
    # these locations, so any Content-Encoding proves the sidecar handler ran
    # (and ran before nginx's static handler).
    # 1) gzip sidecar chosen and served verbatim.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static/asset.txt" || { echo "FAIL [$1 static gzip]: request failed"; return 1; }
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static gzip]: no Content-Encoding (static handler served original?)"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null
    cmp -s "$RUN_DIR/d.txt" /tmp/sgz.txt \
        || { echo "FAIL [$1 static gzip]: served body is not the .gz sidecar"; return 1; }
    echo "PASS [$1 static gzip]: sidecar served (before static handler)"

    # 2) priority: with gzip+br acceptable, the higher-priority br sidecar wins.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip, br' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static/asset.txt" || { echo "FAIL [$1 static prio]: request failed"; return 1; }
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static prio]: expected br sidecar"; return 1; }
    brotli -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null
    cmp -s "$RUN_DIR/d.txt" /tmp/sbr.txt \
        || { echo "FAIL [$1 static prio]: served body is not the .br sidecar"; return 1; }
    echo "PASS [$1 static prio]: br chosen over gzip"

    # 3) `on` + no Accept-Encoding: decline, static serves the original.
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static/asset.txt" || { echo "FAIL [$1 static none]: request failed"; return 1; }
    if grep -qi '^content-encoding:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static none]: unexpected Content-Encoding without Accept-Encoding"; return 1
    fi
    cmp -s "$RUN_DIR/c.bin" "$WWW/static/asset.txt" \
        || { echo "FAIL [$1 static none]: original not served intact"; return 1; }
    echo "PASS [$1 static none]: declines to original when unaccepted"

    # 4) `always` + no Accept-Encoding: serve the highest-priority sidecar anyway.
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/astatic/asset.txt" || { echo "FAIL [$1 static always]: request failed"; return 1; }
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static always]: expected br sidecar without Accept-Encoding"; return 1; }
    brotli -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null
    cmp -s "$RUN_DIR/d.txt" /tmp/sbr.txt \
        || { echo "FAIL [$1 static always]: served body is not the .br sidecar"; return 1; }
    echo "PASS [$1 static always]: serves sidecar regardless of Accept-Encoding"
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
    check_static "$2" || rc=1

    # Worker-local codec reuse: with master_process off there is a single worker,
    # so repeating a coding makes requests 2..N pop a reset codec from the pool.
    # A broken reset would corrupt the 2nd+ decoded body; each must still match.
    for coding in gzip br zstd deflate; do
        n=1
        while [ "$n" -le 4 ]; do
            check "$2 reuse#$n" "$coding" || rc=1
            n=$((n + 1))
        done
    done
    check_ssi "$2" || rc=1

    kill "$ngx_pid" 2>/dev/null || true
    wait "$ngx_pid" 2>/dev/null || true
    return "$rc"
}

smoke_gzip_conflict() {
    # $1 = nginx binary, $2 = label, $3 = optional load_module line
    run=/tmp/ngx-gzip-conflict-$2
    mkdir -p "$run/logs"
    cat > "$run/nginx.conf" <<EOF
$3
daemon off;
master_process off;
error_log $run/logs/error.log info;
pid $run/nginx.pid;
events { worker_connections 64; }
http {
    default_type text/plain;
    access_log off;
    gzip on;
    gzip_types text/plain;
    compress on;
    compress_gzip on;
    compress_brotli on;
    compress_zstd on;
    compress_min_length 20;
    compress_static on;
    server {
        listen $PORT;
        root $WWW;
        location / { }
        location /override/ { alias $WWW/; gzip off; }
        location /conflict-static/ { alias $WWW/static/; }
        location /static-only/ {
            alias $WWW/static/;
            compress off;
            compress_static on;
        }
    }
}
EOF
    : > "$run/logs/error.log"
    "$1" -p "$run" -c "$run/nginx.conf" -t >/tmp/gzip-conflict-$2-t.log 2>&1 \
        || { echo "FAIL [$2 gzip conflict]: nginx -t"; cat /tmp/gzip-conflict-$2-t.log; return 1; }
    if ! grep -q 'class=builtin_gzip_conflict' "$run/logs/error.log"; then
        echo "FAIL [$2 gzip conflict]: nginx -t emitted no warning"
        cat "$run/logs/error.log"
        return 1
    fi

    "$1" -p "$run" -c "$run/nginx.conf" &
    ngx_pid=$!
    i=0
    while [ "$i" -lt 50 ]; do
        curl -s --noproxy '*' "http://127.0.0.1:$PORT/index.txt" >/dev/null 2>&1 && break
        i=$((i + 1)); sleep 0.1
    done
    rc=0

    # Conflict: our br path and sidecar handler both stay untouched.
    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$run/h" -o "$run/c" \
        "http://127.0.0.1:$PORT/index.txt" || rc=1
    if grep -qi '^content-encoding:' "$run/h" || ! cmp -s "$run/c" "$WWW/index.txt"; then
        echo "FAIL [$2 gzip conflict]: runtime compression was not disabled"
        rc=1
    else
        echo "PASS [$2 gzip conflict]: runtime compression disabled"
    fi
    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$run/h" -o "$run/c" \
        "http://127.0.0.1:$PORT/conflict-static/asset.txt" || rc=1
    if grep -qi '^content-encoding:' "$run/h" || ! cmp -s "$run/c" "$WWW/static/asset.txt"; then
        echo "FAIL [$2 gzip conflict]: sidecar handling was not disabled"
        rc=1
    else
        echo "PASS [$2 gzip conflict]: sidecar handling disabled"
    fi

    # Built-in gzip may still encode, but exactly once and with an intact body.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$run/h" -o "$run/c" \
        "http://127.0.0.1:$PORT/index.txt" || rc=1
    if [ "$(grep -ic '^content-encoding: *gzip' "$run/h")" -ne 1 ] \
        || ! gzip -dc < "$run/c" > "$run/d" \
        || ! cmp -s "$run/d" "$WWW/index.txt"; then
        echo "FAIL [$2 gzip conflict]: built-in gzip response damaged or doubled"
        rc=1
    else
        echo "PASS [$2 gzip conflict]: built-in gzip remains single and intact"
    fi

    # Child gzip off accurately overrides inheritance and re-enables this module.
    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$run/h" -o "$run/c" \
        "http://127.0.0.1:$PORT/override/index.txt" || rc=1
    if ! grep -qi '^content-encoding: *br' "$run/h" \
        || ! brotli -dc < "$run/c" > "$run/d" \
        || ! cmp -s "$run/d" "$WWW/index.txt"; then
        echo "FAIL [$2 gzip override]: child gzip off did not re-enable compression"
        rc=1
    else
        echo "PASS [$2 gzip override]: child gzip off re-enables compression"
    fi

    # Sidecar-only mode is explicitly not a conflict.
    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$run/h" -o "$run/c" \
        "http://127.0.0.1:$PORT/static-only/asset.txt" || rc=1
    if ! grep -qi '^content-encoding: *br' "$run/h" \
        || ! brotli -dc < "$run/c" > "$run/d" \
        || ! cmp -s "$run/d" /tmp/sbr.txt; then
        echo "FAIL [$2 gzip static-only]: sidecar-only location treated as conflict"
        rc=1
    else
        echo "PASS [$2 gzip static-only]: sidecar-only location remains enabled"
    fi

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
rm -rf "$DYN_SRC"; cp -a "$NGINX_SRC" "$DYN_SRC"
cd "$DYN_SRC"
./configure --with-compat --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-dyn.log 2>&1 \
    || { echo "configure (dynamic) failed"; tail -40 /tmp/cfg-dyn.log; exit 1; }
assert_dynamic_order "$DYN_SRC/objs/ngx_http_compress_module_modules.c"
make >/tmp/make-dyn.log 2>&1 \
    || { echo "make (dynamic) failed"; tail -60 /tmp/make-dyn.log; exit 1; }
ls -l "$DYN_SRC/objs/ngx_http_compress_module.so"
smoke "$DYN_SRC/objs/nginx" "dynamic" "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" \
    || exit 1
smoke_gzip_conflict "$DYN_SRC/objs/nginx" "dynamic" \
    "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" || exit 1

log "STATIC build (--add-module)"
STATIC_SRC=/tmp/ngx-static
rm -rf "$STATIC_SRC"; cp -a "$NGINX_SRC" "$STATIC_SRC"
cd "$STATIC_SRC"
./configure --add-module="$MODULE_DIR" >/tmp/cfg-static.log 2>&1 \
    || { echo "configure (static) failed"; tail -40 /tmp/cfg-static.log; exit 1; }
assert_static_order "$STATIC_SRC/objs/ngx_modules.c" ngx_http_gzip_filter_module
make >/tmp/make-static.log 2>&1 \
    || { echo "make (static) failed"; tail -60 /tmp/make-static.log; exit 1; }
smoke "$STATIC_SRC/objs/nginx" "static" "" || exit 1
smoke_gzip_conflict "$STATIC_SRC/objs/nginx" "static" "" || exit 1

log "STATIC build without built-in gzip"
STATIC_NO_GZIP_SRC=/tmp/ngx-static-no-gzip
rm -rf "$STATIC_NO_GZIP_SRC"; cp -a "$NGINX_SRC" "$STATIC_NO_GZIP_SRC"
cd "$STATIC_NO_GZIP_SRC"
./configure --without-http_gzip_module --add-module="$MODULE_DIR" >/tmp/cfg-static-no-gzip.log 2>&1 \
    || { echo "configure (static no-gzip) failed"; tail -40 /tmp/cfg-static-no-gzip.log; exit 1; }
assert_static_order "$STATIC_NO_GZIP_SRC/objs/ngx_modules.c" ngx_http_range_header_filter_module
make >/tmp/make-static-no-gzip.log 2>&1 \
    || { echo "make (static no-gzip) failed"; tail -60 /tmp/make-static-no-gzip.log; exit 1; }
smoke "$STATIC_NO_GZIP_SRC/objs/nginx" "static-no-gzip" "" || exit 1

log "ALL COMPRESSION TESTS PASSED"
