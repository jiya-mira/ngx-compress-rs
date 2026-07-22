#!/bin/sh
# Run the official Test::Nginx::Socket suite (t/) against a nginx binary with the
# module compiled in statically (no load_module needed). These tests use small
# return-200 bodies, so the static subrequest-position limitation does not apply.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

SRC=/tmp/ngx-tnginx
rm -rf "$SRC"; cp -a "${NGINX_SRC:-/opt/nginx-1.30.4}" "$SRC"; cd "$SRC"
./configure --add-module=/repo/crates/ngx-compress-module >/tmp/cfg-tnginx.log 2>&1 \
    || { echo "configure failed"; tail -30 /tmp/cfg-tnginx.log; exit 1; }
make >/tmp/make-tnginx.log 2>&1 \
    || { echo "make failed"; tail -40 /tmp/make-tnginx.log; exit 1; }

export TEST_NGINX_BINARY="$SRC/objs/nginx"
cd /repo
prove -v t/
