# v0.1.0 release readiness

Released on 2026-07-22 as a source-only Technical Preview. The evidence below
applies to the exact annotated `v0.1.0` tag target.

## Fixed release contract

- NGINX 1.30.4, Debian Bookworm, Linux x86_64. If the 1.30 stable branch gains
  a patch release before tagging, update the pin and rerun the entire matrix.
- Dynamic/static linking crossed with vendored/system codec backends.
- HTTP/1.1, HTTP/2, and experimental NGINX HTTP/3 without 0-RTT.
- `gzip`, `deflate`, `br`, `zstd`, and `identity`; no `dcb`/`dcz`.
- Source archive only; no general-purpose module binary and no new production
  runtime dependency.

## Implemented

- [x] Static and dynamic filter order after SSI/postpone assembly, including
      generated `objs/ngx_modules.c`, SSI, and addition assertions.
- [x] Built-in gzip metadata discovery without copying private structures.
- [x] Configuration-time warning and request-time fail-closed behavior for
      effective `gzip on` plus runtime compression, including inheritance and
      child override tests.
- [x] Payload-free panic guard and stable failure classes for FFI, allocation,
      codec, state-machine, and downstream errors.
- [x] Test-only fault injection, multi-worker concurrency, 20 graceful reloads,
      five-minute soak, disconnect, and truncated-upstream coverage.
- [x] Dynamic/static by vendored/system build matrix and filter coexistence for
      gzip, gunzip, copy, chunked, range, SSI, and addition.
- [x] Dedicated checksum-pinned HTTP/3 toolchain with `--http3-only`, dynamic/
      static matrix, slow client, concurrency, disconnect, and reload paths.
- [x] Clang ASan/UBSan and focused Valgrind harnesses with narrowly documented
      upstream suppressions.
- [x] GitHub Actions groups for Rust, NGINX integration, HTTP/3, and security/
      release; third-party actions are pinned to commit SHAs.
- [x] cargo-deny policy, CycloneDX generation, toolchain inventory, tracked-file
      source archive, content listing, and SHA-256 generation.

## Required evidence before tag

- [x] All five exact-commit checks pass:
  - `Rust / rust`
  - `NGINX integration / integration`
  - `HTTP/3 / http3`
  - `Security and release / security`
  - `Security and release / rehearsal`
- [x] ASan/UBSan, Valgrind, and HTTP/3 sanitizer logs contain no attributable
      leak, out-of-bounds access, use-after-free, or undefined behavior.
- [x] Run the five-round x86_64 HTTP/1.1/2/3 benchmark and commit its
      [raw TSV, toolchain information, and conclusion](../benchmarks/v0.1.0/http3-buffer/README.md).
      No candidate passed the documented throughput/TTFB/RSS thresholds, so
      the unified 8 KiB default is retained.
- [x] Perform a fresh-checkout rehearsal following the installation guide for
      dynamic and static builds, then run `nginx -t`, reload, and H1/H2/H3
      smoke tests.
- [x] Generate final source ZIP, content listing, SHA-256, two CycloneDX JSON
      SBOMs, and toolchain manifest from the exact release commit.
- [x] Confirm the worktree is clean and `origin/master` points to
      the exact reviewed commit.

## Publication sequence

After the final exact-commit gate succeeds:

1. make the repository public;
2. enable branch protection and required checks;
3. create the annotated `v0.1.0` tag;
4. create the GitHub Release with notes, archive, checksum, SBOMs, and toolchain
   manifest;
5. repeat the public installation smoke and verify rollback instructions.

Any static/dynamic difference, HTTP/3 fallback, gzip conflict that does not fail
closed, sanitizer finding, or failed clean install blocks release. The support
contract must not be weakened to bypass a failing gate.
