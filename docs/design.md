# Design

This document finalizes the module design, the user-facing configuration
schema, and the local testing plan. It builds on the component boundaries and
milestones in [architecture.md](architecture.md); read that first for the
integration model and the `nginx/ngx-rust` research. Nothing here is
implemented yet — this is the agreed specification for M1 onward.

## 1. Filter architecture

The module installs the standard NGINX two-stage compression pattern.

### Header filter

Runs once per response, before any body buffer. It:

1. Reads the request `Accept-Encoding` and parses it with the allocation-free
   negotiation model in `ngx-compress-core`.
2. Reads response `Content-Type`, `Content-Length`, `Content-Encoding`, and
   `Cache-Control`.
3. Applies eligibility (module enabled, MIME allowlist, size threshold, HTTP
   version, proxied policy) and selects a coding via server preference order.
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

The final artifact is a single NGINX dynamic module (`.so`, one `cdylib`).
Codecs compiled into it are selected at runtime by configuration; there is no
plugin-within-plugin loading. "On-demand" therefore means build-time trimming,
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
  ngx-compress-dict     dcb/dcz dictionary core (M4) — its own crate because
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

1. Heavy dependencies or independent lifecycle — e.g. `dcb`/`dcz` in M4.
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

1. **The module owns response compression.** Documentation requires
   `gzip off;`. As defense in depth, the header filter skips any response that
   already carries `Content-Encoding` (upstream already compressed, or the
   built-in gzip filter ran first). The module never double-compresses.
2. **Unknown `Content-Length` (chunked upstream)** is compressed as a stream.
   `compress_min_length` only applies when the length is known, matching NGINX
   gzip behavior.
3. **Codec contexts are reused per worker.** On request cleanup the context is
   reset, not dropped, to avoid rebuilding a codec context per request. Codec
   state is never shared between workers or requests.
4. **No panic crosses the C ABI.** Every `extern "C"` callback has a
   non-unwinding error boundary that maps invalid state, allocation failure, or
   codec failure to a documented NGINX status plus an error log with request
   context.

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

Rationale (industry practice, refined per response class in M3): `zstd`
compresses fast (good for dynamic responses), `br` reaches smaller sizes at
high quality (good for cacheable/static), `gzip`/`deflate` are the
compatibility floor, `identity` is the implicit fallback.

The order is overridable:

```nginx
compress_priority zstd br gzip;
```

Only codecs that are enabled and pass eligibility participate. `identity` is
always an implicit final candidate and is never listed.

## 5. Configuration schema

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
provided under our naming. Directives with no elegant home in our scheme
(`gzip_proxied`, `gzip_http_version`, `gzip_disable`) are intentionally not
mirrored.

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

| Tier | Enables | Intent | Levels (placeholder, benchmark-calibrated) |
| --- | --- | --- | --- |
| `fast` | gzip, br, zstd | high-QPS dynamic, CPU-frugal | gzip 4 / br 4 w18 / zstd 1, min_length 256 |
| `balanced` | gzip, br, zstd | general default | gzip 6 / br 6 w22 / zstd 3, min_length 256 |
| `max` | gzip, br, zstd | cacheable/precompressed, CPU offline | gzip 9 / br 11 w24 / zstd 19, min_length 128 |

- **Precedence:** explicit `compress_*` directive > profile preset > built-in
  default, independent of directive order. `compress max; compress_zstd off;`
  runs the `max` tier with zstd disabled.
- **Codec availability:** a preset only enables codecs compiled into the build;
  `deflate` is never enabled by a preset (clients rarely request raw deflate) —
  turn it on explicitly if needed.
- **Stability:** tier *names* express intent, so re-tuning a tier's numbers after
  benchmarking is not a breaking change; the concrete values are placeholders
  until the M3 benchmark calibrates them.

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

### 4.4 Shared parameters (global default, per-codec override)

These apply to all codecs by default and may be overridden per codec with the
`compress_<codec>_*` form (e.g. `compress_brotli_types`). A per-codec value
fully replaces the shared value for that codec.

| Directive | Type | Default | Notes |
| --- | --- | --- | --- |
| `compress_types <mime>...` | set | text/html (always), text/\*, application/json, application/javascript, … | `*` = all types |
| `compress_min_length <n>` | size | 20 | only when Content-Length known; 256+ recommended |
| `compress_vary on\|off` | bool | `on` | adds `Vary: Accept-Encoding`; on by default because a multi-codec module serving different encodings must mark shared-cache variance |
| `compress_proxied <flags>...` | flags | `off` | mirrors `gzip_proxied`: off / expired / no-cache / no-store / private / no_last_modified / no_etag / auth / any |
| `compress_buffers <n> <size>` | (count, size) | `16 8k` | per-request output buffer pool |
| `compress_http_version 1.0\|1.1` | enum | `1.1` | minimum HTTP version to compress |

Per-codec override examples: `compress_brotli_min_length`,
`compress_zstd_types`, `compress_gzip_buffers`, etc.

### 4.5 Typed configuration model

Directives are parsed at configuration time into a strongly typed
`CompressConfig` in the policy crate (which keeps `forbid(unsafe)`):

- one `CodecConfig` per codec (enabled flag, level, codec-specific knobs,
  resolved shared parameters after per-codec override),
- a resolved priority order,
- shared defaults.

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

# optional: override the default zstd > br > gzip order
compress_priority zstd br gzip;
```

Dictionary directives (`dcb`/`dcz`) are deliberately out of scope here and are
specified with the M4 dictionary milestone.

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

Unit tests already cover negotiation and progress invariants. Add property
tests (`proptest`):

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
`nginx/nginx-acme`) for authoritative coverage, plus a thin Rust harness
(`assert-cmd` + `reqwest`) spawning a pinned NGINX for the fast developer loop.

Coverage: filter order, backpressure, reload, client disconnect, chunked
upstreams, and HTTP/1.1 / HTTP/2 / HTTP/3 interoperability.

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
4. **Divergence tests** — filter order, reload, client disconnect, and chunked
   transfer are each exercised under both link modes.

The dynamic build is the first release target; static support is validated in
parallel but the release matrix in architecture.md prefers dynamic modules.

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

Threaded/async compression (e.g. `zstdmt`) is intentionally not enabled: it
conflicts with the first-release non-goal of thread-pool compression and needs a
bounded work budget on the event loop. That remains a later milestone.

Pinned dependency majors are current as of this milestone: `ngx` 0.5, `flate2`
1, `brotli` 8, `zstd` 0.13.

## 8. Deferred / open items

- HTTP/3 (QUIC) interop test — deferred. The body filter is transport-agnostic
  and HTTP/2 interop is verified in `docker/edge-tests.sh`; a full HTTP/3 test
  needs `--with-http_v3_module`, TLS certificates, and a QUIC-capable client,
  which is disproportionate for the marginal added coverage. The L0 property
  tests, L2 Test::Nginx suite (`t/`), L3 fuzz scaffolding (`fuzz/`), and the
  edge suite (backpressure, client disconnect, truncated upstream, HTTP/2,
  no-panic) are in place.
- Static-build subrequest filter position — known limitation. The body filter is
  ordered at the gzip slot (after `postpone_filter`) via `ngx_module_order` so
  subrequest-assembled responses (SSI includes, `add_after_body`) are compressed
  after assembly. NGINX honors this for **dynamic** modules (re-sorted at load
  time), the supported target — verified by an SSI round-trip test. **Static**
  builds keep the compile-time array order, which places the filter above
  `postpone`, so a static build compresses subrequest responses at the wrong
  point (non-subrequest responses are correct in both modes; the build test
  records this as a documented caveat, not a failure). Consistent with the
  design's dynamic-first stance. Revisit if static SSI support is required.
- Static precompressed serving (`.br` / `.gz` / `.zst` sidecar) — M3, done. A
  content-phase handler (`compress_static on|off|always`, §4.7) that serves a
  precompressed sidecar file when one exists and the client accepts it, like
  `gzip_static`. It is a content handler, not the M1/M2 header/body filter,
  because it replaces the file being served rather than transforming a stream.
  Verified in both dynamic and static builds (it correctly precedes nginx's
  static handler in both).
- Named profiles (`compress fast|balanced|max`) — M3, done. A turnkey preset
  over the per-codec knobs (§4.2.1); explicit directives override, preset numbers
  are benchmark-calibrated.
- Worker-local codec-context reuse — M3. Reset and reuse a per-worker codec
  instead of allocating a fresh encoder per request (the `StreamingCodec::reset`
  seam already exists). Worker-local only, no cross-worker/cross-request shared
  mutable state, no request-path locks.
- Benchmark-driven profiles — M3. A reproducible ratio × throughput × CPU
  harness that calibrates the profile tiers' default levels and the
  per-response-class priority order.
- Per-response-class priority (dynamic vs cacheable) refining the default
  order — M3, calibrated by the benchmark above.
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
- Dictionary transport directives and lifecycle — M4.
- Symmetric decode ("unboxing") — proposed **M5**, to be discussed. This module
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
  (upstream response vs request body). Not started.
