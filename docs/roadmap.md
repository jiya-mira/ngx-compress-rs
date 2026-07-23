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
4. research an immature operational model before implementation, but implement
   an adopted standard completely rather than publishing a protocol subset;
5. treat compatibility reports as a continuous validation input, not as a
   prerequisite for making progress.

The release matrix, sanitizer suites, reload tests, and source-build rehearsal
remain continuous gates. Keeping an existing gate green is maintenance work,
not a separate feature milestone.

## Current assessment

| Order | Workstream | Value | Main risk/cost | Decision |
| --- | --- | --- | --- | --- |
| 1 | Remove `max` and bound event-loop work | Removes an unsuitable runtime preset and establishes a reusable execution contract | Resumable processing must preserve flush, finish, backpressure, and NGINX chain ownership | Immediate implementation; do not spend another round benchmarking whether `max` should remain |
| 2 | Configurable server priority | Lets operators express server policy when client quality values tie, and becomes more important as encodings are added | Directive parsing, inheritance, duplicate/disabled coding validation, and deterministic fallback must remain simple | Add `compress_priority` as a near-term, bounded configuration feature |
| 3 | Asynchronous compression execution | Keeps expensive codec work off the NGINX worker event loop | Cross-thread ownership, completion, cancellation, reload, and request ordering are difficult | Promoted from parked work; design after the resumable work contract, then implement if the lifecycle can remain fail-closed |
| 4 | `compress_proxied` | Fills a common response-eligibility gap for reverse-proxy deployments | Header/date policy and inheritance must match NGINX without importing private gzip state | Next response-policy feature |
| continuous | Build-signature and deployment validation | Reduces installation uncertainty | Every additional target multiplies the build and test matrix | Improve tooling as concrete targets become available; do not block feature work |
| 5 | Complete Compression Dictionary Transport | Adds standardized `dcb`/`dcz` with gains comparable in importance to adding a codec family | Dictionary provisioning, lifecycle, cache partitioning, interoperability, and security form a large subsystem | Complete the design study first, then implement RFC 9842 as one production milestone rather than a static-only prototype |
| deferred | Symmetric decoding | Reuses much of the codec/filter foundation | Decompression bombs, output limits, filter position, and request-vs-response scope | Separate design decision after bounded-work primitives exist |

## 1. Remove `max` and bound event-loop work

The next implementation round removes the `max` profile. Its Brotli 11 and
Zstandard 19 settings are not suitable defaults for runtime work in an NGINX
worker event loop, and there is no remaining product question that justifies
benchmarking whether the profile should stay. The per-codec level directives
remain available for operators with an explicitly controlled workload.

Removal covers configuration parsing and merging, preset resolution, tests,
benchmarks, examples, and current-version documentation. Released v0.1.0
artifacts and release notes remain historical records.

Removing `max` reduces the worst preset but does not itself bound synchronous
work. Output-buffer limits bound retained output memory, not CPU time or codec
iterations in one callback. The same round therefore defines the reusable
event-loop work contract before adding another execution path.

### 1.1 Measure the remaining runtime profiles

Add a versioned callback-level benchmark that covers:

- highly compressible and incompressible 4 KiB, 256 KiB, and 8 MiB streams;
- `fast`, `balanced`, and representative explicit per-codec levels;
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

### Exit gate

This phase is complete when `max` is absent from the active configuration
surface, the budget is part of the safe Rust state machine, large-stream tests
demonstrate resumption under backpressure, and H1/H2/H3 plus static/dynamic
behavior remain equivalent.

## 2. Add configurable server priority

Add `compress_priority` as a compact, inherited server-policy directive. Client
`q` values remain authoritative: the configured order only breaks ties among
enabled, eligible codings with the same effective client quality.

The directive should:

- accept each enabled content coding at most once;
- reject unknown names, duplicates, and ambiguous partial lists rather than
  silently repairing them;
- define whether omitted but enabled codings are appended in the default order
  or require a complete list before implementation begins;
- inherit through the ordinary `http` → `server` → `location` merge cascade;
- keep `identity` as the implicit final fallback rather than exposing it as a
  configurable entry;
- apply consistently to runtime selection and precompressed representations;
- leave explicit client `q=0`, wildcard, and identity rules unchanged.

The design must reserve a clean extension path for `dcb` and `dcz` so the
directive does not need incompatible syntax when dictionary transport lands.

### Exit gate

Parsing, inheritance, explicit codec disablement, client quality precedence,
runtime/static parity, and deterministic fallback are covered in safe-core and
NGINX integration tests.

## 3. Add an asynchronous compression execution path

Asynchronous compression is now planned work rather than a parked possibility.
The goal is worker-event-loop isolation, not preservation of extreme preset
levels and not merely enabling a codec library's internal multithreading.

The design must decide whether to use NGINX thread pools or a narrowly owned
module executor and must enforce these boundaries:

- no `ngx_buf_t`, request-pool allocation, chain link, or borrowed request
  memory may be accessed by a compression thread;
- input handed to a thread is Rust-owned, bounded, and associated with one
  request generation;
- completion returns to the owning NGINX event loop before any header, chain, or
  downstream operation;
- per-request output order, flush, finish, backpressure, disconnect, timeout,
  reload, and cancellation semantics remain explicit;
- queue depth, copied bytes, and outstanding work are bounded per worker;
- queue saturation or executor failure follows a documented fail-closed or
  synchronous-fallback policy without partially encoded retries;
- small responses stay synchronous when dispatch overhead is greater than the
  measured benefit.

The resumable safe-core contract from phase 1 is a prerequisite: asynchronous
work uses the same progress states and validation rather than introducing a
second codec state machine.

### Exit gate

Proceed from design to production implementation only if the ownership model
can be expressed without cross-thread NGINX pointers and lifecycle tests cover
disconnects, graceful reloads, cancellation, queue saturation, and H1/H2/H3
ordering. Benchmarks must show improved worker tail latency under concurrency
without unacceptable throughput or memory regression.

## 4. Add proxied-response eligibility

After the execution-path work, add `compress_proxied` as the next broadly useful
response-policy capability. It should mirror the `gzip_proxied` vocabulary:
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

## 5. Implement Compression Dictionary Transport completely

`dcb` and `dcz` are standardized content codings rather than new compression
algorithms, but they are planned at the same priority class as adding a codec
family. Browser support affects the interoperability matrix, not whether the
work deserves a production design: unsupported clients naturally continue to
receive `br`, `zstd`, `gzip`, or `identity`.

Do not begin implementation until a focused design study resolves the
operational model. This is a design gate, not permission to publish a reduced
static-sidecar feature. The current direction is recorded in
[the dictionary-transport design](dictionary-transport.md): one inherited
`compress_dictionary off|lazy|<file>` directive, with `http`-level lazy
enablement internally partitioned into independent per-origin/per-location
dictionary managers. Each manager may skip generation, generate once, or
maintain progressive immutable generations.

Once that design is accepted, the production milestone implements RFC 9842 as a
whole:

- `Use-As-Dictionary`, `Available-Dictionary`, `Dictionary-ID`, and
  `compression-dictionary` link handling;
- both `dcb` and `dcz`, including their required framing and hash validation;
- external dictionary files and automatic lazy collection/generation through
  the same internal registry;
- dynamic compression and precomputed representations backed by the same
  dictionary registry;
- correct content negotiation, `Vary`, caching, HTTPS, same-origin/CORS,
  readability, privacy, and sensitive-response protections;
- ordinary encoding fallback for clients without a usable dictionary;
- interoperability, malformed-input, cache-version, reload, disconnect, and
  H1/H2/H3 tests.

An interoperability spike may be used inside development and tests, but it is
not a separately shipped milestone. Exact sampling, generation, retention, and
complex-location policies remain implementation questions, not additional
required user-facing configuration.

## Deferred decisions

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
- synchronous or unbounded discovery/training in the NGINX request path;
- reimplementing compression algorithms.
