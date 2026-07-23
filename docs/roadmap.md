# Post-v0.1 development plan

This document records the engineering sequence after the v0.1.0 Technical
Preview. It is driven by known runtime risk, user-visible value, technical
dependencies, and verification cost. Repository activity, issue traffic, and
external requests may add evidence, but they do not determine the plan.

Released behavior remains defined by the README and installation guide. This
plan may change when measurements invalidate an assumption; such a change must
record the evidence and the resulting design decision.

## Planning rules

Work is ordered by these rules:

1. close a known correctness, safety, or runtime-contract gap before expanding
   the same execution path;
2. prefer a broadly useful NGINX behavior over optional configuration surface;
3. measure before adding concurrency, caching, or another stateful subsystem;
4. keep speculative protocol work behind a narrow feasibility gate;
5. treat compatibility reports as a continuous validation input, not as a
   prerequisite for making progress.

The release matrix, sanitizer suites, reload tests, and source-build rehearsal
remain continuous gates. Keeping an existing gate green is maintenance work,
not a separate feature milestone.

## Current assessment

| Order | Workstream | Value | Main risk/cost | Decision |
| --- | --- | --- | --- | --- |
| 1 | Bounded event-loop work and `max` profile semantics | Protects worker latency and establishes a reusable execution contract | Resumable processing must preserve flush, finish, backpressure, and NGINX chain ownership | Immediate design and measurement |
| 2 | `compress_proxied` | Fills a common response-eligibility gap for reverse-proxy deployments | Header/date policy and inheritance must match NGINX without importing private gzip state | Next user-visible feature |
| continuous | Build-signature and deployment validation | Reduces installation uncertainty | Every additional target multiplies the build and test matrix | Improve tooling as concrete targets become available; do not block feature work |
| 3 | Compression Dictionary Transport feasibility | Potentially differentiating compression gains for versioned assets | Dictionary lifecycle, cache partitioning, interoperability, and security are a large new subsystem | Prototype only after orders 1 and 2 |
| deferred | Configurable server priority | Adds operator control | Client quality values and codec toggles already cover most cases | Wait for a concrete case the current controls cannot express |
| deferred | Symmetric decoding | Reuses much of the codec/filter foundation | Decompression bombs, output limits, filter position, and request-vs-response scope | Separate design decision after bounded-work primitives exist |
| parked | Threaded/asynchronous compression | Could isolate expensive codec work | Cross-thread ownership and NGINX integration complexity are high | Reconsider only if bounded synchronous work cannot meet latency goals |

## 1. Bound event-loop work and resolve `max`

This is the first task because v0.1 exposes Brotli 11 and Zstandard 19 through
`compress max`, while the body filter runs inside an NGINX worker event loop.
Output-buffer limits bound retained output memory, but they do not by themselves
bound CPU time or codec iterations in one callback. The current description of
`max` as suitable for controlled or precompressed work is therefore not a
complete runtime contract.

### 1.1 Measure the existing behavior

Add a versioned callback-level benchmark that covers:

- highly compressible and incompressible 4 KiB, 256 KiB, and 8 MiB streams;
- `fast`, `balanced`, and `max`;
- normal and slow downstream consumption;
- representative concurrency rather than only single-request throughput;
- input consumed, output produced, codec-step count, callback latency, request
  latency, throughput, and worker RSS.

The existing codec matrix remains useful for ratio and aggregate throughput,
but it cannot answer event-loop fairness questions because it drives a complete
stream outside NGINX.

### 1.2 Define the safe-core work contract

The design must state and enforce:

- the maximum input bytes and codec steps processed by one body-filter
  invocation;
- which states mean input-starved, output-starved, budget-exhausted, or
  complete;
- how unconsumed input and pending flush/finish state survive resumption;
- how free/busy chains remain owned while downstream applies backpressure;
- which counters are observable in tests without logging request content.

Budget exhaustion is a normal resumable state, not a codec failure and not
permission to retry already-consumed input.

### 1.3 Make an evidence-based `max` decision

After the baseline measurement, choose one of these outcomes:

- keep `max` available for runtime compression only if the enforced budget
  meets the agreed latency and fairness thresholds;
- retune the preset if lower levels preserve nearly all compression benefit;
- or limit `max` to explicitly controlled workloads and make that limitation
  enforceable rather than advisory.

Do not add worker threads merely to preserve the current preset numbers. First
prove that bounded resumable synchronous processing is insufficient.

### Exit gate

This phase is complete when the budget is part of the safe Rust state machine,
large-stream tests demonstrate resumption under backpressure, H1/H2/H3 and
static/dynamic behavior remain equivalent, and the `max` contract is explicit
in configuration and documentation.

## 2. Add proxied-response eligibility

Once the runtime execution contract is settled, add `compress_proxied` as the
next broadly useful capability. It should mirror the `gzip_proxied` vocabulary:
`off`, `expired`, `no-cache`, `no-store`, `private`, `no_last_modified`,
`no_etag`, `auth`, and `any`.

The design remains:

- default to `off`;
- identify a proxied request through `Via`, matching NGINX semantics;
- apply one decision to gzip, deflate, Brotli, and Zstandard runtime output;
- apply it to `compress_static on`, while `compress_static always` bypasses it;
- reduce request and response headers to Rust-owned facts at the FFI boundary;
- evaluate the policy in the unsafe-free core;
- keep built-in gzip conflict handling as an independent fail-closed guard;
- do not call `ngx_http_gzip_ok()` or read private gzip configuration.

Acceptance covers inheritance and child overrides, every flag and meaningful
combination, invalid dates, repeated Cache-Control fields, spoofable `Via`,
runtime/static paths, and HTTP/1.1, HTTP/2, and HTTP/3 parity.

## Continuous validation lane

Compatibility work continues alongside the numbered sequence:

- keep the exact v0.1 baseline reproducible;
- make it easy to capture the target `nginx -V`, compiler/ABI, distribution
  revision, module mode, codec backend, and protocol results;
- fix reproducible build friction, crashes, corrupt output, reload failures, or
  static/dynamic differences immediately;
- add a supported target only when it can be reproduced and maintained in the
  automated matrix.

An absence of external reports means only that no additional target is proven.
It does not block work on the known design sequence.

## 3. Dictionary-transport feasibility gate

Compression Dictionary Transport (`dcb`/`dcz`) is the first candidate after the
runtime contract and proxied policy, but the initial deliverable is deliberately
a narrow prototype for versioned static assets, not production lifecycle code.

The prototype must answer:

- whether an interoperable client can complete advertisement and dictionary
  selection;
- whether the measured byte and latency savings justify the operational cost;
- how dictionaries are identified, validated, expired, and partitioned;
- how `Vary: Accept-Encoding, Available-Dictionary` and caches remain correct;
- how HTTPS, origin boundaries, CORS, and sensitive responses fail closed.

Proceed to a production design only if interoperability, representative benefit,
and the security/cache model all pass. Automatic dictionary training remains
out of scope.

## Deferred decisions

- `compress_priority`: retain the fixed `zstd > br > gzip > deflate` tie-break
  until a real configuration cannot be expressed with codec enablement,
  profiles, or client `q` values.
- Symmetric decoding: keep it in this workspace if selected, but first choose
  upstream-response decoding or request-body decoding and define output/work
  limits against decompression bombs.
- HTTP/1.0 eligibility and legacy user-agent regex controls: add only for a
  demonstrated compatibility requirement.
- HTTP/3 0-RTT: wait for upstream and test-toolchain maturity.

## Explicit non-goals

- a universal `.so` independent of the target NGINX build signature;
- a module-owned cache of compressed dynamic output;
- opaque retries after partial encoder failure;
- per-MIME or per-size algorithm-selection DSLs beyond the shipped shared MIME
  allow-list, minimum length, and per-codec compression levels;
- automatic dictionary training;
- reimplementing compression algorithms.
