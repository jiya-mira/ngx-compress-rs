# v0.1.1 release readiness

This patch release corrects response-cache variance for precompressed sidecars
and removes stale boundary flags from recycled output buffers. It retains the
v0.1.0 source-only Technical Preview support contract.

## Fixed release contract

- NGINX 1.30.4, Debian Bookworm, Linux x86_64.
- Dynamic/static linking crossed with vendored/system codec backends.
- HTTP/1.1, HTTP/2, and experimental NGINX HTTP/3 without 0-RTT.
- `gzip`, `deflate`, `br`, `zstd`, and `identity`; no `dcb`/`dcz`.
- Source archive only; no universal module binary or new runtime dependency.

## Required evidence before tag

- The exact release commit must be pushed to `origin/master`.
- All five exact-commit checks must pass:
  - `Rust / rust`
  - `NGINX integration / integration`
  - `HTTP/3 / http3`
  - `Security and release / security`
  - `Security and release / rehearsal`
- Dynamic/static builds, with and without built-in gzip, must verify negotiated
  sidecars, identity fallback, no-sidecar behavior, `always`, and
  `compress_vary off`.
- The FFI buffer test must prove reused output buffers clear `flush`, `sync`,
  `last_buf`, and `last_in_chain`.
- ASan/UBSan, Valgrind, and HTTP/3 sanitizer checks must contain no attributable
  failure.
- The source archive, checksum, content listing, vendored/system SBOMs, and
  toolchain manifest must be generated from the exact release commit.

The codec parameters, default buffer sizes, and supported baseline are
unchanged, so the committed v0.1.0 five-round benchmark remains the applicable
performance calibration. A fresh exact-commit HTTP/3 matrix is still required.

## Publication sequence

1. complete the exact-commit release gate;
2. create and push annotated tag `v0.1.1`;
3. publish a pre-release from `releases/v0.1.1.md`;
4. attach the complete v0.1.1 source and supply-chain evidence;
5. verify the public archive checksum and installation instructions.

Any failed required check, sanitizer finding, cache-variance regression, stale
buffer-boundary regression, or failed source-package rehearsal blocks release.
