# Project Agent Rules

## Architecture

- Use the official `nginx/ngx-rust` project as the primary NGINX integration layer.
- Keep direct `nginx-sys` access in a dedicated Rust FFI boundary for filter APIs that lack safe wrappers. Add a C shim only when a verified ABI or toolchain limitation requires one.
- Keep protocol negotiation, encoder selection, state transitions, and progress validation in Rust.
- Forbid `unsafe` in protocol and policy crates. Isolate required FFI `unsafe` in a dedicated boundary crate and document every invariant.
- Do not allow panics to cross the C ABI boundary.
- Prefer established compression libraries over reimplementing codecs.

## Correctness

- Every encoder step must report consumed input, produced output, and its next state.
- Reject a step that makes no progress unless it has completed a flush/finish operation or is explicitly waiting for input/output.
- Tests must cover empty flush/finish buffers, truncated upstream responses, backpressure, client disconnects, and Nginx reloads before production use.
- Content negotiation must honor explicit `q=0`, wildcard, identity, and duplicate coding rules.

## Delivery

- Keep the first production milestone limited to `gzip`, `deflate`, `br`, `zstd`, and `identity`.
- Treat `dcb` and `dcz` as a separate milestone because they add dictionary lifecycle, cache, and security requirements.
- Compile and test dynamic modules against every supported Nginx build signature.
