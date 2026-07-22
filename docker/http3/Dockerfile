# Dedicated QUIC/HTTP/3 toolchain; intentionally separate from the regular
# build image because curl's ngtcp2 backend is expensive to compile.
ARG BASE_IMAGE=rust:1-bookworm
FROM ${BASE_IMAGE}

ARG INSTALL_DEPS=1

ARG NGINX_VERSION=1.30.4
ARG NGINX_SHA256=4261dc90e9e47c1c4041276e9aaa3d48ebe2e664f728e14fa95ae6c67d57a08b
ARG OPENSSL_VERSION=3.5.7
ARG OPENSSL_SHA256=a8c0d28a529ca480f9f36cf5792e2cd21984552a3c8e4aa11a24aa31aeac98e8
ARG NGHTTP3_VERSION=1.15.0
ARG NGHTTP3_SHA256=6da0cd06b428d32a54c58137838505d9dc0371a900bb8070a46b29e1ceaf2e0f
ARG NGTCP2_VERSION=1.22.1
ARG NGTCP2_SHA256=dfd2c68bd64b89847c611425b9487105c46e8447b5c21e6aeb00642c8fbe2ca8
ARG CURL_VERSION=8.21.0
ARG CURL_SHA256=aa1b66a70eace83dc624508745646c08ae561de512ab403adffb93ac87fc72e6

RUN if [ "${INSTALL_DEPS}" = 1 ]; then apt-get update \
    && apt-get install -y --no-install-recommends \
        autoconf automake ca-certificates clang curl gcc libclang-rt-14-dev libtool llvm-14 make \
        pkg-config perl python3 libbrotli-dev libnghttp2-dev libpcre2-dev libzstd-dev zlib1g-dev \
        brotli gzip zstd \
    && rm -rf /var/lib/apt/lists/*; fi

WORKDIR /opt/src
RUN curl -fsSLO "https://nginx.org/download/nginx-${NGINX_VERSION}.tar.gz" \
    && echo "${NGINX_SHA256}  nginx-${NGINX_VERSION}.tar.gz" | sha256sum -c - \
    && tar xzf "nginx-${NGINX_VERSION}.tar.gz" \
    && curl -fsSLO "https://www.openssl.org/source/openssl-${OPENSSL_VERSION}.tar.gz" \
    && echo "${OPENSSL_SHA256}  openssl-${OPENSSL_VERSION}.tar.gz" | sha256sum -c - \
    && tar xzf "openssl-${OPENSSL_VERSION}.tar.gz" \
    && curl -fsSLO "https://github.com/ngtcp2/nghttp3/releases/download/v${NGHTTP3_VERSION}/nghttp3-${NGHTTP3_VERSION}.tar.xz" \
    && echo "${NGHTTP3_SHA256}  nghttp3-${NGHTTP3_VERSION}.tar.xz" | sha256sum -c - \
    && tar xJf "nghttp3-${NGHTTP3_VERSION}.tar.xz" \
    && curl -fsSLO "https://github.com/ngtcp2/ngtcp2/releases/download/v${NGTCP2_VERSION}/ngtcp2-${NGTCP2_VERSION}.tar.xz" \
    && echo "${NGTCP2_SHA256}  ngtcp2-${NGTCP2_VERSION}.tar.xz" | sha256sum -c - \
    && tar xJf "ngtcp2-${NGTCP2_VERSION}.tar.xz" \
    && curl -fsSLO "https://curl.se/download/curl-${CURL_VERSION}.tar.xz" \
    && echo "${CURL_SHA256}  curl-${CURL_VERSION}.tar.xz" | sha256sum -c - \
    && tar xJf "curl-${CURL_VERSION}.tar.xz"

RUN cd "openssl-${OPENSSL_VERSION}" \
    && ./Configure --prefix=/opt/http3 --libdir=lib no-shared \
    && make -j"$(nproc)" \
    && make install_sw \
    && make clean

ENV PKG_CONFIG_PATH=/opt/http3/lib/pkgconfig
RUN cd "nghttp3-${NGHTTP3_VERSION}" \
    && ./configure --prefix=/opt/http3 --enable-lib-only \
    && make -j"$(nproc)" \
    && make install

RUN cd "ngtcp2-${NGTCP2_VERSION}" \
    && ./configure --prefix=/opt/http3 --enable-lib-only --with-openssl \
    && make -j"$(nproc)" \
    && make install

RUN cd "curl-${CURL_VERSION}" \
    && LDFLAGS='-Wl,-rpath,/opt/http3/lib' ./configure \
        --prefix=/opt/curl \
        --disable-shared \
        --with-openssl=/opt/http3 \
        --with-nghttp2=/usr \
        --with-nghttp3=/opt/http3 \
        --with-ngtcp2=/opt/http3 \
        --without-libpsl \
    && make -j"$(nproc)" \
    && make install \
    && /opt/curl/bin/curl --version | grep -q 'HTTP3'

RUN rustup component add clippy rustfmt

ENV NGINX_SRC=/opt/src/nginx-${NGINX_VERSION} \
    OPENSSL_SRC=/opt/src/openssl-${OPENSSL_VERSION} \
    HTTP3_CURL=/opt/curl/bin/curl
WORKDIR /repo
