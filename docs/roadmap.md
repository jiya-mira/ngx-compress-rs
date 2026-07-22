# Roadmap

This roadmap ranks work by user value, correctness and safety necessity,
feasibility, and total implementation and verification complexity. It is a
decision record, not a promise that every idea will ship.

Only released behavior documented in the README and installation guide is part
of the support contract. GitHub milestones contain committed release scope;
issues track accepted work; discussions are the preferred place for early
feedback and proposals.

## How priorities are assigned

| Priority | Meaning |
| --- | --- |
| P0 | Correctness, safety, or adoption evidence needed before expanding scope |
| P1 | High-value, feasible work targeted at the next feature release |
| P2 | Valuable but feedback- or design-gated work with no release commitment |
| P3 | Watch-list item; low current necessity or disproportionate complexity |

Complexity includes implementation, FFI surface, configuration compatibility,
security review, test-matrix growth, documentation, and long-term maintenance.

## Current assessment

| Workstream | Priority | Necessity | Feasibility | Complexity | Decision |
| --- | --- | --- | --- | --- | --- |
| Real deployment and build-signature evidence | P0 | High | High | Small–medium | Active in [issue #1](https://github.com/jiya-mira/ngx-compress-rs/issues/1) |
| Event-loop work limits and `max` profile safety | P0 | High | Medium | Large | Design and measurement in [issue #2](https://github.com/jiya-mira/ngx-compress-rs/issues/2) |
| `compress_proxied` | P1 | High | High | Medium | Committed to the [v0.2.0 milestone](https://github.com/jiya-mira/ngx-compress-rs/milestone/1) in [issue #3](https://github.com/jiya-mira/ngx-compress-rs/issues/3) |
| Additional supported platforms/signatures | P1 | Medium–high | Medium | Large and cumulative | Evidence-gated; no support promise from a single successful report |
| Compression Dictionary Transport (`dcb`/`dcz`) | P2 | Medium | Medium | Extra large | Public [RFC discussion](https://github.com/jiya-mira/ngx-compress-rs/discussions/5) and prototype first |
| Configurable `compress_priority` | P2 | Medium | High | Small–medium | Implement only after concrete deployment feedback |
| Symmetric decoding | P3 | Low–medium | Medium | Extra large and security-sensitive | Proposal only; separate design review required |
| HTTP/1.0 eligibility control | P3 | Low | High | Small | Add only for a demonstrated compatibility need |
| Threaded or asynchronous compression | P3 | Low | Low–medium | Extra large | Do not pursue before bounded-work evidence shows it is necessary |
| HTTP/3 0-RTT | P3 | Low | Low–medium | Large | Track upstream maturity; outside the current support contract |

## P0: stabilize the Technical Preview

The immediate goal is not feature count. It is to learn whether real users can
build, configure, and operate the module safely outside the release fixture.

### Deployment and compatibility evidence

- collect exact `nginx -V` signatures, OS/architecture, module mode, codec
  backend, protocol, and outcome through the compatibility-report issue form;
- treat unverified reports as evidence, not as an expanded support contract;
- require a reproducible CI job or at least two independent matching reports
  before proposing a new supported platform/signature;
- prioritize install friction, worker crashes, corrupt output, reload failures,
  and static/dynamic behavioral differences over new directives.

### Event-loop and profile safety

The `max` profile exposes expensive Brotli and Zstandard levels. Before adding
dictionary compression, decoding, or threaded execution, measure worst-case
worker latency for highly compressible and incompressible large streams. The
design decision must state:

- how much input and how many codec iterations one callback may process;
- what yields or backpressure behavior occurs when that budget is exhausted;
- whether `max` remains valid for runtime compression or is explicitly limited
  to controlled/precompressed workloads;
- the latency, throughput, memory, and fairness regression thresholds.

## P1: v0.2.0 proxied-response policy

The next committed user-facing feature is `compress_proxied`, a module-wide
equivalent of NGINX's `gzip_proxied` directive:

- syntax: `off`, `expired`, `no-cache`, `no-store`, `private`,
  `no_last_modified`, `no_etag`, `auth`, and `any`;
- default: `off`, matching NGINX;
- a request is proxied when its request headers contain `Via`, matching NGINX's
  definition rather than whether the location uses `proxy_pass`;
- the decision applies to every runtime coding (`gzip`, `deflate`, `br`, and
  `zstd`), not only gzip;
- `compress_static on` follows the same eligibility decision, while
  `compress_static always` deliberately bypasses it;
- request and response headers are copied or reduced to Rust-owned facts at the
  FFI boundary, and the policy runs in the safe core;
- the implementation does not call `ngx_http_gzip_ok()` or read private gzip
  configuration.

Acceptance includes inheritance and child overrides, all flag combinations,
invalid dates, repeated Cache-Control fields, spoofable `Via`, runtime/static
paths, and HTTP/1.1, HTTP/2, and HTTP/3 parity.

## P2: feedback-gated development

### Compression Dictionary Transport

RFC 9842 standardizes `dcb` and `dcz`, but server-side implementation still has
a large security and lifecycle surface. The first deliverable is a public RFC
and prototype, not an unconditional M4 implementation commitment. It must cover:

- `Use-As-Dictionary`, `Available-Dictionary`, and `Dictionary-ID` parsing;
- dictionary advertisement, selection, freshness, and lifecycle;
- SHA-256 dictionary validation and codec framing;
- HTTPS-only use, same-origin/CORS protections, and sensitive-response policy;
- cache partitioning and `Vary: Accept-Encoding, Available-Dictionary`;
- client interoperability and measurable benefit on versioned static assets.

Implementation enters a release milestone only after the RFC identifies a
real deployment owner or representative workload, interoperable client tests,
and a security model that fails closed. Automatic dictionary training is not
part of this workstream.

### Configurable server priority

v0.1 uses the fixed `zstd > br > gzip > deflate` tie-break order. A future
`compress_priority` is feasible, but client quality values already express much
of the preference space. It should be promoted only when users provide a case
that cannot be handled by codec enablement, profiles, or client `q` values.

## P3: proposals and watch list

### Symmetric decoding

A sibling module capable of decoding gzip, Brotli, and Zstandard may eventually
cover upstream responses, request bodies, or both. These are different filter
positions and security models, so the scope must not be selected implicitly.
Output limits, work budgets, decompression-bomb defenses, and failure semantics
are prerequisites. This remains a proposal rather than an M5 commitment.

### Other watch-list items

- an HTTP/1.0 gate analogous to `gzip_http_version`;
- optional threaded or asynchronous compression after bounded-work evidence;
- HTTP/3 0-RTT after upstream and test-toolchain maturity.

A `gzip_disable`-style legacy user-agent regex is not currently planned.

## Community feedback and tracking

The public workflow is deliberately lightweight:

The current prioritization is open for feedback in the
[post-v0.1 roadmap discussion](https://github.com/jiya-mira/ngx-compress-rs/discussions/4).

1. Questions, early ideas, and design trade-offs begin in
   [GitHub Discussions](https://github.com/jiya-mira/ngx-compress-rs/discussions).
2. Reproducible bugs, compatibility reports, and accepted work use
   [GitHub Issues](https://github.com/jiya-mira/ngx-compress-rs/issues).
3. Only accepted release scope receives a GitHub milestone. Milestones have no
   speculative due dates.
4. Pull requests link the accepted issue and record verification evidence.
5. Completed behavior enters the changelog and release notes.

Priority and feedback labels make triage visible. Complexity and acceptance
gates remain in each roadmap issue so a single label does not disguise risk.
A GitHub Project will be added only when the active roadmap grows beyond what
milestones and issue filters can explain clearly, or when multiple maintainers
need a shared board.

## Explicit non-goals

- a universal `.so` independent of the target NGINX build signature;
- a module-owned cache of compressed dynamic output;
- opaque retries after partial encoder failure;
- per-MIME or per-size algorithm-selection DSLs beyond the shipped shared MIME
  allow-list, minimum length, and per-codec compression levels;
- automatic dictionary training;
- reimplementing compression algorithms.
