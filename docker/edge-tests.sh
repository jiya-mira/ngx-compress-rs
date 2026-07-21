#!/bin/sh
# Adversarial/edge tests against the dynamic module (the supported target):
#   - backpressure: a large response drained by a slow client (busy/free chains)
#   - client disconnect: abort mid-download, the worker must keep serving
#   - truncated upstream: proxy an upstream that closes mid-stream, no crash
#   - HTTP/2: compression over h2c
# Finally assert the error log has no Rust panic crossing the C ABI.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
RUN=/tmp/edge-run
WWW=/tmp/edge-www
H1=8080
H2=8081
BACKEND=8090

log() { printf '\n=== %s ===\n' "$1"; }

setup() {
    mkdir -p "$WWW" "$RUN/logs"
    printf 'small body for liveness checks 0123456789\n' > "$WWW/index.txt"
    # ~3 MB compressible body to force the send buffer to fill under a slow client.
    : > "$WWW/big.txt"
    i=0
    while [ "$i" -lt 60000 ]; do
        printf 'The quick brown fox jumps over the lazy dog %06d\n' "$i" >> "$WWW/big.txt"
        i=$((i + 1))
    done
}

start_backend() {
    cat > "$RUN/trunc.py" <<PY
import socket
srv = socket.socket()
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", $BACKEND))
srv.listen(8)
while True:
    conn, _ = srv.accept()
    try:
        conn.recv(4096)
        conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n")
        conn.sendall(b"2000\r\n" + b"X" * 0x2000 + b"\r\n")  # one chunk, then close with no terminator
    except OSError:
        pass
    finally:
        conn.close()
PY
    python3 "$RUN/trunc.py" &
    backend_pid=$!
}

write_conf() {
    cat > "$RUN/nginx.conf" <<EOF
load_module $1;
daemon off;
master_process off;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 256; }
http {
    default_type text/plain;
    access_log off;
    server {
        listen $H1;
        root $WWW;
        location / { compress on; compress_gzip on; compress_min_length 20; }
        location /trunc {
            compress on; compress_gzip on; compress_min_length 20;
            proxy_pass http://127.0.0.1:$BACKEND;
        }
    }
    server {
        listen $H2;
        http2 on;
        root $WWW;
        location / { compress on; compress_gzip on; compress_min_length 20; }
    }
}
EOF
}

check_backpressure() {
    curl -s --noproxy '*' --limit-rate 800k -H 'Accept-Encoding: gzip' \
        -D "$RUN/h" -o "$RUN/big.gz" "http://127.0.0.1:$H1/big.txt" || { echo "FAIL [backpressure]: request failed"; return 1; }
    grep -qi '^content-encoding: *gzip' "$RUN/h" || { echo "FAIL [backpressure]: not compressed"; return 1; }
    gzip -dc < "$RUN/big.gz" > "$RUN/big.out" 2>/dev/null || { echo "FAIL [backpressure]: decode failed"; return 1; }
    cmp -s "$RUN/big.out" "$WWW/big.txt" || { echo "FAIL [backpressure]: body mismatch under slow drain"; return 1; }
    echo "PASS [backpressure]: $(wc -c < "$RUN/big.gz")B over slow client, $(wc -c < "$WWW/big.txt")B intact"
}

check_disconnect() {
    curl -s --noproxy '*' --limit-rate 200k -H 'Accept-Encoding: gzip' \
        -o /dev/null "http://127.0.0.1:$H1/big.txt" &
    dl=$!
    sleep 0.3
    kill -9 "$dl" 2>/dev/null || true
    wait "$dl" 2>/dev/null || true
    sleep 0.2
    # The worker must still serve after an abrupt client disconnect.
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -o "$RUN/live.gz" \
        "http://127.0.0.1:$H1/index.txt" && gzip -t "$RUN/live.gz" 2>/dev/null \
        && echo "PASS [disconnect]: worker kept serving after client abort" \
        || { echo "FAIL [disconnect]: worker unhealthy after client abort"; return 1; }
}

check_truncated_upstream() {
    # nginx should terminate the response and stay alive, not crash/hang.
    curl -s --noproxy '*' --max-time 5 -H 'Accept-Encoding: gzip' \
        -o /dev/null "http://127.0.0.1:$H1/trunc" || true
    curl -sf --noproxy '*' "http://127.0.0.1:$H1/index.txt" >/dev/null \
        && echo "PASS [truncated-upstream]: survived premature upstream close" \
        || { echo "FAIL [truncated-upstream]: worker unhealthy"; return 1; }
}

check_http2() {
    version=$(curl -s --noproxy '*' --http2-prior-knowledge -o /dev/null \
        -w '%{http_version}' "http://127.0.0.1:$H2/big.txt") || version=?
    [ "$version" = "2" ] || { echo "FAIL [http2]: not HTTP/2 (got $version)"; return 1; }
    curl -s --noproxy '*' --http2-prior-knowledge -H 'Accept-Encoding: gzip' \
        -D "$RUN/h2h" -o "$RUN/h2.gz" "http://127.0.0.1:$H2/big.txt" || { echo "FAIL [http2]: request failed"; return 1; }
    grep -qi '^content-encoding: *gzip' "$RUN/h2h" || { echo "FAIL [http2]: not compressed"; return 1; }
    gzip -dc < "$RUN/h2.gz" 2>/dev/null | cmp -s - "$WWW/big.txt" \
        && echo "PASS [http2]: gzip compression correct over HTTP/2" \
        || { echo "FAIL [http2]: body mismatch over HTTP/2"; return 1; }
}

check_no_panic() {
    if grep -iE 'panic|rust_backtrace|SIGABRT|internal error' "$RUN/logs/error.log" >/dev/null 2>&1; then
        echo "FAIL [no-panic]: Rust panic in error log"
        grep -iE 'panic|backtrace|abort' "$RUN/logs/error.log" | head
        return 1
    fi
    echo "PASS [no-panic]: no Rust panic crossed the C ABI"
}

setup

log "build dynamic module with HTTP/2"
SRC=/tmp/ngx-edge
rm -rf "$SRC"; cp -a "${NGINX_SRC:-/opt/nginx-1.28.0}" "$SRC"; cd "$SRC"
./configure --with-compat --with-http_v2_module --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-edge.log 2>&1 \
    || { echo "configure failed"; tail -30 /tmp/cfg-edge.log; exit 1; }
make >/tmp/make-edge.log 2>&1 || { echo "make failed"; tail -40 /tmp/make-edge.log; exit 1; }

log "run edge tests"
start_backend
write_conf "$SRC/objs/ngx_http_compress_module.so"
"$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
ngx_pid=$!
i=0
while [ "$i" -lt 50 ]; do
    curl -s --noproxy '*' "http://127.0.0.1:$H1/index.txt" >/dev/null 2>&1 && break
    i=$((i + 1)); sleep 0.1
done

rc=0
check_backpressure || rc=1
check_disconnect || rc=1
check_truncated_upstream || rc=1
check_http2 || rc=1
check_no_panic || rc=1

kill "$ngx_pid" 2>/dev/null || true
kill "$backend_pid" 2>/dev/null || true
wait "$ngx_pid" 2>/dev/null || true

[ "$rc" = 0 ] && log "ALL EDGE TESTS PASSED" || { log "EDGE TESTS FAILED"; exit 1; }
