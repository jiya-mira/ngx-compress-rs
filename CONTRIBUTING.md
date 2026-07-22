# Contributing to ngx-compress-rs

Thank you for helping improve `ngx-compress-rs`. Bug reports, focused feature
proposals, documentation fixes, tests, and code contributions are welcome.

Participation in this project is governed by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Before opening an issue

- Search existing issues first.
- Use Discussions for questions, early ideas, and design trade-offs that do not
  yet have an accepted implementation scope.
- Use the bug, compatibility-report, or focused feature issue form and include
  the requested context.
- Do not report suspected vulnerabilities in a public issue. Follow
  [SECURITY.md](SECURITY.md) instead.
- Questions about unsupported NGINX versions or platforms are welcome, but the
current support contract remains the one documented in [README.md](README.md).

The prioritization rules and the distinction between committed, feedback-gated,
and proposed work are documented in [docs/roadmap.md](docs/roadmap.md).

## Development setup

The NGINX-independent crates can be checked on the host:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

The FFI and module crates require a configured NGINX source tree. The pinned
Docker integration environment is the reproducible path:

```sh
docker build -t ngx-compress-build:latest -f docker/Dockerfile .
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/build-and-test.sh
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/verify-backends.sh
docker run --rm -v "$PWD:/repo" ngx-compress-build:latest \
  sh /repo/docker/edge-tests.sh
```

HTTP/3, sanitizer, lifecycle, and release checks run in CI. Run their local
harnesses when a change directly affects those paths.

## Engineering expectations

- Keep changes focused and preserve existing configuration compatibility.
- Keep `unsafe` inside the FFI boundary, use the
  `FFI prefetch -> safe Rust core -> typed submit -> FFI` shape, and document
  every unsafe invariant with a nearby `SAFETY` comment.
- Never allow a panic to cross an NGINX callback boundary.
- Preserve explicit streaming progress, flush, finish, and backpressure
  semantics.
- Do not add a dependency or broaden the support contract without explaining
  the need and trade-offs.
- Add a main-path test and at least one relevant failure or boundary test.
- Never commit credentials, private response data, production configuration,
  or unsanitized logs.

Read [docs/architecture.md](docs/architecture.md),
[docs/design.md](docs/design.md), and
[docs/unsafe-boundary-refactor.md](docs/unsafe-boundary-refactor.md) before
changing module integration, filter ordering, or FFI ownership.

## Pull requests

1. Open a focused pull request with a clear motivation and behavioral impact.
2. Describe the validation you ran and any checks you could not run.
3. Update user-facing documentation for configuration or support changes.
4. Keep the branch current while review is active and address review comments
   with additional commits or a clearly explained alternative.

Conventional Commit-style subjects are preferred, but not required for
external contributions. By submitting a contribution, you agree that it may be
licensed under the project's [Apache License 2.0](LICENSE).
