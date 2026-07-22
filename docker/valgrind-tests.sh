#!/bin/sh
# Focused supplementary leak/UAF smoke under Valgrind.
set -eu

command -v valgrind >/dev/null 2>&1 || { echo 'valgrind is required'; exit 2; }
export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/nginx-1.30.4}
SRC=/tmp/ngx-valgrind
RUN=/tmp/ngx-valgrind-run
WWW=/tmp/ngx-valgrind-www
PORT=8086

rm -rf "$SRC" "$RUN" "$WWW"
cp -a "$NGINX_SRC" "$SRC"
mkdir -p "$RUN/logs" "$WWW"
: > "$WWW/body.txt"
i=0
while [ "$i" -lt 8000 ]; do
    printf 'valgrind payload %06d abcdefghijklmnopqrstuvwxyz\n' "$i" >> "$WWW/body.txt"
    i=$((i + 1))
done

cd "$SRC"
./configure --with-debug --with-compat --add-dynamic-module="$MODULE_DIR" \
    >/tmp/cfg-valgrind.log 2>&1 || { tail -50 /tmp/cfg-valgrind.log; exit 1; }
make >/tmp/make-valgrind.log 2>&1 || { tail -80 /tmp/make-valgrind.log; exit 1; }

cat > "$RUN/nginx.conf" <<EOF
load_module $SRC/objs/ngx_http_compress_module.so;
daemon off;
master_process off;
error_log $RUN/logs/error.log info;
pid $RUN/nginx.pid;
events { worker_connections 128; }
http {
    access_log off;
    default_type text/plain;
    server {
        listen $PORT;
        root $WWW;
        location / { compress on; compress_gzip on; compress_min_length 20; }
    }
}
EOF

valgrind --quiet --leak-check=full --show-leak-kinds=definite \
    --errors-for-leak-kinds=definite --track-origins=yes --error-exitcode=99 \
    --suppressions=/repo/docker/valgrind.supp \
    --log-file="$RUN/valgrind.log" \
    "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
ngx_pid=$!
i=0
while [ "$i" -lt 200 ]; do
    curl -s --noproxy '*' "http://127.0.0.1:$PORT/body.txt" >/dev/null 2>&1 && break
    i=$((i + 1)); sleep 0.05
done
curl -sf --noproxy '*' -H 'Accept-Encoding: gzip' -o "$RUN/body.gz" \
    "http://127.0.0.1:$PORT/body.txt"
gzip -dc < "$RUN/body.gz" | cmp -s - "$WWW/body.txt"
kill -QUIT "$ngx_pid"
wait "$ngx_pid"

if grep -E 'Invalid read|Invalid write|definitely lost: [1-9]|ERROR SUMMARY: [1-9]' \
    "$RUN/valgrind.log" >/dev/null 2>&1; then
    cat "$RUN/valgrind.log"
    exit 1
fi
echo 'PASS [valgrind]: no invalid access or attributable definite leak'
