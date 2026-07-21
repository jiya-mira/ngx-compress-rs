# Installation

`ngx-compress-rs` is distributed as source during the technical-preview phase.
Build the dynamic module against the exact NGINX build that will load it.

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

Set paths for the checked-out project and the exact NGINX source tree:

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
make -j"$(getconf _NPROCESSORS_ONLN)"
```

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
make -j"$(getconf _NPROCESSORS_ONLN)"
```

Use the vendored backend unless the deployment requires distribution-managed
shared libraries. A system-backend artifact also depends on compatible runtime
versions of those shared libraries.

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

See [design.md](design.md#4-content-negotiation-and-server-priority) for all
directives, defaults, validation ranges, and precedence rules.

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

## Known limitation

The first release supports dynamically loaded modules. A statically linked
module compresses ordinary responses correctly, but NGINX's compile-time filter
order can place it incorrectly for SSI/subrequest-assembled responses. Use the
dynamic module whenever SSI, `add_after_body`, or similar subrequest assembly is
involved.
