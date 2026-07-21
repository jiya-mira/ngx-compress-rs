# ngx-compress-rs

`ngx-compress-rs` is an NGINX HTTP response compression module written in Rust
on top of the official [`nginx/ngx-rust`](https://github.com/nginx/ngx-rust)
integration layer.

The project is currently a **technical preview**. Its first supported deployment
target is a dynamically loaded NGINX module built from source against the exact
NGINX build signature used in production.

## Features

- Runtime compression with `gzip`, `deflate`, Brotli (`br`), and Zstandard
  (`zstd`), plus the identity fallback.
- RFC-aware `Accept-Encoding` negotiation, including quality values, wildcard,
  duplicate coding, explicit `q=0`, and identity handling.
- Precompressed `.gz`, `.br`, and `.zst` sidecar serving.
- `fast`, `balanced`, and `max` configuration profiles with explicit directive
  overrides.
- Worker-local codec reuse and streaming progress validation.
- A narrow NGINX FFI boundary around a safe Rust protocol and policy core.
- Vendored and system-library codec backends.

Compression Dictionary Transport (`dcb` and `dcz`) is intentionally deferred to
a later milestone.

## Compatibility

NGINX dynamic modules are ABI-sensitive. A module must be built against the same
NGINX version, configure arguments, compiler/ABI, and distribution patches as
the target binary. `--with-compat` helps with compatible builds but does not make
one `.so` universal.

The current integration environment validates NGINX 1.28.0 on Debian Bookworm.
Other NGINX build signatures should be treated as unverified until exercised in
the release matrix.

Dynamic loading is the supported target for the first release. Static builds
work for ordinary responses, but static SSI/subrequest response compression has
a documented filter-order limitation; use the dynamic module for those cases.

## Installation

Build and install the module from source by following
[docs/installation.md](docs/installation.md). The short form is:

1. Obtain the exact NGINX source and configure arguments for the target binary.
2. Configure NGINX with `--with-compat` and
   `--add-dynamic-module=/path/to/ngx-compress-rs/crates/ngx-compress-module`.
3. Build `ngx_http_compress_module.so`, install it in the target NGINX module
   directory, and add a top-level `load_module` directive.
4. Run `nginx -t` before reloading NGINX.

## Configuration

The simplest recommended configuration is:

```nginx
load_module modules/ngx_http_compress_module.so;

http {
    compress balanced;
}
```

Manual codec selection is also supported:

```nginx
http {
    compress on;
    compress_zstd on;
    compress_brotli on;
    compress_gzip on;
    compress_min_length 256;
    compress_types text/plain text/css application/json application/javascript;
    compress_priority zstd br gzip;
}
```

See [docs/design.md](docs/design.md#4-content-negotiation-and-server-priority)
for the complete directive schema and precedence rules.

## Development and verification

The host-side commands intentionally exercise only the NGINX-independent crates:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Build the pinned integration image and run the NGINX-dependent suites with:

```sh
docker build -t ngx-compress-build:latest -f docker/Dockerfile .
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/build-and-test.sh
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/edge-tests.sh
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/verify-backends.sh
```

The root workspace uses `default-members` because the FFI and module crates need
an NGINX source/configure tree. Do not replace the host-side commands above with
`cargo test --workspace` unless that environment has been prepared.

## Architecture and safety

- [Architecture and milestones](docs/architecture.md)
- [Detailed module design](docs/design.md)
- [Unsafe-boundary refactor](docs/unsafe-boundary-refactor.md)

The governing rule is `NGINX/codec FFI -> validated prefetch -> safe Rust core ->
typed submit plan -> NGINX/codec FFI`. Panics must not unwind across C callbacks,
and streaming steps must always report and validate consumed input, produced
output, and their next state.

## License

Licensed under the [Apache License 2.0](LICENSE).
