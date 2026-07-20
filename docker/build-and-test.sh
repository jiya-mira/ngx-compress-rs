#!/bin/sh
# Build the ngx-compress module against the pinned NGINX source in BOTH link
# modes (dynamic and static) and smoke-test each. Runs inside the Docker image
# built from docker/Dockerfile, with the repository mounted at /repo.
set -eu

# Local requests must bypass the build proxy or curl gets a 503 from it.
export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

NGINX_SRC="${NGINX_SRC:-/opt/nginx-1.28.0}"
MODULE_DIR=/repo/crates/ngx-compress-module
RUN_DIR=/tmp/ngx-run
PORT=8080
MARKER='X-Compress-Module: active'
BODY='hello from ngx-compress'

log() { printf '\n=== %s ===\n' "$1"; }

write_conf() {
    # $1 = optional load_module line
    mkdir -p "$RUN_DIR/logs"
    cat > "$RUN_DIR/nginx.conf" <<EOF
${1}
daemon off;
master_process off;
error_log $RUN_DIR/logs/error.log info;
pid $RUN_DIR/nginx.pid;
events { worker_connections 64; }
http {
    access_log off;
    server {
        listen $PORT;
        location / {
            compress on;
            return 200 "$BODY";
        }
    }
}
EOF
}

smoke() {
    # $1 = nginx binary, $2 = mode label, $3 = load_module line (may be empty)
    [ -x "$1" ] || { echo "FAIL [$2]: nginx binary $1 not found"; return 1; }
    write_conf "$3"
    "$1" -p "$RUN_DIR" -c "$RUN_DIR/nginx.conf" &
    ngx_pid=$!
    # wait for the listener
    i=0
    while [ $i -lt 50 ]; do
        if curl -s --noproxy '*' "http://127.0.0.1:$PORT/" >/dev/null 2>&1; then break; fi
        i=$((i + 1)); sleep 0.1
    done

    headers=$(curl -fsS --noproxy '*' -D - -o /tmp/body.txt "http://127.0.0.1:$PORT/")
    body=$(cat /tmp/body.txt)
    kill "$ngx_pid" 2>/dev/null || true
    wait "$ngx_pid" 2>/dev/null || true

    printf '%s\n' "$headers" | grep -qi "$MARKER" \
        || { echo "FAIL [$2]: marker header missing"; printf '%s\n' "$headers"; return 1; }
    [ "$body" = "$BODY" ] \
        || { echo "FAIL [$2]: body mismatch: '$body'"; return 1; }
    echo "PASS [$2]: marker header present and body intact"
}

log "toolchain"
cargo --version
rustc --version

# Each mode builds in its own copy of the NGINX source. The dynamic build needs
# --with-compat and the static build does not, which changes struct layouts;
# isolating the trees gives each its own objs/ and cargo target-dir so the Rust
# staticlib is rebuilt against that configure's exact ABI (no stale-.a reuse).
log "DYNAMIC build (--add-dynamic-module, --with-compat)"
DYN_SRC=/tmp/ngx-dynamic
rm -rf "$DYN_SRC"; cp -a "$NGINX_SRC" "$DYN_SRC"
cd "$DYN_SRC"
./configure --with-compat --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-dyn.log 2>&1 \
    || { echo "configure (dynamic) failed"; tail -40 /tmp/cfg-dyn.log; exit 1; }
# `make` builds both the nginx binary and the dynamic module .so.
make >/tmp/make-dyn.log 2>&1 \
    || { echo "make (dynamic) failed"; tail -60 /tmp/make-dyn.log; exit 1; }
ls -l "$DYN_SRC/objs/ngx_http_compress_module.so"
smoke "$DYN_SRC/objs/nginx" "dynamic" "load_module $DYN_SRC/objs/ngx_http_compress_module.so;" \
    || exit 1

log "STATIC build (--add-module)"
STATIC_SRC=/tmp/ngx-static
rm -rf "$STATIC_SRC"; cp -a "$NGINX_SRC" "$STATIC_SRC"
cd "$STATIC_SRC"
./configure --add-module="$MODULE_DIR" >/tmp/cfg-static.log 2>&1 \
    || { echo "configure (static) failed"; tail -40 /tmp/cfg-static.log; exit 1; }
make >/tmp/make-static.log 2>&1 \
    || { echo "make (static) failed"; tail -60 /tmp/make-static.log; exit 1; }
echo "static nginx built; module linked into the binary (verified by smoke)"
smoke "$STATIC_SRC/objs/nginx" "static" "" || exit 1

log "ALL SMOKE TESTS PASSED"
