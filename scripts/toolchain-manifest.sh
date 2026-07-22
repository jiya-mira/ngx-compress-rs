#!/bin/sh
set -eu

output=${1:-toolchain.txt}
nginx_version=${NGINX_VERSION:-1.30.4}
backend=${NGX_COMPRESS_BACKEND:-vendored,system-libs}

{
    echo "commit=$(git rev-parse HEAD)"
    echo "target=$(rustc -vV | sed -n 's/^host: //p')"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "cc=$(cc --version | head -1)"
    echo "nginx=$nginx_version"
    echo "nginx_supported_os=Debian Bookworm"
    echo "codec_backends=$backend"
    echo "cargo_lock_sha256=$(sha256sum Cargo.lock | cut -d' ' -f1)"
    echo "dependencies:"
    cargo tree --locked -p ngx-compress-module --depth 1
} > "$output"
