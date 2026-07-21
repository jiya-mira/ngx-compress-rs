# Release readiness

Status as of 2026-07-22: the repository is a private technical preview. The
implementation and current integration tests are suitable for continued
engineering work, but the module is not yet declared production-ready.

This checklist records the remaining work for the first public release. Items
under **Release blockers** should be closed or explicitly removed from the v0.1
support contract before publishing production artifacts.

## Completed publication groundwork

- [x] Adopt Apache-2.0 and add package license metadata.
- [x] Replace the obsolete project README with current status, configuration,
      compatibility, and validation guidance.
- [x] Add source-build and installation instructions.
- [x] State that the first supported deployment target is a dynamic module.
- [x] Create the private `jiya-mira/ngx-compress-rs` GitHub repository.
- [x] Exclude local IDE state, credentials, build products, test runtime state,
      and regenerable code-graph output from the published tree.

## Release blockers

### 1. Continuous verification

- [ ] Add CI for formatting, Clippy, pure-Rust tests, and package-content checks.
- [ ] Run the Docker dynamic/static integration suite in CI.
- [ ] Run edge tests and both codec backends in CI.
- [ ] Retain failing NGINX logs and relevant build artifacts for diagnosis.
- [ ] Make required checks blocking for the release branch/tag.

### 2. Compatibility and artifact model

- [ ] Decide whether v0.1 is source-only or also distributes binary modules.
- [ ] Define the supported NGINX versions, distributions, architectures,
      configure signatures, and codec backends.
- [ ] Exercise every supported build signature rather than treating
      `--with-compat` as a universal ABI guarantee.
- [ ] Record the NGINX version, configure arguments, compiler, target, backend,
      and dependency versions alongside every binary artifact.
- [ ] If binaries are published, provide checksums and a reproducible naming
      scheme that cannot be mistaken for a universal `.so`.

### 3. NGINX lifecycle and memory-safety proof

- [ ] Add repeated graceful-reload tests that exercise configuration ownership,
      worker shutdown, request cleanup, and codec destruction.
- [ ] Add multi-worker concurrent traffic and a sustained soak test.
- [ ] Run an appropriate memory-safety diagnostic build around NGINX/FFI paths
      (for example sanitizer-enabled native components or an equivalent tool).
- [ ] Exercise allocation failures, codec initialization/reset failures, and
      downstream error returns without leaking or partially mutating responses.
- [ ] Confirm client disconnect and truncated-upstream paths under the diagnostic
      build, not only through post-event worker liveness.

### 4. Failure observability

- [ ] Log caught Rust panics as structured NGINX errors before returning the
      callback fallback status.
- [ ] Include safe request/module context while keeping logs free of sensitive
      header or response data.
- [ ] Verify that codec, allocation, invalid-state, and FFI validation failures
      remain distinguishable in production logs.

### 5. Supported-target contract

- [ ] Make dynamic-module support the unambiguous v0.1 contract in release notes
      and artifact names.
- [ ] Either exclude static modules from the supported contract or prominently
      retain the SSI/subrequest filter-order limitation.
- [ ] Verify filter coexistence with the NGINX gzip, gunzip, copy, chunked, and
      range filters used by supported builds.
- [ ] Define how third-party filter-module combinations are classified: tested,
      best-effort, or unsupported.

### 6. Supply-chain and release controls

- [ ] Add dependency advisory and dependency-license checks.
- [ ] Pin and document the `ngx`/`nginx-sys` update policy because their API and
      generated ABI surface are not yet treated as stable.
- [ ] Generate an SBOM or equivalent dependency inventory for release artifacts.
- [ ] Add `CHANGELOG.md`, release notes, a version/tag procedure, and rollback
      instructions.
- [ ] Add `SECURITY.md` before making the repository public.

## Important follow-up work

These items strengthen the release but may be deferred when the v0.1 support
contract explicitly excludes them.

- [ ] Run the existing fuzz targets on a schedule and retain a small regression
      corpus for every discovered failure.
- [ ] Preserve benchmark inputs, machine/toolchain metadata, raw results, and the
      rationale for the `fast`, `balanced`, and `max` defaults.
- [ ] Add broader MIME/content-size and incompressible-data performance coverage.
- [ ] Decide whether HTTP/3 interoperability belongs in v0.1; it is currently
      deferred and must not be implied by transport-agnostic filter logic.
- [ ] Add contributor guidance and a code of conduct before accepting public
      contributions.

## Proposed v0.1 exit criteria

The first release can be called production-ready only when:

1. all required CI checks pass from a clean checkout;
2. every advertised NGINX build signature passes integration, edge, reload, and
   diagnostic memory-safety testing;
3. the dynamic/static support boundary is explicit and matches the artifacts;
4. panic and operational failure paths are visible in NGINX logs;
5. dependency, license, SBOM, checksum, changelog, and rollback material is
   attached to the release; and
6. the installation guide has been followed successfully against at least one
   clean target environment outside the development checkout.

## Decisions for the next review

1. Source-only v0.1 or per-signature binary artifacts?
2. Which NGINX distributions, versions, architectures, and configure signatures
   form the initial support matrix?
3. Is static-module support excluded from v0.1, or documented as partial?
4. Which diagnostic memory-safety toolchain is practical for the NGINX build?
5. Which CI runner architecture should be required before making the repository
   public?
