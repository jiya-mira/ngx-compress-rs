# Unsafe boundary refactor (deferred until after M3)

## Status

This is a deferred safety-architecture initiative. Do not start the refactor
while M3 is still in progress. After M3 lands, re-audit the final code before
turning this note into an implementation plan.

The work is intentionally separate from feature development. It is expected to
span multiple small changes rather than one mechanical rewrite.

## Goal

Move nearly all protocol, policy, selection, and streaming decisions into safe
Rust, leaving raw NGINX and codec FFI access in a narrow, auditable boundary.

The target data flow is:

```text
NGINX raw state
    -> prefetch and validate
    -> safe-core decision or state transition
    -> submit the validated plan to NGINX
```

Reducing the number of `unsafe` blocks is not the primary metric. The important
property is that unsafe code cannot manufacture overly broad lifetimes, leak raw
pointers into policy models, or mix business decisions with unchecked memory
access.

## Core engineering principle

Choosing Rust should change the architecture, not merely the implementation
language. The project must avoid becoming C-style pointer and lifetime management
written with Rust syntax.

The default rule is:

> Convert external data into Rust-owned typed values. Use a validated,
> lifetime-bound zero-copy view only when the cost of copying is demonstrated to
> be unacceptable. Business logic must not receive raw pointers.

This deliberately places the burden of proof on zero-copy, not on copying.
Small, bounded copies and clones are an acceptable safety cost when they remove
FFI lifetimes, aliasing assumptions, or external ownership from the safe core.
Examples include configuration data, MIME types, header values, URI fragments,
and compact request/response snapshots.

Large streaming bodies and NGINX output buffers are the main expected exception.
They should remain zero-copy, but only through views whose lifetimes and mutable
access are tied to validated Rust borrows. Zero-copy must not mean that raw
pointers or caller-selected lifetimes enter the state machine.

The intended split is therefore:

```text
extern callback
    -> unsafe prefetch and validation
    -> owned facts or lifetime-bound views
    -> forbid(unsafe) Rust core
    -> typed decision / plan / StepResult
    -> unsafe submit
    -> NGINX
```

The safe core is the product. The FFI code is an adapter around it.

### Copy and clone policy

Prefer copying or cloning when all of the following hold:

- the amount of data has a clear, small bound;
- the copy occurs during configuration, reload, or once per request;
- it removes a raw pointer, external lifetime, or aliasing requirement;
- it makes the resulting logic directly testable as safe Rust.

Avoid copying when it would:

- duplicate an unbounded response body;
- allocate on every codec step;
- accumulate behind a slow downstream;
- duplicate large codec state or NGINX chains.

Where zero-copy is retained, document why it is needed and which type enforces
the ownership, lifetime, capacity, and mutation invariants.

## Boundary model

### 1. Prefetch

Read and validate external state once, as close to the callback or configuration
boundary as possible.

- Configuration data should become Rust-owned typed values during config load.
- Small request/response data should become a typed per-callback snapshot.
- Body buffers should become zero-copy, lifetime-bound views rather than owned
  copies.
- Invalid nullability, lengths, pointer ordering, UTF-8, or element types must
  return an explicit error. Do not hide invariant failures with fallback values.
- An unsafe conversion may return an owned value or a view whose lifetime is
  tied to a real Rust borrow. It must not return a caller-selected lifetime.

### 2. Safe core

The following logic should run under `forbid(unsafe_code)`:

- content negotiation and eligibility;
- MIME matching and profile resolution;
- codec selection;
- streaming state transitions and progress validation;
- backpressure and output-budget decisions;
- static-sidecar candidate ordering;
- production of typed decisions/plans for the submit layer.

Safe-core code must not know about `ngx_http_request_t`, `ngx_buf_t`,
`ngx_array_t`, `ngx_str_t`, or raw chain pointers.

Safe-core APIs should prefer types that make invalid states difficult or
impossible to construct: owned facts, enums for decisions and states, bounded
budgets, and plans that are complete before submission begins.

### 3. Submit

Apply a safe-core decision to NGINX with short, explicit FFI operations.

- update or clear response headers;
- install request context and cleanup handlers;
- commit consumed/produced buffer positions and boundary flags;
- update free/busy/out chains;
- open and emit precompressed files;
- invoke the next filter.

Submit code should not repeat policy decisions or reinterpret raw input. Partial
submission followed by pass-through must be avoided.

Submit should consume a typed plan wherever practical. This keeps mutation order
and failure behavior explicit and prevents a response from being partially
rewritten before a later operation fails.

## Current priority areas to re-audit after M3

### P0: lifetime soundness

The current raw-pointer helpers that return an unconstrained `'a` must be
removed first:

- `input_slice<'a>(*mut ngx_buf_t) -> &'a [u8]`;
- `writable<'a>(*mut ngx_buf_t) -> &'a mut [u8]`;
- `request_ctx<'a>(*mut ngx_http_request_t) -> Option<&'a mut RequestCtx>`.

Replace them with lifetime-bound buffer/request views or callback-scoped access.
Mutable request context should use a safe ownership/borrowing model (for example,
a request-owned state wrapper with fallible interior borrowing) so re-entry is
an explicit error instead of an aliasing risk or panic.

### P1: configuration prefetch

`compress_types` should no longer retain `NonNull<ngx_array_t>` in
`CompressConfig`/`Resolved`. Convert it once at configuration time into a typed,
Rust-owned MIME collection. Configuration inheritance should share or borrow the
typed collection without request-path allocation.

Unify directive callback marshalling so `cf.args` and the raw config pointer are
validated once, then passed to safe parsers/setters.

### P1: header snapshot and decision

Prefetch status, main/subrequest identity, existing content encoding, known
content length, content type, and parsed `Accept-Encoding` into a typed snapshot.
Eligibility and codec selection should accept only that snapshot plus resolved
configuration and return a typed compression decision.

Header mutation belongs in submit and should occur only after the decision and
required request state are ready.

### P1: body buffer and chain views

Create filter-specific safe views that validate and bind the lifetimes of:

- readable `pos..last` input;
- writable `last..end` output capacity;
- operation/boundary flags;
- consumed/produced commits;
- free/busy/out chain ownership.

The safe streaming loop should call the checked codec step and return a typed
result. Only the FFI view should advance pointers or link NGINX buffers.

### P1: static sidecar handler

After M3 stabilizes, split request method/path/config/negotiation prefetch from
safe candidate selection and from file-open/output submission. Encapsulate mapped
path pointer arithmetic in a checked boundary type. Prefer existing safe
`ngx-rust` request methods where their semantics are sufficient.

### P2: unavoidable FFI

Keep, but tighten, the inherently unsafe areas:

- module symbols, directive tables, and phase/filter registration;
- next-filter invocation and NGINX global filter pointers;
- request-pool cleanup callbacks;
- file/output-chain construction;
- system libbrotli calls.

Opaque codec state should use RAII and non-null typed handles after successful
construction. Exported callbacks should share one non-unwinding error boundary.

## Proposed implementation sequence

1. Re-audit the post-M3 tree and update this note with final locations.
2. Remove the unconstrained-lifetime helpers without changing behavior.
3. Move MIME types and directive arguments to configuration-time prefetch.
4. Introduce the header snapshot and safe compression decision.
5. Introduce lifetime-bound body buffer/chain views and make the streaming loop
   safe.
6. Apply the same split to the static-sidecar handler.
7. Move remaining raw NGINX access behind the dedicated FFI boundary and enforce
   `forbid(unsafe_code)` on safe modules/crates.

Each step should be independently reviewable and tested. Do not combine this
work with backpressure behavior changes, new directives, or codec/profile tuning.

## Acceptance criteria

- No unsafe function returns a reference with a caller-selected lifetime.
- Raw NGINX pointers do not appear in resolved policy/configuration models.
- Safe-core public APIs accept only owned values or lifetime-bound safe views.
- Small bounded copies/clones are preferred when they remove external ownership
  or aliasing from the core; every retained zero-copy path has a documented
  performance reason and an enforcing type.
- MIME, negotiation, eligibility, selection, and state-machine logic are safe
  Rust and directly unit-testable.
- The body path remains zero-copy while slice lifetimes are tied to validated
  owners.
- Large unsafe blocks contain no policy branching.
- Every unsafe operation documents the local invariant it relies on.
- Prefetch and submit failures are explicit and fail closed.
- No panic can unwind or abort through an exported callback during normal error
  handling.
- Existing NGINX integration behavior remains covered across backpressure,
  truncated upstreams, disconnects, reloads, and static/dynamic module builds.

## Review questions

Use these questions during implementation and review:

1. Can this external value be copied or parsed once into an owned Rust type?
2. If zero-copy is retained, is the performance need real and documented?
3. Is every returned reference tied to a real Rust owner rather than a raw
   pointer and caller-selected lifetime?
4. Can the business decision be unit-tested without constructing NGINX objects?
5. Does the safe core produce a complete typed plan before any response mutation?
6. Can an FFI failure leave NGINX in a partially submitted state?
7. Does this unsafe block perform only validation/marshalling/submission, or has
   policy logic leaked back into it?
8. Would `forbid(unsafe_code)` still compile for the module containing the
   business logic?

## Related work

The separate `codex/m1-runtime-safety` branch contains two prerequisite safety
commits created before this refactor:

- `a01c0f1` enforces codec progress validation on the production step path;
- `d53875f` makes codec reset fallible and prevents reuse of poisoned contexts.

Integrate or reconcile those commits with M3 before beginning the boundary
refactor.
