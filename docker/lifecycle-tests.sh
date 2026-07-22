#!/bin/sh
# Failure injection plus multi-worker concurrency/reload/soak coverage.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/nginx-1.30.4}
SRC=/tmp/ngx-lifecycle
RUN=/tmp/ngx-lifecycle-run
WWW=/tmp/ngx-lifecycle-www
PORT=8084
SOAK_SECONDS=${SOAK_SECONDS:-300}

log() { printf '\n=== %s ===\n' "$1"; }

setup() {
    rm -rf "$SRC" "$RUN" "$WWW"
    cp -a "$NGINX_SRC" "$SRC"
    mkdir -p "$RUN/logs" "$WWW"
    : > "$WWW/body.txt"
    i=0
    while [ "$i" -lt 12000 ]; do
        printf 'lifecycle payload %06d abcdefghijklmnopqrstuvwxyz\n' "$i" >> "$WWW/body.txt"
        i=$((i + 1))
    done
}

write_conf() {
    # $1 master_process, $2 worker count
    cat > "$RUN/nginx.conf" <<EOF
load_module $SRC/objs/ngx_http_compress_module.so;
env NGX_COMPRESS_FAULT;
daemon off;
master_process $1;
worker_processes $2;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 1024; }
http {
    access_log off;
    default_type text/plain;
    server {
        listen $PORT;
        root $WWW;
        location / {
            compress on;
            compress_gzip on;
            compress_min_length 20;
        }
    }
}
EOF
}

wait_ready() {
    i=0
    while [ "$i" -lt 100 ]; do
        [ -s "$RUN/nginx.pid" ] && kill -0 "$(cat "$RUN/nginx.pid")" 2>/dev/null && return 0
        i=$((i + 1)); sleep 0.05
    done
    return 1
}

stop_nginx() {
    if [ -s "$RUN/nginx.pid" ]; then
        pid=$(cat "$RUN/nginx.pid")
        "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" -s quit 2>/dev/null || true
        i=0
        while [ "$i" -lt 100 ] && kill -0 "$pid" 2>/dev/null; do
            i=$((i + 1)); sleep 0.05
        done
    fi
    wait "${ngx_pid:-0}" 2>/dev/null || true
}

compressed_request() {
    curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN/headers" \
        -o "$RUN/body.gz" "http://127.0.0.1:$PORT/body.txt"
    grep -qi '^content-encoding: *gzip' "$RUN/headers"
    gzip -dc < "$RUN/body.gz" | cmp -s - "$WWW/body.txt"
}

fault_case() {
    fault=$1
    expected=$2
    : > "$RUN/logs/error.log"
    write_conf off 1
    NGX_COMPRESS_FAULT=$fault "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
    ngx_pid=$!
    wait_ready || { echo "FAIL [$fault]: nginx did not start"; return 1; }
    sleep 0.1

    case "$fault" in
        codec_reset)
            compressed_request
            compressed_request
            ;;
        output_allocation|downstream)
            if curl -s --noproxy '*' --max-time 3 -H 'Accept-Encoding: gzip' \
                -D "$RUN/fault-headers" -o "$RUN/fault-body" \
                "http://127.0.0.1:$PORT/body.txt"; then
                if grep -qi '^content-encoding: *gzip' "$RUN/fault-headers"; then
                    gzip -t "$RUN/fault-body"
                fi
            fi
            compressed_request
            ;;
        *)
            curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -D "$RUN/fault-headers" \
                -o "$RUN/fault-body" "http://127.0.0.1:$PORT/body.txt"
            ! grep -qi '^content-encoding:' "$RUN/fault-headers"
            cmp -s "$RUN/fault-body" "$WWW/body.txt"
            compressed_request
            ;;
    esac

    grep -q "class=$expected" "$RUN/logs/error.log" \
        || { echo "FAIL [$fault]: missing class=$expected"; cat "$RUN/logs/error.log"; stop_nginx; return 1; }
    kill -0 "$ngx_pid"
    stop_nginx
    echo "PASS [$fault]: classified failure contained; worker remained healthy"
}

concurrent_batch() {
    seq 1 64 | xargs -P32 -I{} sh -c \
        "curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' 'http://127.0.0.1:$PORT/body.txt' | gzip -t"
}

lifecycle() {
    : > "$RUN/logs/error.log"
    write_conf on 4
    "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
    ngx_pid=$!
    wait_ready || { echo 'FAIL [lifecycle]: nginx did not start'; return 1; }
    sleep 0.3
    master_pid=$(cat "$RUN/nginx.pid")
    # Linux exposes direct children without needing procps in the test image.
    children=$(cat "/proc/$master_pid/task/$master_pid/children")
    worker_count=$(printf '%s\n' "$children" | awk '{ print NF }')
    [ "$worker_count" -eq 4 ] \
        || { echo "FAIL [lifecycle]: expected 4 workers, found $worker_count"; return 1; }
    concurrent_batch

    log '20 graceful reloads under active traffic'
    (
        i=0
        while [ "$i" -lt 80 ]; do
            curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' \
                "http://127.0.0.1:$PORT/body.txt" | gzip -t
            i=$((i + 1))
        done
    ) &
    traffic_pid=$!
    i=0
    while [ "$i" -lt 20 ]; do
        "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" -s reload
        i=$((i + 1)); sleep 0.1
    done
    wait "$traffic_pid"
    concurrent_batch

    log "${SOAK_SECONDS}s short soak"
    deadline=$(( $(date +%s) + SOAK_SECONDS ))
    rounds=0
    while [ "$(date +%s)" -lt "$deadline" ]; do
        concurrent_batch
        rounds=$((rounds + 1))
    done
    kill -0 "$(cat "$RUN/nginx.pid")"
    if grep -iE 'AddressSanitizer|UndefinedBehaviorSanitizer|panic|SIG(SEGV|ABRT)' \
        "$RUN/logs/error.log" >/dev/null 2>&1; then
        echo 'FAIL [lifecycle]: fatal diagnostic in error log'
        grep -iE 'AddressSanitizer|UndefinedBehaviorSanitizer|panic|SIG(SEGV|ABRT)' "$RUN/logs/error.log"
        stop_nginx
        return 1
    fi
    stop_nginx
    echo "PASS [lifecycle]: 4 workers, 32 concurrency, 20 reloads, ${rounds} soak batches"
}

setup
log 'build test-only fault-injection module'
cd "$SRC"
NGX_COMPRESS_TEST_FAULTS=1 ./configure --with-compat --add-dynamic-module="$MODULE_DIR" \
    >/tmp/cfg-lifecycle.log 2>&1 \
    || { tail -50 /tmp/cfg-lifecycle.log; exit 1; }
NGX_COMPRESS_TEST_FAULTS=1 make >/tmp/make-lifecycle.log 2>&1 \
    || { tail -80 /tmp/make-lifecycle.log; exit 1; }

log 'test-only fault injection'
fault_case codec_initialization codec_initialization
fault_case codec_reset codec_reset
fault_case header_allocation output_allocation
fault_case output_allocation output_allocation
fault_case downstream downstream

log 'multi-worker lifecycle'
lifecycle
