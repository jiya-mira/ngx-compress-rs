# Migrating to v0.2.0

v0.2.0 changes negotiation and streaming behavior. Validate configuration and
decoded response bytes in a canary before replacing an earlier build.

## Required configuration changes

- Remove `compress max;`. The value is now a configuration error. Use `fast`,
  `balanced`, or explicit per-codec levels.
- Check any equal-quality server preference. The default is
  `zstd > br > gzip > deflate > identity`, except `fast`, which uses
  `zstd > gzip > br > deflate > identity`. Use `compress_priority` only for an
  explicit tie-break prefix; it never overrides a client's higher q value.
- Treat `compress_static always` as a deliberate protocol bypass. It now emits
  a strong configuration warning. Prefer `compress_static on` for negotiated
  clients.

## Behavioral changes

- `identity` participates in selection. If it has a higher effective q value,
  the original response is sent. If every module-owned representation,
  including identity, has q=0, the response is `406 Not Acceptable`.
- Missing and empty `Accept-Encoding` conservatively select identity; the
  module does not invent an encoder order for the client.
- `compress_buffers <count> <size>` now enforces both dimensions. A saturated
  output pool yields and resumes after downstream progress instead of allocating
  past `count`.
- Every body-filter callback is capped at 64 KiB of input and 32 codec steps.
  Unconsumed input and flush/finish state resume on a later NGINX event turn.
- Encoded responses clear `Content-Length` and `Accept-Ranges`, weaken strong
  ETags, and materialize file buffers through the native C ABI.

## Optional observability

`compress_stats variables;` enables the six `$compress_*` log variables.
`compress_stats server_timing;` additionally emits one best-effort
`Server-Timing` trailer. The default remains `off`, which avoids timing and byte
accounting.
