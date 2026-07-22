#!/bin/sh
# Balanced H1/H2/H3 buffer benchmark. Writes raw TSV and toolchain metadata;
# analyze.py applies the release plan's conservative default-change gate.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

MODULE_DIR=/repo/crates/ngx-compress-module
NGINX_SRC=${NGINX_SRC:-/opt/src/nginx-1.30.4}
OPENSSL_SRC=${OPENSSL_SRC:-/opt/src/openssl-3.5.7}
CURL=${HTTP3_CURL:-/opt/curl/bin/curl}
EVIDENCE_DIR=${EVIDENCE_DIR:-/evidence}
ROUNDS=${ROUNDS:-5}
SRC=/tmp/ngx-h3-bench
TLS=/tmp/openssl-h3-bench
RUN=/tmp/ngx-h3-bench-run
WWW=/tmp/ngx-h3-bench-www
PORT=8443

mkdir -p "$EVIDENCE_DIR"
rm -rf "$SRC" "$TLS" "$RUN" "$WWW"
cp -a "$NGINX_SRC" "$SRC"
cp -a "$OPENSSL_SRC" "$TLS"
mkdir -p "$RUN/logs" "$WWW"

python3 - "$WWW" <<'PY'
import pathlib, random, sys
root = pathlib.Path(sys.argv[1])
random.seed(20260722)
for size, label in ((4096, "4k"), (262144, "256k"), (8388608, "8m")):
    pattern = b"compressible HTTP/3 payload abcdefghijklmnopqrstuvwxyz\n"
    (root / f"compressible-{label}.bin").write_bytes((pattern * (size // len(pattern) + 1))[:size])
    (root / f"incompressible-{label}.bin").write_bytes(random.randbytes(size))
PY

OPENSSL_CONF=/etc/ssl/openssl.cnf /opt/http3/bin/openssl req \
    -x509 -newkey rsa:2048 -nodes -days 1 \
    -subj '/CN=localhost' -keyout "$RUN/key.pem" -out "$RUN/cert.pem" >/dev/null 2>&1

cd "$SRC"
./configure \
    --with-compat \
    --with-http_ssl_module \
    --with-http_v2_module \
    --with-http_v3_module \
    --with-openssl="$TLS" \
    --with-openssl-opt=no-tests \
    --add-dynamic-module="$MODULE_DIR" >/tmp/cfg-h3-bench.log 2>&1 \
    || { tail -80 /tmp/cfg-h3-bench.log; exit 1; }
make >/tmp/make-h3-bench.log 2>&1 || { tail -100 /tmp/make-h3-bench.log; exit 1; }

cat > "$EVIDENCE_DIR/toolchain.txt" <<EOF
date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
machine=$(uname -a)
cpu=$(awk -F: '/model name|Model/ { gsub(/^ +/, "", $2); print $2; exit }' /proc/cpuinfo)
rustc=$(rustc --version)
cargo=$(cargo --version)
nginx=1.30.4
openssl=$(/opt/http3/bin/openssl version)
curl=$($CURL --version | head -1)
nghttp3=1.15.0
ngtcp2=1.22.1
rounds=$ROUNDS
profile=balanced
EOF

printf 'buffer_kib\tprotocol\tpayload\tround\tttfb_s\ttotal_s\tspeed_Bps\tcompressed_bytes\tworker_rss_kib\n' \
    > "$EVIDENCE_DIR/http3-raw.tsv"

write_conf() {
    buffer=$1
    cat > "$RUN/nginx.conf" <<EOF
load_module $SRC/objs/ngx_http_compress_module.so;
daemon off;
worker_processes 1;
worker_shutdown_timeout 5s;
error_log $RUN/logs/error.log warn;
pid $RUN/nginx.pid;
events { worker_connections 512; }
http {
    access_log off;
    default_type application/octet-stream;
    server {
        listen $PORT ssl;
        listen $PORT quic reuseport;
        http2 on;
        ssl_certificate $RUN/cert.pem;
        ssl_certificate_key $RUN/key.pem;
        ssl_protocols TLSv1.3;
        root $WWW;
        location / {
            compress on;
            compress_gzip on;
            compress_min_length 1;
            compress_buffers 16 ${buffer}k;
            compress_types application/octet-stream;
        }
    }
}
EOF
}

rss_kib() {
    master=$(cat "$RUN/nginx.pid")
    worker=$(cat "/proc/$master/task/$master/children" | awk '{print $1}')
    awk '/VmRSS:/ { print $2 }' "/proc/$worker/status"
}

measure() {
    buffer=$1
    protocol=$2
    payload=$3
    round=$4
    case "$protocol" in
        h1) option=--http1.1 ;;
        h2) option=--http2 ;;
        h3) option=--http3-only ;;
    esac
    metrics=$($CURL -sk "$option" -H 'Accept-Encoding: gzip' -o /dev/null \
        -w '%{http_version}\t%{time_starttransfer}\t%{time_total}\t%{speed_download}\t%{size_download}' \
        "https://127.0.0.1:$PORT/$payload")
    version=$(printf '%s' "$metrics" | cut -f1)
    case "$protocol:$version" in
        h1:1.1|h2:2|h3:3) ;;
        *) echo "protocol mismatch: $protocol reported $version"; exit 1 ;;
    esac
    values=$(printf '%s' "$metrics" | cut -f2-)
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$buffer" "$protocol" "$payload" "$round" "$values" "$(rss_kib)" \
        >> "$EVIDENCE_DIR/http3-raw.tsv"
}

for buffer in 4 8 16 32; do
    write_conf "$buffer"
    "$SRC/objs/nginx" -p "$RUN" -c "$RUN/nginx.conf" &
    ngx_pid=$!
    sleep 0.5
    for protocol in h1 h2 h3; do
        for payload in \
            compressible-4k.bin incompressible-4k.bin \
            compressible-256k.bin incompressible-256k.bin \
            compressible-8m.bin incompressible-8m.bin; do
            # One unrecorded warm-up precedes at least five measured rounds.
            measure "$buffer" "$protocol" "$payload" warmup
            # Remove the warm-up row rather than maintaining a second curl path.
            sed -i '$d' "$EVIDENCE_DIR/http3-raw.tsv"
            round=1
            while [ "$round" -le "$ROUNDS" ]; do
                measure "$buffer" "$protocol" "$payload" "$round"
                round=$((round + 1))
            done
        done
    done
    kill -QUIT "$(cat "$RUN/nginx.pid")"
    wait "$ngx_pid"
done

python3 /repo/docker/http3/analyze.py \
    "$EVIDENCE_DIR/http3-raw.tsv" "$EVIDENCE_DIR/conclusion.md"
echo "benchmark evidence written to $EVIDENCE_DIR"
