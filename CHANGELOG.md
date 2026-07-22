# Changelog

All notable changes are recorded here. The project follows Semantic Versioning
for its public configuration and documented support contract.

## [Unreleased]

## [0.1.0] - 2026-07-22

### Added

- Source-only Technical Preview for NGINX 1.30.4 on Debian Bookworm/Linux
  x86_64.
- Runtime gzip, deflate, Brotli, Zstandard, and identity negotiation.
- Precompressed `.gz`, `.br`, and `.zst` sidecar handling.
- Dynamic/static and vendored/system build support.
- HTTP/1.1, HTTP/2, and experimental HTTP/3 coverage.
- Named compression profiles, explicit codec controls, worker-local encoder
  reuse, and streaming progress validation.
- Fail-closed coexistence with built-in `gzip on` and configuration warnings.
- Sanitizer, Valgrind, lifecycle, reload, fault-injection, supply-chain, SBOM,
  and source-package release gates.

### Security

- Isolated raw NGINX access in the FFI boundary and moved request policy,
  negotiation, state transitions, and submit planning into safe Rust.
- Added payload-free panic guards so Rust unwinding cannot cross C callbacks.

[Unreleased]: https://github.com/jiya-mira/ngx-compress-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jiya-mira/ngx-compress-rs/releases/tag/v0.1.0
