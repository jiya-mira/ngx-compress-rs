#!/bin/sh
# Dynamic/static HTTP/3 integration. Every QUIC request is --http3-only and
# asserts curl's negotiated protocol, so HTTP/2 fallback cannot pass.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/src/nginx-1.30.4}
OPENSSL_SRC=${OPENSSL_SRC:-/opt/src/openssl-3.5.7}
CURL=${HTTP3_CURL:-/opt/curl/bin/curl}
RUN=/tmp/ngx-h3-run
WWW=/tmp/ngx-h3-www
PORT=8443
BACKEND=8093
DIAGNOSTIC_DIR=${DIAGNOSTIC_DIR:-/repo/artifacts/http3/integration-details}

log() { printf '\n=== %s ===\n' "$1"; }

capture_diagnostics() {
    mkdir -p "$DIAGNOSTIC_DIR"
    for file in /tmp/cfg-h3-*.log /tmp/make-h3-*.log "$RUN/logs/error.log"; do
        if [ -f "$file" ]; then
            cp "$file" "$DIAGNOSTIC_DIR/$(basename "$file")"
        fi
    done
    chmod -R a+rX "$DIAGNOSTIC_DIR"
}
trap capture_diagnostics EXIT

setup() {
    rm -rf "$RUN" "$WWW"
    mkdir -p "$RUN/logs" "$WWW"
    : > "$WWW/body.txt"
    i=0
    while [ "$i" -lt 160000 ]; do
        printf 'HTTP/3 compression payload %06d abcdefghijklmnopqrstuvwxyz\n' "$i" >> "$WWW/body.txt"
        i=$((i + 1))
    done
    OPENSSL_CONF=/etc/ssl/openssl.cnf /opt/http3/bin/openssl req \
        -x509 -newkey rsa:2048 -nodes -days 1 \
        -subj '/CN=localhost' -keyout "$RUN/key.pem" -out "$RUN/cert.pem" \
        >/dev/null 2>&1
    cat > "$RUN/chunked.py" <<PY
import socket
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", $BACKEND))
server.listen(32)
payload = (b"chunked upstream payload abcdefghijklmnopqrstuvwxyz\\n" * 4096)
while True:
    conn, _ = server.accept()
    try:
        conn.recv(4096)
        conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n")
        for offset in range(0, len(payload), 4096):
            chunk = payload[offset:offset + 4096]
            conn.sendall(("%x\r\n" % len(chunk)).encode() + chunk + b"\r\n")
        conn.sendall(b"0\r\n\r\n")
    finally:
        conn.close()
PY
    python3 "$RUN/chunked.py" &
    backend_pid=$!
}

build_mode() {
    mode=$1
    src=/tmp/ngx-h3-$mode
    tls=/tmp/openssl-h3-$mode
    rm -rf "$src" "$tls"
    cp -a "$NGINX_SRC" "$src"
    cp -a "$OPENSSL_SRC" "$tls"
    cd "$src"
    if [ "$mode" = dynamic ]; then
        add="--add-dynamic-module=$MODULE_DIR"
    else
        add="--add-module=$MODULE_DIR"
    fi
    ./configure \
        --with-compat \
        --with-http_ssl_module \
        --with-http_v2_module \
        --with-http_v3_module \
        --with-openssl="$tls" \
        --with-openssl-opt=no-tests \
        "$add" >"/tmp/cfg-h3-$mode.log" 2>&1 \
        || { tail -80 "/tmp/cfg-h3-$mode.log"; exit 1; }
    make >"/tmp/make-h3-$mode.log" 2>&1 \
        || { tail -100 "/tmp/make-h3-$mode.log"; exit 1; }
}

write_conf() {
    mode=$1
    if [ "$mode" = dynamic ]; then
        load="load_module /tmp/ngx-h3-dynamic/objs/ngx_http_compress_module.so;"
    else
        load=
    fi
    cat > "$RUN/nginx.conf" <<EOF
$load
daemon off;
worker_processes 2;
worker_shutdown_timeout 15s;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 512; }
http {
    access_log off;
    default_type text/plain;
    server {
        listen $PORT quic reuseport;
        ssl_certificate $RUN/cert.pem;
        ssl_certificate_key $RUN/key.pem;
        ssl_protocols TLSv1.3;
        root $WWW;
        location / {
            compress on;
            compress_gzip on;
            compress_deflate on;
            compress_brotli on;
            compress_zstd on;
            compress_min_length 20;
            compress_types text/plain;
        }
        location = /chunked {
            compress on;
            compress_gzip on;
            compress_min_length 20;
            proxy_pass http://127.0.0.1:$BACKEND;
        }
    }
}
EOF
}

request() {
    # $1 path, $2 encoding, $3 output stem
    version=$($CURL -sk --http3-only -H "Accept-Encoding: $2" \
        -D "$3.headers" -o "$3.body" -w '%{http_version}' \
        "https://127.0.0.1:$PORT$1")
    [ "$version" = 3 ] || { echo "FAIL [protocol]: expected HTTP/3, got $version"; return 1; }
}

decode() {
    case "$1" in
        gzip) gzip -dc < "$2" ;;
        deflate) python3 -c 'import sys,zlib; sys.stdout.buffer.write(zlib.decompress(sys.stdin.buffer.read()))' < "$2" ;;
        br) brotli -dc < "$2" ;;
        zstd) zstd -qdc < "$2" ;;
    esac
}

check_coding() {
    mode=$1
    coding=$2
    stem="$RUN/$mode-$coding"
    request /body.txt "$coding" "$stem"
    [ "$(grep -ic "^content-encoding: *$coding" "$stem.headers")" -eq 1 ]
    if grep -qi '^content-length:' "$stem.headers"; then
        echo "FAIL [$mode h3 $coding]: compressed response retained content length"
        return 1
    fi
    decode "$coding" "$stem.body" | cmp -s - "$WWW/body.txt"
    echo "PASS [$mode h3 $coding]: byte-exact roundtrip"
}

check_identity() {
    mode=$1
    stem="$RUN/$mode-identity"
    version=$($CURL -sk --http3-only -D "$stem.headers" -o "$stem.body" \
        -w '%{http_version}' "https://127.0.0.1:$PORT/body.txt")
    [ "$version" = 3 ]
    if grep -qi '^content-encoding:' "$stem.headers"; then
        echo "FAIL [$mode h3 identity]: unexpected content encoding"
        return 1
    fi
    cmp -s "$stem.body" "$WWW/body.txt"
    echo "PASS [$mode h3 identity]: unchanged"
}

check_chunked_upstream() {
    mode=$1
    stem="$RUN/$mode-chunked"
    request /chunked gzip "$stem"
    grep -qi '^content-encoding: *gzip' "$stem.headers"
    gzip -dc < "$stem.body" > "$stem.decoded"
    python3 -c 'import sys; sys.stdout.buffer.write(b"chunked upstream payload abcdefghijklmnopqrstuvwxyz\n" * 4096)' \
        | cmp -s - "$stem.decoded"
    echo "PASS [$mode h1-upstream-to-h3]: intact"
}

check_pressure() {
    mode=$1
    version=$($CURL -sk --http3-only --limit-rate 256k -H 'Accept-Encoding: gzip' \
        -o "$RUN/$mode-slow.gz" -w '%{http_version}' "https://127.0.0.1:$PORT/body.txt")
    [ "$version" = 3 ]
    gzip -dc < "$RUN/$mode-slow.gz" | cmp -s - "$WWW/body.txt"

    seq 1 32 | xargs -P16 -I{} sh -c \
        "$CURL -sk --http3-only -H 'Accept-Encoding: gzip' 'https://127.0.0.1:$PORT/body.txt' | gzip -t"
    $CURL -sk --http3-only --limit-rate 64k --max-time 0.2 \
        -H 'Accept-Encoding: gzip' "https://127.0.0.1:$PORT/body.txt" -o /dev/null || true
    version=$($CURL -sk --http3-only -o /dev/null -w '%{http_version}' \
        "https://127.0.0.1:$PORT/body.txt")
    [ "$version" = 3 ]
    echo "PASS [$mode h3 pressure]: slow, concurrent and interrupted clients"
}

check_reload() {
    mode=$1
    binary=/tmp/ngx-h3-$mode/objs/nginx
    reload_body="$RUN/$mode-reload.body"
    reload_version="$RUN/$mode-reload.version"
    $CURL -sk --http3-only --limit-rate 2m -H 'Accept-Encoding: identity' \
        -o "$reload_body" -w '%{http_version}' \
        "https://127.0.0.1:$PORT/body.txt" > "$reload_version" &
    active_pid=$!
    i=0
    while [ "$i" -lt 100 ] && [ ! -s "$reload_body" ]; do
        kill -0 "$active_pid" 2>/dev/null \
            || { echo "FAIL [$mode h3 reload]: request ended before reload"; return 1; }
        i=$((i + 1))
        sleep 0.02
    done
    [ -s "$reload_body" ] \
        || { echo "FAIL [$mode h3 reload]: request did not start"; return 1; }
    "$binary" -p "$RUN" -c "$RUN/nginx.conf" -s reload
    wait "$active_pid"
    [ "$(cat "$reload_version")" = 3 ]
    cmp -s "$reload_body" "$WWW/body.txt"
    version=$($CURL -sk --http3-only -o /dev/null -w '%{http_version}' \
        "https://127.0.0.1:$PORT/body.txt")
    [ "$version" = 3 ]
    echo "PASS [$mode h3 reload]: active QUIC traffic survived"
}

run_mode() {
    mode=$1
    binary=/tmp/ngx-h3-$mode/objs/nginx
    : > "$RUN/logs/error.log"
    write_conf "$mode"
    "$binary" -p "$RUN" -c "$RUN/nginx.conf" -t \
        || { echo "FAIL [$mode h3]: nginx -t"; return 1; }
    "$binary" -p "$RUN" -c "$RUN/nginx.conf" &
    ngx_pid=$!
    sleep 0.5
    for coding in gzip deflate br zstd; do
        check_coding "$mode" "$coding"
    done
    check_identity "$mode"
    check_chunked_upstream "$mode"
    check_pressure "$mode"
    check_reload "$mode"
    kill -QUIT "$(cat "$RUN/nginx.pid")"
    wait "$ngx_pid"
    if grep -iE 'panic|SIG(SEGV|ABRT)|AddressSanitizer|runtime error:' "$RUN/logs/error.log" >/dev/null 2>&1; then
        cat "$RUN/logs/error.log"
        return 1
    fi
}

setup
$CURL --version
for mode in dynamic static; do
    log "build $mode HTTP/3 module"
    build_mode "$mode"
    log "test $mode HTTP/3 module"
    run_mode "$mode"
done
kill "$backend_pid" 2>/dev/null || true
wait "$backend_pid" 2>/dev/null || true
log 'ALL HTTP/3 TESTS PASSED WITHOUT FALLBACK'
