# Installation

`ngx-compress-rs` is distributed as source during the technical-preview phase.
The v0.1.1 supported baseline is NGINX 1.30.4 on Debian Bookworm/Linux x86_64.
Build dynamic or static against the exact NGINX build signature that will run
it; the release does not include a generic `.so`.

## Compatibility rule

An NGINX module is coupled to the target binary's build signature:

- NGINX version and downstream distribution patches;
- configure arguments and enabled modules;
- compiler, target architecture, and ABI;
- linked system libraries when using the `system` backend.

`--with-compat` is required for the dynamic build described here, but it is not
a promise that a module built for one arbitrary NGINX package can be loaded by
another. Never publish or deploy an unqualified “universal” module artifact.

Inspect the target before building:

```sh
nginx -V 2>&1
```

Record both the version and the complete `configure arguments` line. When using
a distribution package, obtain its matching patched source package rather than
assuming the pristine upstream tarball has the same ABI.

## Prerequisites

The build needs:

- Rust 1.85 or newer;
- a C toolchain, `make`, CMake, Clang, and libclang;
- the normal dependencies required by the target NGINX configuration;
- `pkg-config`, zlib, Zstandard, and Brotli development packages when using the
  system-library backend.

For Debian Bookworm, a practical development environment is:

```sh
sudo apt-get update
sudo apt-get install -y \
  ca-certificates curl gcc make cmake pkg-config clang libclang-dev \
  libpcre2-dev zlib1g-dev libssl-dev libzstd-dev libbrotli-dev
```

Install Rust using an organization-approved method and verify:

```sh
rustc --version
cargo --version
```

## Build a dynamic module

Set paths for the checked-out project and the exact NGINX source tree.
`MODULE_DIR` is **not** the repo root: it is the in-repo module directory
`crates/ngx-compress-module`, the folder that contains the NGINX module `config`
file, which `./configure` reads through `--add-dynamic-module` / `--add-module`.

```sh
MODULE_DIR=/absolute/path/to/ngx-compress-rs/crates/ngx-compress-module
NGINX_SRC=/absolute/path/to/matching/nginx-source
cd "$NGINX_SRC"
```

Re-run the target binary's configure arguments, then append the module options:

```sh
./configure \
  <the target nginx configure arguments> \
  --with-compat \
  --add-dynamic-module="$MODULE_DIR"
make
```

To parallelize the build, append a job count, e.g. `make -j"$(nproc)"`.

The resulting module is normally:

```text
objs/ngx_http_compress_module.so
```

The default codec backend is `vendored`, which embeds the selected codec
implementations in the module. To link the distribution's zlib, Zstandard, and
Brotli libraries instead, set the backend while configuring:

```sh
NGX_COMPRESS_BACKEND=system ./configure \
  <the target nginx configure arguments> \
  --with-compat \
  --add-dynamic-module="$MODULE_DIR"
make
```

Use the vendored backend unless the deployment requires distribution-managed
shared libraries. A system-backend artifact also depends on compatible runtime
versions of those shared libraries.

### Build flow (what runs when)

The build is a single pass, not two:

- `./configure` generates the `Makefile` and wires in the Rust build; it does not
  compile anything itself.
- `make` invokes `cargo rustc` **once** to build the module staticlib and links
  it into `objs/ngx_http_compress_module.so` (or into `objs/nginx` for a static
  build).
- NGINX cannot track the Rust sources, so **every** `make` re-echoes the full
  `cargo rustc …` command line — including the `make install` step, which
  re-checks the build target. This looks like a second compile but is not: when
  the crate is already up to date cargo prints `Finished … in 0.0Xs` and does no
  work (a full first build takes tens of seconds; the re-check is a fingerprint
  cache hit). `make install`, or copying the `.so` yourself, only installs the
  built artifact.

Do **not** run a host-side `cargo build` first expecting to speed this up: the
NGINX build uses its own `--target-dir` under `objs/`, so a separate `cargo build`
compiles into a different directory and its result is not reused here — which can
look like the crate compiling twice.

### Advanced: build flags (`RUSTFLAGS` / target-cpu)

To pass extra flags to the Rust compiler, set `RUSTFLAGS` in the environment for
both `./configure` and `make` (or export it once for the shell session):

```sh
RUSTFLAGS="-C target-cpu=native" ./configure \
  <the target nginx configure arguments> \
  --with-compat \
  --add-dynamic-module="$MODULE_DIR"
RUSTFLAGS="-C target-cpu=native" make
```

The build system also accepts `NGX_RUSTC_OPT` for options appended directly to
the `cargo rustc` invocation, if you prefer to keep them out of the environment.

`-C target-cpu=native` optimizes for the CPU of the **build** host and ties the
binary to that instruction set. Omit it (or pass a specific baseline such as
`-C target-cpu=x86-64-v2`) when the build host and the deployment host may differ,
or the module can fail with an illegal-instruction crash on older hardware.

## Build statically

Use the target NGINX configure arguments and append `--add-module`:

```sh
NGX_COMPRESS_BACKEND=vendored ./configure \
  <the target nginx configure arguments> \
  --add-module="$MODULE_DIR"
make
```

Use `NGX_COMPRESS_BACKEND=system` for the system codec backend. The resulting
`objs/nginx` contains the module; do not add `load_module`. The build hook
places the filter after SSI/postpone assembly in both static and dynamic modes.

## Install and load

Copy the module to the module directory used by the target NGINX installation.
The exact directory varies by distribution; common locations include
`/usr/lib/nginx/modules` and `/etc/nginx/modules`.

Add `load_module` in the main configuration context, before `events` and `http`:

```nginx
load_module modules/ngx_http_compress_module.so;

events {
}

http {
    compress balanced;
}
```

Validate before applying the configuration:

```sh
nginx -t
```

Then use the deployment's normal graceful-reload mechanism. Keep the previous
module artifact available so the deployment can be rolled back atomically.

## Built-in gzip coexistence

Do not enable runtime `compress` in an effective location that also inherits or
sets built-in `gzip on`. During `nginx -t`, startup, and reload the module emits
one warning for each conflicting effective configuration. At request time it
fails closed: it creates no codec, changes no response header or body, and also
declines sidecars. Built-in gzip continues normally.

A child `gzip off` removes the conflict for that child. Sidecar-only operation
is also valid:

```nginx
location /assets/ {
    gzip on;
    compress off;
    compress_static on;
}
```

## Manual configuration

Profiles are the smallest useful setup:

```nginx
http {
    compress fast;       # or balanced / max
}
```

Explicit directives override profile values independent of directive order:

```nginx
http {
    compress balanced;
    compress_zstd off;
    compress_gzip_comp_level 5;
    compress_types text/plain text/css application/json application/javascript;
    compress_min_length 256;
}
```

Precompressed sidecars are independent of runtime compression:

```nginx
location /assets/ {
    compress off;
    compress_static on;
}
```

See [directives.md](directives.md) for every directive's syntax, default,
context, and validation range, and
[design.md](design.md#4-content-negotiation-and-server-priority) for the
precedence rationale.

## Verify the installed module

Request a compressible resource and retain the encoded body:

```sh
curl --fail --silent --show-error \
  -H 'Accept-Encoding: zstd, br, gzip' \
  -D /tmp/ngx-compress.headers \
  -o /tmp/ngx-compress.body \
  https://example.test/a-compressible-resource
```

Confirm that the response contains the expected `Content-Encoding` and
`Vary: Accept-Encoding`, then decode the body with the corresponding standard
tool and compare it with an identity response.

Also verify at least one response that must remain uncompressed, such as an
unsupported MIME type or a body below `compress_min_length`.

## HTTP/3

HTTP/3 uses the same module build and request path. Build NGINX with the SSL,
HTTP/2, and HTTP/3 modules, then configure a TLS 1.3 QUIC listener:

```sh
./configure \
  <the target nginx configure arguments> \
  --with-http_ssl_module \
  --with-http_v2_module \
  --with-http_v3_module \
  --add-module="$MODULE_DIR"
make
```

```nginx
quic_bpf on;

server {
    listen 443 ssl;
    listen 443 quic reuseport;
    ssl_protocols TLSv1.3;
    ssl_certificate     /path/to/fullchain.pem;
    ssl_certificate_key /path/to/private-key.pem;
    add_header Alt-Svc 'h3=":443"; ma=86400' always;

    compress balanced;
}
```

Verify with a QUIC-capable client that forbids fallback:

```sh
curl --http3-only --fail --show-error \
  -H 'Accept-Encoding: zstd, br, gzip' https://example.test/resource
```

NGINX HTTP/3 is experimental upstream. The v0.1.1 contract covers ordinary
QUIC/HTTP/3 only and excludes 0-RTT. The repository's pinned client uses curl's
non-experimental ngtcp2 backend and asserts the negotiated protocol is HTTP/3.
The supported Linux baseline enables `quic_bpf on` so `reuseport` keeps an
existing QUIC connection routed to its worker across a graceful reload. This
native NGINX directive requires Linux 5.7 or newer and belongs in the `main`
context, outside the `http` block.
