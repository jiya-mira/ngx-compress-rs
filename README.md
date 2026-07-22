# ngx-compress-rs

`ngx-compress-rs` is an NGINX HTTP response compression module written in Rust
on top of the official [`nginx/ngx-rust`](https://github.com/nginx/ngx-rust)
integration layer.

The project is a source-only **v0.1.0 Technical Preview**. It supports dynamic
and static builds, but every build must use the exact NGINX source and configure
signature of the target deployment. No universal binary module is distributed.

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
- HTTP/1.1, HTTP/2, and experimental NGINX HTTP/3 interoperability.

Compression Dictionary Transport (`dcb` and `dcz`) is intentionally deferred to
a later milestone.

## Compatibility

NGINX dynamic modules are ABI-sensitive. A module must be built against the same
NGINX version, configure arguments, compiler/ABI, and distribution patches as
the target binary. `--with-compat` helps with compatible builds but does not make
one `.so` universal.

The v0.1.0 support baseline is **NGINX 1.30.4, Debian Bookworm, Linux x86_64**,
using either dynamic/static linking and vendored/system codec libraries. Other
versions, distributions, architectures, and signatures are unverified. HTTP/3
inherits NGINX upstream's experimental status and does not include 0-RTT.

If built-in `gzip on` and runtime `compress` are both effective in a location,
`nginx -t`, startup, or reload emits one warning and this module fails closed
for that location: runtime compression and sidecar handling are both disabled.
The built-in gzip filter remains authoritative. `compress off` with only
`compress_static on` is not a conflict.

## Installation

Build and install the module from source by following
[docs/installation.md](docs/installation.md). The short form is:

1. Obtain the exact NGINX source and configure arguments for the target binary.
2. For a dynamic build, configure NGINX with `--with-compat` and
   `--add-dynamic-module=/path/to/ngx-compress-rs/crates/ngx-compress-module`.
   For a static build, use `--add-module` with the same exact signature.
3. Build and install the resulting NGINX/module artifact. Dynamic builds add a
   top-level `load_module`; static builds do not.
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

docker build -t ngx-compress-http3:latest -f docker/http3/Dockerfile .
docker run --rm -v "$PWD:/repo" ngx-compress-http3:latest \
  sh /repo/docker/http3/test.sh
```

The root workspace uses `default-members` because the FFI and module crates need
an NGINX source/configure tree. Do not replace the host-side commands above with
`cargo test --workspace` unless that environment has been prepared.

## Contributing

Bug reports, focused feature proposals, documentation improvements, tests, and
pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before
submitting a contribution and follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Report suspected vulnerabilities privately as described in
[SECURITY.md](SECURITY.md).

## Architecture and safety

- [Architecture and milestones](docs/architecture.md)
- [Detailed module design](docs/design.md)
- [Post-v0.1 roadmap](docs/roadmap.md)
- [Unsafe-boundary refactor](docs/unsafe-boundary-refactor.md)
- [Release-readiness checklist](docs/release-readiness.md)
- [Release, tag, and rollback runbook](docs/release.md)
- [v0.1.0 release notes](docs/releases/v0.1.0.md)

The governing rule is `NGINX/codec FFI -> validated prefetch -> safe Rust core ->
typed submit plan -> NGINX/codec FFI`. Panics must not unwind across C callbacks,
and streaming steps must always report and validate consumed input, produced
output, and their next state.

## License

Licensed under the [Apache License 2.0](LICENSE).
