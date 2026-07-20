#!/bin/sh
# Run the full rust-lint-toolkit suite (clippy + heuristic scanners) inside the
# container, where an NGINX source tree lets the nginx-linked crates compile.
# Expects the repo at /repo and the lint skill mounted at /lint.
set -eu

export no_proxy=127.0.0.1,localhost
export NO_PROXY=127.0.0.1,localhost

# A configure (no make) is enough for nginx-sys to generate bindings.
LINT_SRC=/tmp/ngx-lint
rm -rf "$LINT_SRC"; cp -a "${NGINX_SRC:-/opt/nginx-1.28.0}" "$LINT_SRC"
cd "$LINT_SRC"
./configure --with-compat >/tmp/cfg-lint.log 2>&1 \
    || { echo "configure (lint) failed"; tail -30 /tmp/cfg-lint.log; exit 1; }

export NGINX_SOURCE_DIR="$LINT_SRC"
export NGINX_BUILD_DIR="$LINT_SRC/objs"

cd /repo

# The suite's clippy honors default-members (core + codecs). Cover the
# nginx-linked crates explicitly; its heuristic scanners already read every
# .rs file regardless of workspace membership.
echo "=== workspace clippy (ffi + module included) ==="
cargo clippy --workspace --all-targets --locked 2>&1 \
    | grep -nE 'warning:|error' || echo "workspace clippy: clean"

echo "=== rust-lint-toolkit suite ==="
bash /lint/scripts/run-rust-lint-suite.sh --project /repo --mode advisory
