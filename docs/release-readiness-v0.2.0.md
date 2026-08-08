# v0.2.0 release readiness

This release changes negotiation, streaming scheduling, ARM64 buffer handling,
and public configuration. It remains a source-only Technical Preview.

## Fixed release contract

- NGINX 1.30.4 on Ubuntu 24.04, Linux x86_64 and ARM64.
- Dynamic/static linking crossed with vendored/system codec backends.
- HTTP/1.1, HTTP/2, and experimental NGINX HTTP/3 without 0-RTT.
- `gzip`, `deflate`, `br`, `zstd`, and `identity`; no `dcb`/`dcz`.
- Source archive only; no universal module binary or asynchronous compression.

## Required evidence before tag

- The exact release commit must be pushed to `origin/master`.
- All exact-commit Rust, NGINX integration, HTTP/3, security, and release
  rehearsal checks must pass.
- x86_64 and ARM64 must decode dynamic gzip, Brotli, and Zstandard responses
  and compare their bytes or hashes with the source payload for file, memory,
  and mixed-buffer chains.
- Negotiation tests must cover explicit/wildcard/identity quality, `q=0`,
  duplicates, missing and empty fields, `406`, profile completion, inheritance,
  and dynamic/static consistency.
- Streaming tests must cover budget recovery, empty flush/finish, truncated
  input, slow downstreams, output exhaustion, disconnects, and reloads.
- ASan/UBSan, applicable Valgrind, and HTTP/3 sanitizer checks must contain no
  attributable failure.
- An isolated `oc.ams` canary must match the production NGINX build signature
  and verify identity/gzip/br/zstd, conditional requests, HEAD, concurrency,
  and callback p99 without replacing the production module.
- The source archive, checksum, content listing, vendored/system SBOMs, and
  toolchain manifest must be generated from the exact release commit.

## Performance gate

- `fast` and `balanced` target p99 codec callback work of at most 1 ms on the
  shared x86_64, GitHub ARM64, and `oc.ams` budget.
- Explicit custom high-compression levels are required to remain bounded but do
  not carry the 1 ms target.

## Publication sequence

1. complete the exact-commit release gate;
2. obtain explicit authorization to push, tag, and publish;
3. create and push annotated tag `v0.2.0`;
4. publish a prerelease from `releases/v0.2.0.md`;
5. attach and verify the complete v0.2.0 source and supply-chain evidence.

Any failed required check, sanitizer finding, undecodable body, negotiation
regression, lost streaming state, or failed source-package rehearsal blocks
release.
