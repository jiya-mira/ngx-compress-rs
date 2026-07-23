# Architecture

## Component boundaries

```text
Nginx worker
  -> NGINX static registration or dynamic module ABI
  -> nginx/ngx-rust bindings and Rust FFI boundary
  -> negotiation and policy
  -> typed streaming state machine
  -> gzip / deflate / brotli / zstd codec adapters
  -> Nginx output chain
```

The module is built through NGINX's native static or dynamic module flow. The
default integration path uses the official [`nginx/ngx-rust`](https://github.com/nginx/ngx-rust)
project (`ngx` and `nginx-sys`) instead of maintaining a parallel hand-written
C adapter. A C shim remains a fallback only if a verified ABI or toolchain
limitation requires one.

The Rust boundary registers directives and header/body filters and converts raw NGINX pointers into request-scoped views. Required `unsafe` code belongs only in this boundary and must validate lengths, nullability, ownership, and lifetime assumptions before constructing slices. No panic may unwind across an `extern "C"` callback.

The protocol core is independent of Nginx. It parses content negotiation, selects an eligible codec, and validates progress after every streaming operation.

## Encoding policy

Selection first honors client quality values and explicit exclusions. Server policy then breaks ties using request properties such as MIME type, body length, cacheability, and available dictionaries.

Initial default order for equal client quality:

1. `zstd` for dynamic responses
2. `br` for cacheable or precompressed responses
3. `gzip`
4. `deflate`
5. `identity`

The v0.1 tie-break order is fixed. An inherited `compress_priority` directive is
the second post-v0.1 phase. Adaptive response-class selection remains deferred
and requires benchmark evidence before it becomes a release commitment.

## Streaming contract

Every codec adapter returns a structured result:

- input bytes consumed
- output bytes produced
- next state: needs input, needs output, or complete

A step with available input and output capacity that consumes and produces nothing is an error. Flush and finish operations may complete without producing bytes, but must report completion explicitly.

This contract prevents the unbounded `redo` loops seen in older third-party Nginx compression filters.

## Memory and concurrency

- Borrow Nginx input buffers instead of copying them into intermediate `Vec` values.
- Write into request-owned output buffers supplied by the adapter.
- Reuse codec contexts within a worker after request cleanup.
- Do not share mutable codec state between workers or requests.
- Avoid locks on the request path; Nginx workers are event-loop processes.
- Add bounded work and output budgets before enabling high compression levels.

## Rust and NGINX integration research

### Projects considered

| Project | Maturity and relevance | Integration approach | Decision |
| --- | --- | --- | --- |
| [`nginx/ngx-rust`](https://github.com/nginx/ngx-rust) | Official NGINX project, actively developed and used by NGINX products. Its API is explicitly not stable yet. | Generates bindings from the configured NGINX source with `bindgen`; provides module/configuration, request, buffer, pool, allocator, logging, and event-loop APIs; exports dynamic-module symbols from Rust. | Use as the primary SDK and pin an audited release or commit. |
| [`nginx/nginx-acme`](https://github.com/nginx/nginx-acme) | Active production-grade module maintained by NGINX. It is not a body filter, but is the strongest public reference for build, packaging, lifecycle, async work, and integration tests. | Pure Rust module built through NGINX's `--add-dynamic-module` flow and the `ngx-rust` build helpers. Uses NGINX pools and cleanup-aware Rust allocation. | Copy its build matrix, module layout, linting, sanitizer, and NGINX test-harness patterns. |
| [Cloudflare ROFL](https://blog.cloudflare.com/rust-nginx-module/) | A Rust response body filter reported by Cloudflare to run on millions of responses per second. Its implementation is not public. | Pure Rust dynamic module with generated bindings, request `ctx`, NGINX pool cleanup, explicit filter ordering, and saved/free/busy/output chains for backpressure. | Treat as production evidence and a design checklist, not a code dependency. |
| [`dcoles/nginx-rs`](https://github.com/dcoles/nginx-rs) | Early experimental SDK; inactive and its README redirects users to official `ngx`. It influenced Cloudflare's buffer and pool handling. | Pure Rust symbols plus generated bindings; cleanup callbacks drop Rust values stored in NGINX pools. | Historical reference only. Do not depend on it. |
| [`arvancloud/nginx-rs`](https://github.com/arvancloud/nginx-rs) | Older low-level bindings, tied by default to NGINX 1.19.3 and inactive since 2022. | Downloads or accepts an NGINX source tree and generates bindings with `bindgen`. | Historical reference only. Do not depend on it. |
| [`ngx-strict-sni`](https://github.com/JyJyJcr/ngx-strict-sni) | Useful external example, but it depends on a fork/branch of `ngx-rust` rather than a released official SDK. It is not a body filter. | Rust request/module integration using NGINX pools. | Use only to identify missing SDK ergonomics; do not copy its dependency strategy. |

No mature, reusable, public Rust compression body-filter implementation was found. The most relevant body-filter implementation, Cloudflare ROFL, is described publicly but its source is not available. The project therefore cannot outsource the filter state machine to an existing Rust module.

### Adopted integration model

1. Build a Rust `cdylib` through NGINX's native `config` and `config.make` flow with `--add-dynamic-module`.
2. Generate bindings against the exact NGINX source tree and configure arguments used for the target binary. NGINX exposes conditionally compiled structures directly to modules, so `--with-compat` is not a substitute for testing the exact production build.
3. Use `ngx-rust` wrappers where they preserve NGINX semantics; use `nginx-sys` only inside a narrow filter boundary for APIs that have no high-level wrapper.
4. Store non-trivial per-request Rust state through the request `ctx` and register pool cleanup so codec destructors run. Raw pool allocation alone is insufficient for Rust values that own resources.
5. Register header and body filters during post-configuration, retain the next
   filter pointers, and test the module's position relative to gzip, gunzip,
   copy, chunked, range, SSI, and addition. Dynamic builds declare
   `ngx_module_order`; static builds reorder `HTTP_FILTER_MODULES` after module
   registration so both paths run after SSI/postpone assembly. NGINX documents
   that [dynamic-module order is significant for filters](https://blog.nginx.org/blog/nginx-dynamic-modules-how-they-work).
6. Model NGINX backpressure explicitly. Input buffers not fully accepted downstream remain request-owned and are retried through saved/free/busy/output chains; `NGX_AGAIN` must never be treated as success or data consumption.
7. Put a non-unwinding error boundary around every callback exported to C. Invalid state, allocation failure, or codec failure must map to a documented NGINX status and an error log with request context.

### Gaps to prove before codec work

- `ngx-rust` has useful buffer and pool primitives but no public high-level body-filter abstraction. The M1 identity filter must prove chain ownership and backpressure before any codec is integrated.
- The SDK declares breaking API changes possible. Pinning, dependency update policy, and a small compatibility layer owned by this project are required.
- Dynamic module compatibility depends on NGINX version, configure flags, compiler/ABI, and distribution patches. Release artifacts need an explicit compatibility matrix; a single universal `.so` is not a goal.
- Filter order differs between static and dynamic registration. v0.1.0 supports
  both only because generated `objs/ngx_modules.c` order and SSI/addition output
  are checked directly in the release matrix.

## v0.1.0 support boundary

- Source-only Technical Preview; no universal `.so`.
- NGINX 1.30.4, Debian Bookworm, Linux x86_64.
- Dynamic/static linking crossed with vendored/system codec backends.
- HTTP/1.1, HTTP/2, and ordinary HTTP/3; NGINX HTTP/3 remains experimental and
  0-RTT is excluded.
- `gzip`, `deflate`, `br`, `zstd`, and `identity`; dictionary transports remain
  outside the v0.1.0 support boundary.

Built-in `gzip on` plus effective runtime compression is handled fail-closed.
The FFI boundary discovers the public module/command metadata and copies only a
typed `BuiltinGzipState`; no private gzip configuration structure is mirrored.
Configuration scanning emits warnings, and request-time validation disables
runtime and sidecar paths if the effective state conflicts.

## Project naming

The public project and repository name is **`ngx-compress-rs`**. It balances NGINX discoverability, purpose, and Rust identity.

Regardless of repository name, keep runtime names language-neutral: use an NGINX module symbol such as `ngx_http_compress_module` and a consistent `compress_*` directive namespace. This avoids forcing configuration changes if the implementation language or repository branding changes later.

## Milestones

M0-M3 are the delivered v0.1.0 foundation. Post-v0.1 work is ordered by the
technical dependencies below; external issue or discussion activity is evidence,
not the planning authority.

### M0: Protocol core

- `Accept-Encoding` parser and selection policy
- progress invariant model
- unit and property tests

### M1: Nginx filter foundation

- official `ngx-rust` integration pinned to an audited revision
- generated bindings for the exact target NGINX build
- Rust FFI boundary
- identity/pass-through filter
- filter-order, backpressure, reload, and lifecycle tests

### M2: Standard codecs

- gzip and deflate
- Brotli
- Zstandard
- streaming, backpressure, truncation, and disconnect tests
- HTTP/1.1, HTTP/2, and HTTP/3 interoperability tests

### M3: Static and profile optimization

- precompressed static variants (`.gz`/`.br`/`.zst` sidecar content handler)
- named profiles (`compress fast|balanced|max`) — turnkey presets over the
  per-codec knobs, explicit directives override
- worker-local context reuse (reset a per-worker codec instead of reallocating)
- benchmark-driven compression profiles (calibrate the preset tiers and fixed
  default priority order)

Dropped from the original M3 list: per-MIME/per-size *policy* beyond the shipped
`compress_types` + `compress_min_length` + per-codec level — no upstream module
has it and the gain does not justify the config surface. A runtime cache of the
module's own compressed output is a non-goal (precompressed sidecars + upstream
`proxy_cache` cover it).

### Post-v0.1 phase 1: remove `max` and bound event-loop work

- remove the `max` profile from the active configuration surface without
  spending another round deciding whether to retain or retune it;
- measure callback latency, codec iterations, throughput, and worker RSS for the
  remaining profiles and representative explicit levels;
- add a resumable safe-core work budget without losing input, flush/finish
  state, or NGINX chain ownership.

### Post-v0.1 phase 2: configurable server priority

- add inherited `compress_priority` for equal-client-quality tie-breaking;
- keep client `q=0`, wildcard, identity, and duplicate-coding semantics
  authoritative;
- apply the configured order consistently to runtime codecs and precompressed
  representations, with an extension path for `dcb` and `dcz`.

### Post-v0.1 phase 3: asynchronous execution

- move eligible expensive codec work off the NGINX event loop;
- hand threads only bounded Rust-owned data and return completion to the owning
  event loop before touching request or chain state;
- bound queue depth and copied bytes, and verify disconnect, cancellation,
  graceful reload, backpressure, and H1/H2/H3 ordering.

### Post-v0.1 phase 4: proxied-response policy

- add `compress_proxied` with the complete `gzip_proxied` flag vocabulary and
  NGINX-compatible `Via` semantics;
- apply one safe-core eligibility decision to every runtime codec and to
  `compress_static on`, while `compress_static always` bypasses it;
- prefetch only owned header facts at the FFI boundary; do not call the
  gzip-specific `ngx_http_gzip_ok()` helper;
- verify inheritance, cache-header combinations, static/runtime parity, and
  HTTP/1.1, HTTP/2, and HTTP/3 behavior.

The detailed engineering sequence and its exit gates are maintained in
[the post-v0.1 development plan](roadmap.md).

### Post-v0.1 phase 5: complete Compression Dictionary Transport

- first complete a design study covering dedicated dictionaries, previous-build
  artifacts, generated manifests, multiple active versions, storage, rollout,
  rollback, and the boundary between runtime and offline tooling;
- then implement RFC 9842 advertisement, selection, `dcb` and `dcz`, hash
  validation, cache isolation, correct `Vary`, fallback, and origin/privacy
  protections as one production milestone;
- include offline dictionary generation/evaluation and manifest tooling so
  protocol support is operationally usable without handwritten per-resource
  mappings;
- do not publish a static-only prototype as a separate product milestone.

## Explicit non-goals for v0.1.0

- implementing compression algorithms from scratch
- asynchronous thread-pool compression
- externally supplied compression dictionaries
- automatic dictionary training
- opaque retries after encoder failure
