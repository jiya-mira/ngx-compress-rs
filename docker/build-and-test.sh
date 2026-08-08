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
BACKEND=8091
MEMORY_PAYLOAD='MEMORY BUFFER PAYLOAD 0123456789 abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ repeated repeated repeated repeated repeated repeated'

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
    printf 'ADDITION BEFORE\n' > "$WWW/before.txt"
    printf 'ADDITION AFTER\n' > "$WWW/after.txt"
    printf '%s' "$MEMORY_PAYLOAD" > "$WWW/memory.txt"
    cp "$WWW/index.txt" /tmp/upstream.txt

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
    printf 'PLAIN STATIC FILE WITHOUT A SIDECAR\n' > "$WWW/static/plain.txt"
    # A second copy for the `always` location.
    cp "$WWW/static/asset.txt" "$WWW/astatic/asset.txt"
    cp "$WWW/static/asset.txt.gz" "$WWW/astatic/asset.txt.gz"
    cp "$WWW/static/asset.txt.br" "$WWW/astatic/asset.txt.br"
}

start_backend() {
    cat > /tmp/ngx-compress-backend.py <<PY
import gzip, socket
body = open("/tmp/upstream.txt", "rb").read()
encoded = gzip.compress(body)
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", $BACKEND))
server.listen(16)
while True:
    conn, _ = server.accept()
    try:
        conn.recv(4096)
        headers = (
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n"
            b"Content-Encoding: gzip\r\nContent-Length: "
            + str(len(encoded)).encode() + b"\r\nConnection: close\r\n\r\n"
        )
        conn.sendall(headers + encoded)
    finally:
        conn.close()
PY
    python3 /tmp/ngx-compress-backend.py &
    backend_pid=$!
    trap 'kill "$backend_pid" 2>/dev/null || true; wait "$backend_pid" 2>/dev/null || true' EXIT
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
    sendfile on;
    log_format compress_stats '\$compress_coding|\$compress_level|\$compress_input_bytes|\$compress_output_bytes|\$compress_ratio|\$compress_time_ms';
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
            # One small output buffer forces the module to yield to the next
            # filter, reclaim it, and resume the unconsumed input suffix.
            compress_buffers 1 1k;
            compress_types text/plain application/json text/html;
        }
        location = /memory {
            compress on;
            compress_gzip on;
            compress_deflate on;
            compress_brotli on;
            compress_zstd on;
            compress_min_length 20;
            return 200 '$MEMORY_PAYLOAD';
        }
        location = /page.shtml {
            default_type text/html;
            ssi on;
            compress on;
            compress_gzip on;
            compress_min_length 20;
        }
        location = /addition {
            default_type text/plain;
            addition_types text/plain;
            add_before_body /before.txt;
            add_after_body /after.txt;
            compress on;
            compress_gzip on;
            compress_brotli on;
            compress_zstd on;
            compress_min_length 20;
            alias $WWW/index.txt;
        }
        location = /upstream-gzip {
            gunzip on;
            compress on;
            compress_brotli on;
            compress_min_length 20;
            proxy_pass http://127.0.0.1:$BACKEND;
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
        location /astatic-fast/ {
            alias $WWW/astatic/;
            compress fast;
            compress_static always;
        }
        location /static-novary/ {
            alias $WWW/static/;
            compress off;
            compress_static on;
            compress_vary off;
        }
        location /static-priority/ {
            alias $WWW/static/;
            compress off;
            compress_static on;
            compress_priority gzip br;
        }
        location /static-runtime/ {
            alias $WWW/static/;
            compress on;
            compress_gzip on;
            compress_min_length 1;
            compress_static on;
        }
        location = /stats {
            alias $WWW/index.txt;
            compress on;
            compress_gzip on;
            compress_min_length 1;
            compress_buffers 1 1k;
            compress_stats server_timing;
            access_log $RUN_DIR/logs/stats.log compress_stats;
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

check_stats() {
    label=$1
    : > "$RUN_DIR/logs/stats.log"
    curl -sf --http1.1 --noproxy '*' -H 'TE: trailers' -H 'Accept-Encoding: gzip' \
        -D "$RUN_DIR/stats.h" -o "$RUN_DIR/stats.gz" \
        "http://127.0.0.1:$PORT/stats" \
        || { echo "FAIL [$label stats]: request failed"; return 1; }
    gzip -dc < "$RUN_DIR/stats.gz" | cmp -s - "$WWW/index.txt" \
        || { echo "FAIL [$label stats]: invalid gzip body"; return 1; }

    line=$(tail -n 1 "$RUN_DIR/logs/stats.log")
    coding=$(printf '%s' "$line" | cut -d'|' -f1)
    level=$(printf '%s' "$line" | cut -d'|' -f2)
    input=$(printf '%s' "$line" | cut -d'|' -f3)
    output=$(printf '%s' "$line" | cut -d'|' -f4)
    ratio=$(printf '%s' "$line" | cut -d'|' -f5)
    time_ms=$(printf '%s' "$line" | cut -d'|' -f6)
    expected_input=$(wc -c < "$WWW/index.txt" | tr -d ' ')
    expected_output=$(wc -c < "$RUN_DIR/stats.gz" | tr -d ' ')
    if [ "$coding" != gzip ] || [ "$level" != 6 ] \
        || [ "$input" != "$expected_input" ] || [ "$output" != "$expected_output" ] \
        || [ -z "$ratio" ] || [ -z "$time_ms" ]; then
        echo "FAIL [$label stats]: invalid variables: $line"; return 1
    fi
    grep -Eqi '^server-timing: *compress;dur=[0-9.]+;desc="gzip";level=6;input=' \
        "$RUN_DIR/stats.h" \
        || { echo "FAIL [$label stats]: Server-Timing trailer missing"; cat "$RUN_DIR/stats.h"; return 1; }
    echo "PASS [$label stats]: final variables and Server-Timing trailer"
}

check_buffer_source() {
    # $1 = mode label, $2 = source kind, $3 = URI, $4 = expected bytes,
    # $5 = coding. Every case decodes bytes and compares them with the identity
    # representation, rather than accepting Content-Encoding as sufficient.
    curl -sf --noproxy '*' -H "Accept-Encoding: $5" \
        -D "$RUN_DIR/source.h" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT$3" \
        || { echo "FAIL [$1 $2 $5]: request failed"; return 1; }
    grep -qi "^content-encoding: *$5" "$RUN_DIR/source.h" \
        || { echo "FAIL [$1 $2 $5]: Content-Encoding missing"; cat "$RUN_DIR/source.h"; return 1; }
    decode "$5"
    cmp -s "$RUN_DIR/d.txt" "$4" \
        || { echo "FAIL [$1 $2 $5]: decoded body != identity bytes"; return 1; }
    echo "PASS [$1 $2 $5]: decoded bytes match"
}

check_buffer_sources() {
    # Static-file output is an in-file buffer when sendfile is enabled. The
    # return directive supplies a memory buffer. Addition combines the main
    # file with before/after subrequest chains and exercises mixed input.
    curl -sf --noproxy '*' -o "$RUN_DIR/addition.ref" \
        "http://127.0.0.1:$PORT/addition"

    for coding in gzip br zstd; do
        check_buffer_source "$1" file /index.txt "$WWW/index.txt" "$coding" || return 1
        check_buffer_source "$1" memory /memory "$WWW/memory.txt" "$coding" || return 1
        check_buffer_source "$1" mixed-chain /addition "$RUN_DIR/addition.ref" "$coding" || return 1
    done
}

check_representation_headers() {
    # A transformed representation has no stable byte range or original
    # content length. Its static-file ETag must be weakened, and cache variance
    # must name Accept-Encoding.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' \
        -D "$RUN_DIR/representation.h" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/index.txt" \
        || { echo "FAIL [$1 headers]: request failed"; return 1; }
    if grep -qi '^content-length:\|^accept-ranges:' "$RUN_DIR/representation.h"; then
        echo "FAIL [$1 headers]: stale length or range metadata retained"
        cat "$RUN_DIR/representation.h"
        return 1
    fi
    grep -qi '^etag: *W/"' "$RUN_DIR/representation.h" \
        || { echo "FAIL [$1 headers]: ETag missing or not weak"; cat "$RUN_DIR/representation.h"; return 1; }
    grep -qi '^vary:.*Accept-Encoding' "$RUN_DIR/representation.h" \
        || { echo "FAIL [$1 headers]: Vary lacks Accept-Encoding"; cat "$RUN_DIR/representation.h"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" | cmp -s - "$WWW/index.txt" \
        || { echo "FAIL [$1 headers]: transformed bytes damaged"; return 1; }
    echo "PASS [$1 headers]: transformed representation metadata is coherent"
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

check_filter_coexistence() {
    # $1 = mode label
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN_DIR/h.txt" \
        -o "$RUN_DIR/c.bin" "http://127.0.0.1:$PORT/index.txt"
    [ "$(grep -ic '^content-encoding: *gzip' "$RUN_DIR/h.txt")" -eq 1 ]
    if grep -qi '^content-length:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 coexistence]: compressed response retained Content-Length"
        return 1
    fi
    grep -qi '^transfer-encoding: *chunked' "$RUN_DIR/h.txt"
    gzip -dc < "$RUN_DIR/c.bin" | cmp -s - "$WWW/index.txt"

    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -H 'Range: bytes=0-99' \
        -D "$RUN_DIR/range.h" -o "$RUN_DIR/range.body" \
        "http://127.0.0.1:$PORT/index.txt" \
        || { echo "FAIL [$1 coexistence]: range request failed"; return 1; }
    if grep -q '^HTTP/1.1 206' "$RUN_DIR/range.h"; then
        if grep -qi '^content-encoding:' "$RUN_DIR/range.h"; then
            echo "FAIL [$1 coexistence]: partial range response was encoded"
            return 1
        fi
        head -c 100 "$WWW/index.txt" | cmp -s - "$RUN_DIR/range.body" \
            || { echo "FAIL [$1 coexistence]: partial range body was damaged"; return 1; }
    elif grep -q '^HTTP/1.1 200' "$RUN_DIR/range.h"; then
        [ "$(grep -ic '^content-encoding: *gzip' "$RUN_DIR/range.h")" -eq 1 ] \
            || { echo "FAIL [$1 coexistence]: full range fallback has invalid encoding"; return 1; }
        gzip -dc < "$RUN_DIR/range.body" | cmp -s - "$WWW/index.txt" \
            || { echo "FAIL [$1 coexistence]: full range fallback was damaged"; return 1; }
    else
        echo "FAIL [$1 coexistence]: unexpected range status"
        cat "$RUN_DIR/range.h"
        return 1
    fi

    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$RUN_DIR/gunzip.h" \
        -o "$RUN_DIR/gunzip.body" "http://127.0.0.1:$PORT/upstream-gzip"
    [ "$(grep -ic '^content-encoding: *br' "$RUN_DIR/gunzip.h")" -eq 1 ] \
        || { echo "FAIL [$1 coexistence]: gunzip response was not re-encoded as br"; cat "$RUN_DIR/gunzip.h"; return 1; }
    brotli -dc < "$RUN_DIR/gunzip.body" | cmp -s - /tmp/upstream.txt \
        || { echo "FAIL [$1 coexistence]: gunzip/re-encode response was damaged"; return 1; }
    echo "PASS [$1 coexistence]: copy/chunked/range/gunzip filters intact"
}

check_addition() {
    # $1 = mode label; addition subrequests must be assembled before compression.
    curl -sf --noproxy '*' -o "$RUN_DIR/addition.ref" \
        "http://127.0.0.1:$PORT/addition"
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN_DIR/addition.h" \
        -o "$RUN_DIR/addition.gz" "http://127.0.0.1:$PORT/addition"
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/addition.h"
    gzip -dc < "$RUN_DIR/addition.gz" > "$RUN_DIR/addition.out"
    cmp -s "$RUN_DIR/addition.out" "$RUN_DIR/addition.ref"
    grep -q 'ADDITION BEFORE' "$RUN_DIR/addition.out"
    grep -q 'ADDITION AFTER' "$RUN_DIR/addition.out"
    echo "PASS [$1 addition]: before/after subrequests assembled then compressed"
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
    grep -qi '^vary: *Accept-Encoding' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static gzip]: Vary missing"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null
    cmp -s "$RUN_DIR/d.txt" /tmp/sgz.txt \
        || { echo "FAIL [$1 static gzip]: served body is not the .gz sidecar"; return 1; }
    echo "PASS [$1 static gzip]: sidecar served (before static handler)"

    # 2) priority: with gzip+br acceptable, the higher-priority br sidecar wins.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip, br' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static/asset.txt" || { echo "FAIL [$1 static prio]: request failed"; return 1; }
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static prio]: expected br sidecar"; return 1; }
    grep -qi '^vary: *Accept-Encoding' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static prio]: Vary missing"; return 1; }
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
    grep -qi '^vary: *Accept-Encoding' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static none]: identity fallback lacks Vary"; return 1; }
    cmp -s "$RUN_DIR/c.bin" "$WWW/static/asset.txt" \
        || { echo "FAIL [$1 static none]: original not served intact"; return 1; }
    echo "PASS [$1 static none]: declines to original when unaccepted"

    # 4) No sidecar means the resource cannot vary and should not get Vary.
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static/plain.txt" || { echo "FAIL [$1 static plain]: request failed"; return 1; }
    if grep -qi '^vary:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static plain]: Vary present without any sidecar"; return 1
    fi
    cmp -s "$RUN_DIR/c.bin" "$WWW/static/plain.txt" \
        || { echo "FAIL [$1 static plain]: original not served intact"; return 1; }
    echo "PASS [$1 static plain]: no sidecar leaves Vary absent"

    # 5) `always` + no Accept-Encoding: serve the highest-priority sidecar anyway.
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/astatic/asset.txt" || { echo "FAIL [$1 static always]: request failed"; return 1; }
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static always]: expected br sidecar without Accept-Encoding"; return 1; }
    brotli -dc < "$RUN_DIR/c.bin" > "$RUN_DIR/d.txt" 2>/dev/null
    cmp -s "$RUN_DIR/d.txt" /tmp/sbr.txt \
        || { echo "FAIL [$1 static always]: served body is not the .br sidecar"; return 1; }
    if grep -qi '^vary:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static always]: Vary present for non-negotiated mode"; return 1
    fi
    echo "PASS [$1 static always]: serves sidecar regardless of Accept-Encoding"

    # 6) compress_vary off suppresses Vary for encoded and identity responses.
    curl -sf --noproxy '*' -H 'Accept-Encoding: br' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static-novary/asset.txt" || { echo "FAIL [$1 static vary-off encoded]: request failed"; return 1; }
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static vary-off encoded]: expected br sidecar"; return 1; }
    if grep -qi '^vary:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static vary-off encoded]: unexpected Vary"; return 1
    fi
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static-novary/asset.txt" || { echo "FAIL [$1 static vary-off identity]: request failed"; return 1; }
    if grep -qi '^content-encoding:\\|^vary:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static vary-off identity]: unexpected encoding or Vary"; return 1
    fi
    cmp -s "$RUN_DIR/c.bin" "$WWW/static/asset.txt" \
        || { echo "FAIL [$1 static vary-off identity]: original not served intact"; return 1; }
    echo "PASS [$1 static vary-off]: Vary suppressed for both variants"

    # 7) Equal q uses the configured server order, but a higher client q wins.
    curl -sf --noproxy '*' -H 'Accept-Encoding: br, gzip' -D "$RUN_DIR/h.txt" \
        -o "$RUN_DIR/c.bin" "http://127.0.0.1:$PORT/static-priority/asset.txt"
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static configured priority]: expected gzip"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" | cmp -s - /tmp/sgz.txt \
        || { echo "FAIL [$1 static configured priority]: invalid gzip sidecar"; return 1; }

    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip;q=0.5, br;q=1, identity;q=0' \
        -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static-priority/asset.txt"
    grep -qi '^content-encoding: *br' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static client quality]: expected br"; return 1; }
    brotli -dc < "$RUN_DIR/c.bin" | cmp -s - /tmp/sbr.txt \
        || { echo "FAIL [$1 static client quality]: invalid br sidecar"; return 1; }
    echo "PASS [$1 static priority]: q first, configured order only breaks ties"

    # 8) identity participates in q selection; excluding every representation is 406.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip;q=0.5, identity;q=0.8' \
        -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static-priority/asset.txt"
    if grep -qi '^content-encoding:' "$RUN_DIR/h.txt"; then
        echo "FAIL [$1 static identity quality]: unexpected Content-Encoding"; return 1
    fi
    cmp -s "$RUN_DIR/c.bin" "$WWW/static/asset.txt" \
        || { echo "FAIL [$1 static identity quality]: original not served"; return 1; }

    status=$(curl -s --noproxy '*' -H 'Accept-Encoding: *;q=0' -o "$RUN_DIR/c.bin" \
        -w '%{http_code}' "http://127.0.0.1:$PORT/static-priority/asset.txt")
    [ "$status" = 406 ] \
        || { echo "FAIL [$1 static not acceptable]: expected 406, got $status"; return 1; }
    echo "PASS [$1 static identity]: identity q honored and empty set rejected"

    # 9) A missing sidecar must fall through to an acceptable dynamic coding.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip, identity;q=0' \
        -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/static-runtime/plain.txt"
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static runtime fallback]: expected dynamic gzip"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" | cmp -s - "$WWW/static/plain.txt" \
        || { echo "FAIL [$1 static runtime fallback]: invalid dynamic gzip"; return 1; }
    echo "PASS [$1 static runtime fallback]: missing sidecar reaches dynamic coding"

    # 10) `always` bypasses negotiation but still follows the active profile.
    curl -sf --noproxy '*' -D "$RUN_DIR/h.txt" -o "$RUN_DIR/c.bin" \
        "http://127.0.0.1:$PORT/astatic-fast/asset.txt"
    grep -qi '^content-encoding: *gzip' "$RUN_DIR/h.txt" \
        || { echo "FAIL [$1 static always fast]: expected gzip sidecar"; return 1; }
    gzip -dc < "$RUN_DIR/c.bin" | cmp -s - /tmp/sgz.txt \
        || { echo "FAIL [$1 static always fast]: invalid gzip sidecar"; return 1; }
    echo "PASS [$1 static always profile]: fast order used while negotiation bypassed"
}

smoke() {
    # $1 = nginx binary, $2 = mode label, $3 = load_module line
    [ -x "$1" ] || { echo "FAIL [$2]: nginx binary $1 not found"; return 1; }
    write_conf "$3"
    "$1" -p "$RUN_DIR" -c "$RUN_DIR/nginx.conf" -t \
        || { echo "FAIL [$2]: nginx -t"; return 1; }
    cp "$RUN_DIR/nginx.conf" "$RUN_DIR/nginx-valid.conf"
    sed '0,/compress on;/s//compress max;/' "$RUN_DIR/nginx-valid.conf" > "$RUN_DIR/nginx.conf"
    if "$1" -p "$RUN_DIR" -c "$RUN_DIR/nginx.conf" -t >"$RUN_DIR/max.log" 2>&1; then
        echo "FAIL [$2]: removed max profile was accepted"; return 1
    fi
    grep -q 'invalid value for compress directive' "$RUN_DIR/max.log" \
        || { echo "FAIL [$2]: max rejection lacked configuration error"; return 1; }
    mv "$RUN_DIR/nginx-valid.conf" "$RUN_DIR/nginx.conf"
    echo "PASS [$2]: removed max profile rejected by nginx -t"
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
    check_stats "$2" || rc=1
    check_buffer_sources "$2" || rc=1
    check_representation_headers "$2" || rc=1
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
    check_addition "$2" || rc=1
    check_filter_coexistence "$2" || rc=1

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
    test_log="/tmp/gzip-conflict-$2-t.log"
    "$1" -p "$run" -c "$run/nginx.conf" -t >"$test_log" 2>&1 \
        || { echo "FAIL [$2 gzip conflict]: nginx -t"; cat "$test_log"; return 1; }
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
start_backend

log "toolchain"
cargo --version
rustc --version

log "DYNAMIC build (--add-dynamic-module, --with-compat)"
DYN_SRC=/tmp/ngx-dynamic
rm -rf "$DYN_SRC"; cp -a "$NGINX_SRC" "$DYN_SRC"
cd "$DYN_SRC"
./configure --with-compat --with-http_addition_module --with-http_gunzip_module \
    --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-dyn.log 2>&1 \
    || { echo "configure (dynamic) failed"; tail -40 /tmp/cfg-dyn.log; exit 1; }
assert_dynamic_order "$DYN_SRC/objs/ngx_http_compress_module_modules.c"
make >/tmp/make-dyn.log 2>&1 \
    || { echo "make (dynamic) failed"; tail -60 /tmp/make-dyn.log; exit 1; }
ls -l "$DYN_SRC/objs/ngx_http_compress_module.so"
smoke "$DYN_SRC/objs/nginx" "dynamic" "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" \
    || exit 1
smoke_gzip_conflict "$DYN_SRC/objs/nginx" "dynamic" \
    "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" || exit 1

log "DYNAMIC build without built-in gzip"
DYN_NO_GZIP_SRC=/tmp/ngx-dynamic-no-gzip
rm -rf "$DYN_NO_GZIP_SRC"; cp -a "$NGINX_SRC" "$DYN_NO_GZIP_SRC"
cd "$DYN_NO_GZIP_SRC"
./configure --with-compat --without-http_gzip_module \
    --with-http_addition_module --with-http_gunzip_module \
    --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-dyn-no-gzip.log 2>&1 \
    || { echo "configure (dynamic no-gzip) failed"; tail -40 /tmp/cfg-dyn-no-gzip.log; exit 1; }
assert_dynamic_order "$DYN_NO_GZIP_SRC/objs/ngx_http_compress_module_modules.c"
make >/tmp/make-dyn-no-gzip.log 2>&1 \
    || { echo "make (dynamic no-gzip) failed"; tail -60 /tmp/make-dyn-no-gzip.log; exit 1; }
smoke "$DYN_NO_GZIP_SRC/objs/nginx" "dynamic-no-gzip" \
    "load_module $DYN_NO_GZIP_SRC/objs/ngx_http_compress_module.so;" || exit 1

log "STATIC build (--add-module)"
STATIC_SRC=/tmp/ngx-static
rm -rf "$STATIC_SRC"; cp -a "$NGINX_SRC" "$STATIC_SRC"
cd "$STATIC_SRC"
./configure --with-http_addition_module --with-http_gunzip_module \
    --add-module="$MODULE_DIR" >/tmp/cfg-static.log 2>&1 \
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
./configure --without-http_gzip_module --with-http_addition_module --with-http_gunzip_module \
    --add-module="$MODULE_DIR" >/tmp/cfg-static-no-gzip.log 2>&1 \
    || { echo "configure (static no-gzip) failed"; tail -40 /tmp/cfg-static-no-gzip.log; exit 1; }
assert_static_order "$STATIC_NO_GZIP_SRC/objs/ngx_modules.c" ngx_http_range_header_filter_module
make >/tmp/make-static-no-gzip.log 2>&1 \
    || { echo "make (static no-gzip) failed"; tail -60 /tmp/make-static-no-gzip.log; exit 1; }
smoke "$STATIC_NO_GZIP_SRC/objs/nginx" "static-no-gzip" "" || exit 1

log "ALL COMPRESSION TESTS PASSED"
