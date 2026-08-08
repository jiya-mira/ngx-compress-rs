# ngx-compress-rs

`ngx-compress-rs` is an NGINX HTTP response compression module written in Rust
on top of the official [`nginx/ngx-rust`](https://github.com/nginx/ngx-rust)
integration layer.

The project is a source-only **v0.2.0 Technical Preview**: you build it from
source, and no universal binary module is distributed. Trying it out takes only
the [Quick start](#quick-start) below. Deploying it into an NGINX you already run
additionally requires building against that binary's exact version and configure
signature — see [Deploy into your NGINX](#deploy-into-your-nginx).

## Features

- Runtime compression with `gzip`, `deflate`, Brotli (`br`), and Zstandard
  (`zstd`), plus the identity fallback.
- RFC-aware `Accept-Encoding` negotiation, including quality values, wildcard,
  duplicate coding, explicit `q=0`, and identity handling.
- Precompressed `.gz`, `.br`, and `.zst` sidecar serving.
- `fast` and `balanced` configuration profiles with explicit directive
  overrides.
- Worker-local codec reuse, validated progress, and bounded resumable callbacks.
- Compression log variables and an optional `Server-Timing` trailer.
- A narrow NGINX FFI boundary around a safe Rust protocol and policy core.
- Vendored and system-library codec backends.
- HTTP/1.1, HTTP/2, and experimental NGINX HTTP/3 interoperability.

Compression Dictionary Transport (`dcb` and `dcz`) is intentionally deferred to
a later milestone.

## Quick start

Try it in a few minutes. This builds a throwaway NGINX with the module compiled
in — nothing is installed system-wide, and it needs no matching against an
existing NGINX. On Debian/Ubuntu (also install [Rust](https://rustup.rs) 1.85+):

```sh
# 1. Build prerequisites
sudo apt-get install -y \
  git curl gcc make cmake pkg-config clang libclang-dev \
  libpcre2-dev zlib1g-dev libssl-dev

# 2. Get the module and a vanilla NGINX source, side by side
git clone https://github.com/jiya-mira/ngx-compress-rs
curl -fsSL https://nginx.org/download/nginx-1.30.4.tar.gz | tar -xz
cd nginx-1.30.4

# 3. Build NGINX with the module compiled in
./configure --add-module=../ngx-compress-rs/crates/ngx-compress-module
make
```

Then enable it with one directive in a `server` (or `http`) block and start
`objs/nginx`:

```nginx
compress balanced;   # enable gzip / brotli / zstd, negotiated automatically
```

To run it inside **your** NGINX instead of a throwaway build, see
[Deploy into your NGINX](#deploy-into-your-nginx).

## Configuration

`compress balanced;` is the whole turnkey setup. Manual codec selection is also
supported:

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

### Directives

All directives are valid in the `http`, `server`, and `location` contexts.

| Directive | Default | Description |
| --- | --- | --- |
| `compress off\|on\|fast\|balanced` | `off` | Master switch and profile selector. `fast`/`balanced` are presets; `on` is custom mode. |
| `compress_gzip on\|off` | `off` | Enable the gzip codec. |
| `compress_deflate on\|off` | `off` | Enable the raw deflate codec. |
| `compress_brotli on\|off` | `off` | Enable the Brotli (`br`) codec. |
| `compress_zstd on\|off` | `off` | Enable the Zstandard (`zstd`) codec. |
| `compress_gzip_comp_level <1-9>` | `6` | gzip compression level. |
| `compress_deflate_comp_level <1-9>` | `6` | deflate compression level. |
| `compress_brotli_comp_level <0-11>` | `6` | Brotli quality level. |
| `compress_brotli_window <1k-16m>` | `512k` | Brotli sliding-window size. |
| `compress_zstd_comp_level <1-22>` | `3` | Zstandard compression level (negative fast levels allowed). |
| `compress_types <mime>...` | `text/html`, `text/*`, `application/json`, … | MIME allowlist; `*` matches all. |
| `compress_min_length <n>` | `20` | Minimum response size (bytes) to compress at runtime. |
| `compress_vary on\|off` | `on` | Add `Vary: Accept-Encoding`. |
| `compress_buffers <n> <size>` | `16 8k` | Hard per-request output-buffer limit and buffer size. |
| `compress_priority <coding>...` | profile-dependent | Server order used only to break equal client q values. |
| `compress_stats off\|variables\|server_timing` | `off` | Compression variables and optional timing trailer. |
| `compress_static off\|on\|always` | `off` | Serve precompressed `.zst`/`.br`/`.gz` sidecars. |

Precedence is **explicit directive > profile preset > built-in default**,
independent of order (`compress balanced; compress_zstd off;` runs `balanced` without zstd).

Full syntax, contexts, ranges, and behaviour for every directive are in
[docs/directives.md](docs/directives.md).

## Deploy into your NGINX

The Quick start builds a self-contained NGINX so you can try the module without
touching your system. For a real deployment you instead build it as a **dynamic
module** against the exact NGINX you already run, then load the `.so` — no need
to replace your NGINX binary. That path (recording your `nginx -V` signature,
`--with-compat --add-dynamic-module`, `load_module`, the vendored vs system codec
backends, static linking, and HTTP/3) is covered step by step in
[docs/installation.md](docs/installation.md). Read the ABI note below first.

## Compatibility

NGINX dynamic modules are ABI-sensitive. A module must be built against the same
NGINX version, configure arguments, compiler/ABI, and distribution patches as
the target binary. `--with-compat` helps with compatible builds but does not make
one `.so` universal.

The v0.2.0 support baseline is **NGINX 1.30.4, Ubuntu 24.04, Linux x86_64 and ARM64**,
using either dynamic/static linking and vendored/system codec libraries. Other
versions, distributions, architectures, and signatures are unverified. HTTP/3
inherits NGINX upstream's experimental status and does not include 0-RTT.

If built-in `gzip on` and runtime `compress` are both effective in a location,
`nginx -t`, startup, or reload emits one warning and this module fails closed
for that location: runtime compression and sidecar handling are both disabled.
The built-in gzip filter remains authoritative. `compress off` with only
`compress_static on` is not a conflict.

## Contributing

Bug reports, focused feature proposals, documentation improvements, tests, and
pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers the
development setup — host-side checks and the pinned Docker test suites — and the
engineering expectations; read it before submitting a contribution and follow
the [Code of Conduct](CODE_OF_CONDUCT.md).
Report suspected vulnerabilities privately as described in
[SECURITY.md](SECURITY.md).

## Architecture and safety

- [Architecture and milestones](docs/architecture.md)
- [Detailed module design](docs/design.md)
- [Post-v0.1 development plan](docs/roadmap.md)
- [Compression Dictionary Transport design direction](docs/dictionary-transport.md)
- [Unsafe-boundary refactor](docs/unsafe-boundary-refactor.md)
- [v0.2.0 migration guide](docs/migration-v0.2.0.md)
- [v0.2.0 release-readiness checklist](docs/release-readiness-v0.2.0.md)
- [Release, tag, and rollback runbook](docs/release.md)
- [v0.2.0 release notes](docs/releases/v0.2.0.md)
- [v0.1.1 release notes](docs/releases/v0.1.1.md)
- [v0.1.0 release notes](docs/releases/v0.1.0.md)

The governing rule is `NGINX/codec FFI -> validated prefetch -> safe Rust core ->
typed submit plan -> NGINX/codec FFI`. Panics must not unwind across C callbacks,
and streaming steps must always report and validate consumed input, produced
output, and their next state.

## License

Licensed under the [Apache License 2.0](LICENSE).
