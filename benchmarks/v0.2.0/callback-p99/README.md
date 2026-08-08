# v0.2.0 bounded-callback calibration

The `bench/src/bin/callback_p99.rs` harness runs each profile codec through the
production 64 KiB input / 32-step `WorkBudget`. It discards one warm-up request,
then records every callback across 200 approximately 1.3 MiB requests. Values
below are release builds over the embedded mixed text corpus.

## Local Apple Silicon ARM64

| Profile | Coding | Callbacks | p99 ms | Max ms | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| fast | gzip | 4200 | 0.089 | 0.618 | pass |
| fast | br | 4200 | 0.151 | 0.779 | pass |
| fast | zstd | 4200 | 0.036 | 0.466 | pass |
| balanced | gzip | 4200 | 0.068 | 1.236 | pass |
| balanced | br | 4200 | 0.479 | 52.155 | pass |
| balanced | zstd | 4200 | 0.094 | 0.384 | pass |

The isolated maximum is retained rather than filtered; the release gate is p99.

## `oc.ams` ARM64 canary

- Host: Ubuntu 24.04 ARM64, NGINX 1.30.4.
- Candidate source: `068711b26ace`.
- NGINX was rebuilt with the production configure arguments, with only source
  paths adjusted to the isolated copies.
- Canary listener: `127.0.0.1:18082`; it was stopped and all canary files were
  removed after validation. The production binary and 80/443/8055 listeners
  were not replaced or reloaded.

| Profile | Coding | Callbacks | p99 ms | Max ms | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| fast | gzip | 4200 | 0.048 | 0.103 | pass |
| fast | br | 4200 | 0.522 | 0.656 | pass |
| fast | zstd | 4200 | 0.042 | 0.065 | pass |
| balanced | gzip | 4200 | 0.050 | 0.158 | pass |
| balanced | br | 4200 | 0.413 | 1.750 | pass |
| balanced | zstd | 4200 | 0.121 | 0.139 | pass |

The same canary decoded identity/gzip/br/zstd to the source bytes and verified
HEAD, ETag conditional requests, 64 requests at concurrency 16,
`Server-Timing`, and `compress_buffers 1 1k` continuation.

## Remaining exact-commit evidence

The GitHub-hosted x86_64 and ARM64 jobs cannot run until the candidate is pushed.
They remain a prerelease gate; these local/canary results do not substitute for
the exact-commit GitHub matrix.
