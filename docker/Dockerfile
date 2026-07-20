# Build/integration environment for ngx-compress-rs.
#
# Provides a Rust toolchain, NGINX build dependencies, and a pinned NGINX source
# tree. The actual module build (static and dynamic) and smoke tests run through
# docker/build-and-test.sh so iteration does not require rebuilding this image.
FROM rust:1-bookworm

ARG HTTP_PROXY=
ARG HTTPS_PROXY=
ARG NGINX_VERSION=1.28.0

ENV http_proxy=${HTTP_PROXY} \
    https_proxy=${HTTPS_PROXY} \
    HTTP_PROXY=${HTTP_PROXY} \
    HTTPS_PROXY=${HTTPS_PROXY} \
    NGINX_VERSION=${NGINX_VERSION} \
    NGINX_SRC=/opt/nginx-${NGINX_VERSION}

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl gcc make ripgrep \
        clang libclang-dev python3 \
        libpcre2-dev zlib1g-dev libssl-dev \
        zstd brotli gzip \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

WORKDIR /opt
RUN curl -fsSL "https://nginx.org/download/nginx-${NGINX_VERSION}.tar.gz" -o nginx.tar.gz \
    && tar xzf nginx.tar.gz \
    && rm nginx.tar.gz

WORKDIR /repo
