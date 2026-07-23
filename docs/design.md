# Design

This document finalizes the module design, the user-facing configuration
schema, and the local testing plan. It builds on the component boundaries and
milestones in [architecture.md](architecture.md); read that first for the
integration model and the `nginx/ngx-rust` research. M0 through M3 are now
implemented; this document records the resulting behavior as well as explicitly
deferred work.

## 1. Filter architecture

The module installs the standard NGINX two-stage compression pattern.

### Header filter

Runs once per response, before any body buffer. It:

1. Reads the request `Accept-Encoding` and parses it with the allocation-free
   negotiation model in `ngx-compress-core`.
2. Reads response `Content-Type`, `Content-Length`, and `Content-Encoding` into
   an owned snapshot.
3. Applies the shipped eligibility rules (module enabled, MIME allowlist, size
   threshold, response status, main request, and existing encoding) and selects
   a coding via the fixed server preference order. The planned v0.2.0
   `compress_proxied` work will add owned proxy/cache facts here.
4. On a hit: sets `Content-Encoding`, drops or rewrites `Content-Length`
   (streaming responses switch to chunked), adds `Vary: Accept-Encoding` when
   configured, and stores typed per-request state in the request `ctx`.
5. On a miss: leaves headers untouched and marks the `ctx` so the body filter
   short-circuits to a pass-through.

### Body filter

Runs the streaming state machine over the NGINX buffer chain:

- Each codec adapter step returns a `StepResult { consumed, produced, state }`.
- Every step is checked by `validate_progress` before its output is trusted;
  a step that consumes and produces nothing without a legitimate flush/finish
  or input/output wait is a hard error, not a silent `redo`.
- Backpressure is modeled explicitly through the saved/free/busy/output chains.
  Input not fully accepted downstream stays request-owned and is retried;
  `NGX_AGAIN` is never treated as success or as consumed data.

## 2. Crate structure

The Rust artifact is linked into one NGINX module, dynamically (`.so`) or
statically into the NGINX executable. Codecs compiled into it are selected at
runtime by configuration; there is no plugin-within-plugin loading. "On-demand"
therefore means build-time trimming,
and in Rust that is done with **Cargo features (optional dependencies)**, not
with one crate per algorithm. Splitting a codec into its own crate solves a
different problem — isolation, independent testing/versioning, and third-party
extension — and is orthogonal to on-demand compilation. A concrete win: a
disabled codec's C dependency (`libzstd`, `libbrotli`) and its binary weight
disappear entirely under `--no-default-features --features gzip,zstd`.

### Layout

```
crates/
  ngx-compress-core     negotiation + progress contract + ContentCoding
                        + the StreamingCodec trait          [forbid unsafe, no heavy deps]
  ngx-compress-codecs   the four standard adapters, per-codec Cargo features [forbid unsafe]
  ngx-compress-ffi      nginx/ngx-rust boundary, all unsafe isolated here
  ngx-compress-module   cdylib; forwards features to codecs, wires everything

later:
  ngx-compress-dict     dcb/dcz dictionary core (post-v0.1, gated) — its own crate because
                        dictionary lifecycle, cache, and security earn isolation
```

### Rationale

- **`core` stays dependency-light and separate from `codecs`.** `core` is pure
  protocol with no C dependency, so L0 property tests stay fast and hermetic;
  only `codecs` pulls in `flate2`/`brotli`/`zstd`. The `StreamingCodec` trait
  lives in `core` (core owns the contract); each adapter's step returns the
  same `StepResult` that `validate_progress` checks.
- **The four standard codecs share one `codecs` crate, gated by features**
  (`gzip`, `deflate`, `brotli`, `zstd`; `gzip`/`deflate` share `flate2`), rather
  than four near-empty crates. Features already deliver on-demand builds;
  splitting thin wrappers buys no isolation.
- **Registration is `cfg`-gated.** The module builds a codec registry keyed by
  `ContentCoding`; a disabled codec is simply not registered
  (`#[cfg(feature = …)]`), so the runtime list of available codings contains
  only what was compiled in.

### When a codec earns its own crate

1. Heavy dependencies or independent lifecycle — e.g. the gated `dcb`/`dcz`
   dictionary phase.
2. Independent versioning or third-party ecosystem — the `StreamingCodec` trait
   is the stable extension seam, so a new algorithm can be added in-tree as a
   feature, or out-of-tree as a separate crate implementing the trait and
   registered by the module. That path is supported by the seam without
   splitting each codec today.

The rule: build the trait seam now (so later splits and third-party codecs are
cheap), but do not split each codec yet (premature); on-demand is delivered by
features.

## 3. Safety and coexistence policies

These are hard rules, not defaults.

1. **Compression ownership is fail-closed.** If built-in `gzip on` and runtime
   `compress` are both effective, configuration validation warns and this
   module disables both runtime and sidecar handling for that location. A child
   `gzip off` is honored, while `compress off` plus sidecar-only handling is not
   a conflict. An existing `Content-Encoding` remains an independent second
   defense. The module never double-compresses.
2. **Unknown `Content-Length` (chunked upstream)** is compressed as a stream.
   `compress_min_length` only applies when the length is known, matching NGINX
   gzip behavior.
3. **Codec contexts are reused per worker.** On request cleanup the context is
   reset, not dropped, to avoid rebuilding a codec context per request. Codec
   state is never shared between workers or requests.
4. **No panic crosses the C ABI.** Every `extern "C"` callback has a
   non-unwinding error boundary. Logs use stable, payload-free key/value classes
   (`module`, `callback`, `class`) and never include panic payloads, URIs,
   headers, or response data.

## 4. Content negotiation and server priority

Negotiation splits into a standardized layer and a policy layer.

- **Client preference — standardized.** `Accept-Encoding` with `q` values
  (RFC 9110 §12.5.3); `br` (RFC 7932) and `zstd` (RFC 8878) are registered in
  the IANA HTTP Content Coding registry. Explicit `q=0` exclusion, the `*`
  wildcard, identity rules, and duplicate-coding resolution are already
  implemented in `ngx-compress-core::negotiation`.
- **Server tie-break order — NOT standardized.** No RFC or IANA registry ranks
  `zstd` vs `br` vs `gzip` when client `q` values tie. This is server policy.

Default order for equal client quality:

```
zstd > br > gzip > deflate > identity
```

Rationale (industry practice and the M3 benchmark work): `zstd`
compresses fast (good for dynamic responses), `br` reaches smaller sizes at
high quality (good for cacheable/static), `gzip`/`deflate` are the
compatibility floor, `identity` is the implicit fallback.

The v0.1 order is fixed. Only codecs that are enabled and pass eligibility
participate. `identity` is always an implicit final candidate and is never
listed. `compress_priority` is not registered in v0.1.0, but is scheduled as the
second post-v0.1 phase; see [roadmap.md](roadmap.md).

## 5. Configuration schema

> The normative operator reference — every directive's syntax, default, context,
> and range — is [directives.md](directives.md). This section records the design
> rationale (naming, upstream analogs, calibration, precedence) behind those
> values; the tables below carry that rationale rather than serving as the
> place operators must read.

Directive namespace is `compress_*`. All directives are valid in `http`,
`server`, and `location` contexts and inherit with standard NGINX
`merge_loc_conf` cascade (child overrides parent).

### 4.1 Naming: our own scheme, not an upstream drop-in

A literal drop-in of the upstream directive names is impossible: `gzip`,
`brotli`, and `zstd` are already registered by `ngx_http_gzip_module`,
`google/ngx_brotli`, and `tokers/zstd-nginx-module`, and two modules defining
the same directive is a configuration-time error. Rather than chase near-miss
name compatibility, the schema is designed as one tidy `compress_*` family with
our own consistent semantics (read-as-you-see). Two ideas are worth borrowing
because all three upstream modules converged on them — a MIME allowlist
(`compress_types`) and output buffer sizing (`compress_buffers`) — so those are
provided under our naming. `gzip_proxied` has a codec-independent policy
meaning, so v0.2.0 will expose it as `compress_proxied`. `gzip_http_version`
remains an unscheduled candidate and a `gzip_disable`-style legacy user-agent
regex is not currently planned.

### 4.2 Master switch, profiles, and per-codec toggles

The `compress` directive is both the master gate and the profile selector:

| `compress` value | Meaning |
| --- | --- |
| `off` | module disabled (default) |
| `on` | enabled, *custom* mode — no preset; only explicit `compress_*` directives and built-in defaults apply (backward compatible with pre-profile configs) |
| `fast` \| `balanced` \| `max` | enabled with a named preset (see §4.2.1) |

So the minimal turnkey config is a single line — `compress balanced;` — which
enables the compiled-in codecs at that tier and sets a sensible `min_length`.
The manual path is `compress on;` plus at least one per-codec toggle below.

| Directive | Type | Default | Upstream analog |
| --- | --- | --- | --- |
| `compress off\|on\|fast\|balanced\|max` | enum | `off` | `gzip` (master gate) + preset |
| `compress_gzip on\|off` | bool | `off` | `gzip` |
| `compress_deflate on\|off` | bool | `off` | — (raw deflate) |
| `compress_brotli on\|off` | bool | `off` | `brotli` |
| `compress_zstd on\|off` | bool | `off` | `zstd` |

### 4.2.1 Profiles (`compress <tier>`)

A profile is a preset bundle so a user need not learn each directive's meaning
and recommended value. No upstream module offers this; it is a UX layer over the
already-validated per-codec knobs, adding no request-path logic. Folding it into
`compress` (rather than a separate `compress_profile`) follows the common nginx
pattern of a directive that takes more than `on`/`off`, and lets `compress on`
naturally mean the "custom" (no-preset) mode.

| Tier | Enables | Intent | Levels (calibrated via `bench/` on a real web corpus) |
| --- | --- | --- | --- |
| `fast` | gzip, br, zstd | high-QPS dynamic, CPU-frugal | gzip 4 / br 4 w18 / zstd 3, min_length 256 |
| `balanced` | gzip, br, zstd | general default | gzip 6 / br 5 w22 / zstd 6, min_length 256 |
| `max` | gzip, br, zstd | cacheable/precompressed, CPU offline | gzip 9 / br 11 w24 / zstd 19, min_length 128 |

Calibration basis (HTML/CSS/JS/JSON, x86-64): brotli has a sharp speed cliff at
q4→q5 (q5 buys ~1–2% ratio for ~40–60% throughput) and again at q9→q10 (q10/q11
drop to 1–6 MB/s — offline only); gzip ratio is converged by L6 (L7–9 add ≈0);
zstd stays fast through L6 (>130 MB/s) but L19 is offline-only. So `fast` stops
before each knee, `balanced` takes the first (still online-fast), `max` uses the
ceilings.

- **Precedence:** explicit `compress_*` directive > profile preset > built-in
  default, independent of directive order. `compress max; compress_zstd off;`
  runs the `max` tier with zstd disabled.
- **Codec availability:** a preset only enables codecs compiled into the build;
  `deflate` is never enabled by a preset (clients rarely request raw deflate) —
  turn it on explicitly if needed.
- **Stability:** tier *names* express intent, so re-tuning a tier's numbers after
  later evidence is not a breaking change. The table records the v0.1.0 values.

### 4.3 Per-codec parameters

Compression level scales differ per codec, so each has its own directive with
the upstream range and default.

| Directive | Range | Default | Upstream analog |
| --- | --- | --- | --- |
| `compress_gzip_comp_level` | 1–9 | 6 | `gzip_comp_level` |
| `compress_deflate_comp_level` | 1–9 | 6 | — |
| `compress_brotli_comp_level` | 0–11 | 6 | `brotli_comp_level` |
| `compress_brotli_window` | 1k–16m | 512k | `brotli_window` |
| `compress_zstd_comp_level` | 1–22 (neg. fast levels allowed) | 3 | `zstd_comp_level` |

Levels are validated at configuration time against the codec's range; an
out-of-range value is a configuration error, not a clamp.

### 4.4 Shared parameters

These shipped directives apply to all enabled codecs.

| Directive | Type | Default | Notes |
| --- | --- | --- | --- |
| `compress_types <mime>...` | set | text/html (always), text/\*, application/json, application/javascript, … | `*` = all types |
| `compress_min_length <n>` | size | 20 | only when Content-Length known; 256+ recommended |
| `compress_vary on\|off` | bool | `on` | adds `Vary: Accept-Encoding`; on by default because a multi-codec module serving different encodings must mark shared-cache variance |
| `compress_buffers <n> <size>` | (count, size) | `16 8k` | per-request output buffer pool |

Per-codec MIME, minimum-length, and buffer overrides are not registered. The
per-codec controls in v0.1 are enablement and compression level, plus the
Brotli window.

#### 4.4.1 Planned v0.2.0 proxied policy

`compress_proxied <flags>...` will mirror the policy vocabulary and default of
`gzip_proxied`: `off`, `expired`, `no-cache`, `no-store`, `private`,
`no_last_modified`, `no_etag`, `auth`, and `any`. It will apply to all runtime
codings and to `compress_static on`; `compress_static always` will bypass it.
The directive is planned and is not accepted by v0.1.0.

### 4.5 Typed configuration model

Directives are parsed at configuration time into a strongly typed,
Rust-owned `CompressConfig`. Its optional fields preserve NGINX inheritance;
resolution produces an allocation-free `Resolved` snapshot containing the
effective shared policy, enabled codec levels, Brotli window, sidecar mode, and
output-buffer size.

`serde_json::Value`-style dynamic structures are confined to any external
boundary; business logic operates only on the typed model. Merge and
validation happen once at configuration load, never on the request path.

### 4.6 Example

```nginx
compress on;
compress_zstd on;
compress_brotli on;
compress_gzip on;

compress_zstd_comp_level   9;
compress_brotli_comp_level 5;
compress_gzip_comp_level   6;

compress_types      text/plain text/css application/json application/javascript;
compress_min_length 256;
compress_vary       on;
```

Dictionary directives (`dcb`/`dcz`) are deliberately out of scope here and
will be specified after the post-v0.1 design study. The current single-directive
and lazy per-location lifecycle direction is recorded in
[dictionary-transport.md](dictionary-transport.md).

### 4.7 Precompressed static (`compress_static`)

A content-phase handler (in the spirit of `gzip_static`) that serves a
precompressed sidecar file when one exists next to the requested file, instead
of compressing at runtime.

| `compress_static` | Meaning |
| --- | --- |
| `off` (default) | never serve sidecars |
| `on` | serve a sidecar only when the client accepts that coding |
| `always` | serve the highest-priority existing sidecar even without a matching `Accept-Encoding` (e.g. behind a decompressing proxy) |

- For `GET`/`HEAD`, it probes `<file>.zst`, `<file>.br`, `<file>.gz` in
  server-priority order (zstd > br > gzip), among codings the client accepts,
  and serves the first that exists with the matching `Content-Encoding`,
  `Last-Modified`, and `ETag`. If none is usable it declines and the normal
  static handler serves the original.
- It is independent of the runtime `compress` switch: `compress off;
  compress_static on;` serves sidecars with no runtime compression. When both
  are on, serving a sidecar sets `Content-Encoding` before the body filters, so
  the runtime compressor skips the response (no double compression).
- deflate has no conventional sidecar extension and is not probed.
- The handler must run before nginx's built-in static handler to intercept a
  request whose original file exists. This holds in both the dynamic and static
  builds (verified end-to-end), because the module's `ngx_module_order` places it
  early enough in the content phase — unlike the subrequest *filter*-position
  caveat in §8, which only the dynamic build resolves.

## 6. Local testing plan

Pure-Rust layers stay hermetic and fast; NGINX-dependent layers are gated so
`cargo test --workspace` never needs an NGINX build.

### L0 — Protocol core (now, plain `cargo`)

Unit and property tests (`proptest`) cover negotiation and progress invariants:

- parser never panics on arbitrary input; duplicate codings keep the highest
  `q`; `q` values stay monotonic and in range.
- `validate_progress` never rejects a step that a real codec could legitimately
  produce, and always rejects a no-progress stall.

### L1 — Codec adapters (M2, plain Rust)

Per codec:

- round-trip: compress then decompress equals the input.
- tiny output/input buffers to force `NeedsOutput` / `NeedsInput` transitions.
- empty input, flush, and finish boundaries.
- truncated input.
- every emitted step fed through `validate_progress` (property: real codecs
  never violate the contract).

### L2 — NGINX integration (M1+, needs an NGINX source tree)

Harness: **Test::Nginx** (the official Perl framework, matching
`nginx/nginx-acme`) plus shell-driven Docker integration and edge suites against
a pinned NGINX source tree.

Current coverage includes compression round-trips, dynamic/static link modes,
SSI/addition order, gzip conflict inheritance, filter coexistence, backpressure,
client disconnect, truncated/chunked upstream responses, repeated reloads, and
HTTP/1.1, HTTP/2, and HTTP/3 interoperability.

### Docker build-and-integration environment (static + dynamic)

A pinned Docker image (Debian slim + fixed NGINX source tree + Rust toolchain +
`libzstd`/`libbrotli` headers) validates **both link modes**, because NGINX
filter order differs between them:

1. **Dynamic** — `configure --add-dynamic-module=…` → `ngx_http_compress_module.so`
   → `load_module` → smoke test.
2. **Static** — `configure --add-module=…` compiled into the NGINX binary →
   smoke test.
3. **Smoke** — `curl -H 'Accept-Encoding: zstd,br,gzip'`, assert the correct
   `Content-Encoding`, decode the body with the system `zstd`/`brotli`/`gzip`
   CLI back to the original, and assert `Vary` is present.
4. **Edge tests** — exercise filter order, backpressure, client disconnect,
   truncated/chunked upstream responses, reload, HTTP/2, and HTTP/3. Both link
   modes run ordinary compression, sidecar, SSI, and addition validation.

### L3 — Fuzzing and conformance (ongoing)

- `cargo-fuzz` on the `Accept-Encoding` parser and the streaming state machine.
- Conformance: module output is decoded with the standard `gzip`/`brotli`/`zstd`
  CLIs and compared byte-for-byte with the original response.

### Prerequisite note

Executing the Docker static/dynamic integration test requires a minimal M1
identity pass-through module to exist first — there is nothing to link until
then. That bootstrap is the first M1 task, per architecture.md ("the M1
identity filter must prove chain ownership and backpressure before any codec is
integrated").

## 7. Build backends and performance

Two orthogonal, build-time dimensions, kept flat to avoid a per-codec matrix:

1. **Which codings** — the `gzip` / `deflate` / `brotli` / `zstd` features
   (identity always present).
2. **Backend** — an all-or-nothing switch, selected in the NGINX build via
   `NGX_COMPRESS_BACKEND`:
   - `vendored` (default): codecs are self-compiled and statically embedded.
     flate2 uses `zlib-ng` (SIMD/vectorized); brotli is pure Rust with
     `vector_scratch_space` (its `simd` feature needs nightly); zstd uses its
     optimized C library. Enforced by a `compile_error!` against enabling both
     backends.
   - `system-libs`: flate2 links the distro's shared `libz`, zstd links shared
     `libzstd` (pkg-config), and `br` links shared `libbrotlienc` through a
     dedicated `ngx-compress-brotli-sys` boundary crate. That crate only
     *declares and calls* the libbrotli C encoder API (it neither compiles C nor
     exports symbols — unlike the pure-Rust `brotli` crate's `ffi-api`, whose
     `no_mangle` exports would risk symbol collisions in the NGINX process). The
     module `config` adds `-lz -lzstd -lbrotlienc -lbrotlicommon` to
     `ngx_module_libs` so the staticlib→module flow yields a `.so` with `NEEDED`
     entries for the shared objects (cargo's `-sys` link directives are otherwise
     dropped when building a staticlib). The pure-Rust brotli codec is used for
     `vendored` and the FFI adapter for `system-libs`, cfg-switched behind the
     same `Brotli` name so callers are unaffected. `docker/verify-backends.sh`
     builds both backends and checks `ldd` plus end-to-end compression.

The release profile is `lto = "fat"`, `opt-level = 3`, `codegen-units = 1`,
`strip = "symbols"`. The NGINX flow builds through `ngx-release` (inherits
`release`), so lto/opt-level/codegen-units apply there too; `strip` only affects
standalone cargo builds. Note `zstd`'s `fat-lto` feature is deliberately off: it
emits LTO-bitcode C objects the NGINX linker cannot resolve from our staticlib.

Threaded/async compression is not enabled in v0.1.0. It is now a numbered
post-v0.1 task after the bounded, resumable work contract. The goal is to move
eligible expensive work off the NGINX event loop with Rust-owned bounded input
and event-loop completion; merely enabling a codec's internal multithreading
does not satisfy the ownership or lifecycle contract.

Pinned dependency majors are current as of this milestone: `ngx` 0.5, `flate2`
1, `brotli` 8, `zstd` 0.13.

## 8. Post-v0.1 design sequence and deferred work

The completed M0-M3 foundation includes HTTP/3 coverage, static/dynamic filter
ordering, sidecar serving, named profiles, worker-local codec reuse, and the
ratio/throughput calibration harness. These are maintained by the release gates
rather than carried as open planning items.

The canonical post-v0.1 sequence is maintained in
[the development plan](roadmap.md):

1. remove the `max` profile without another keep/retune experiment, then measure
   the remaining runtime profiles and add a bounded, resumable safe-core work
   contract;
2. add inherited `compress_priority` as the server tie-break among codings with
   equal effective client quality, consistently for runtime and static paths;
3. design and, if the ownership/lifecycle gate passes, implement asynchronous
   compression with no cross-thread NGINX pointers;
4. add `compress_proxied` using Rust-owned `Via`, authorization, expiry,
   Cache-Control, Last-Modified, and ETag facts;
5. complete the dictionary-provisioning design study, then implement RFC 9842
   `dcb`/`dcz` as one production milestone rather than a static-only prototype.

Build-signature and deployment validation continues in parallel. New external
reports may alter support targets, but a lack of repository traffic does not
block this engineering sequence.

- HTTP/3 is included in v0.1.0 through a dedicated pinned NGINX/OpenSSL/
  ngtcp2/nghttp3/curl image. Every request uses `--http3-only`; fallback is a
  test failure. Upstream NGINX HTTP/3 remains experimental and 0-RTT is out of
  scope.
- Per-response-class automatic priority (for example, a different implicit
  order for dynamic versus cacheable responses) remains deferred.
  `compress_priority` itself is scheduled as a near-term explicit server
  tie-break and must preserve client `q` semantics.
- A runtime cache of the module's *own* compressed output — explicitly **not
  built**. Static content is served from precompressed sidecars (the filesystem
  is the cache); "compress once, reuse" for dynamic content is delegated to
  nginx `proxy_cache` at an origin that emits `Content-Encoding`. A bespoke
  shared-memory compressed cache would re-implement `proxy_cache` for a narrow
  gain and is not worth the invalidation/eviction complexity.
- Per-MIME quality / per-size algorithm switching — **dropped**. No upstream
  module offers it; the table-stakes controls (`compress_types` allowlist,
  `compress_min_length`, per-codec `*_comp_level`) already ship and match the
  ecosystem.
- Dictionary transport is gated by a design study of provisioning, versioning,
  cache, and security, after which the planned deliverable is complete RFC 9842
  support. The settled configuration direction is one inherited
  `compress_dictionary off|lazy|<file>` directive; lazy mode automatically
  manages independent per-origin/per-location dictionary generations. See
  [dictionary-transport.md](dictionary-transport.md).
- Symmetric decode ("unboxing") remains a separate design decision. This module
  is compress-only; enabling it does not make NGINX decompress anything (the
  client transparently decodes the response). A decode capability would be a
  gunzip-equivalent that also covers `br`/`zstd` (the built-in
  `ngx_http_gunzip_module` only handles gzip) and/or request-body decoding.
  Decision so far: build it **inside this repo/workspace**, not a separate repo —
  it reuses ~70–80% of the foundation (the direction-agnostic `validate_progress`
  contract; the same flate2/brotli/zstd crates, which also provide decoders; the
  FFI filter-chain and free/busy backpressure; the vendored/system-libs
  backends; the docker/lint harness). Likely shape: a feature-gated sibling
  module crate (e.g. `ngx_http_decompress_module`) with a `StreamingDecoder`
  trait mirroring `StreamingCodec`. Requires its own design pass because decode
  has a distinct security surface — bounded output/work budgets are mandatory
  (decompression bombs) — and the filter position/scope differ by target
  (upstream response vs request body). It cannot start before the bounded-work
  primitives exist and the target direction is selected explicitly.
