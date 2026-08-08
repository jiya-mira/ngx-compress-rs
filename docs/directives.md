# Directive reference

This is the normative reference for every `ngx-compress-rs` configuration
directive: its syntax, default, valid context, accepted values, and behaviour.
For the design rationale behind the naming and precedence, see
[design.md §4](design.md#4-content-negotiation-and-server-priority).

All directives are valid in the `http`, `server`, and `location` contexts and
follow NGINX inheritance: a child context inherits its parent's value and an
explicit setting in the child overrides it.

## Master switch and profiles

`compress` is both the master gate and the profile selector. It is the only
directive you need for a working setup — `compress balanced;` enables the
compiled-in codecs at a sensible tier.

| Directive | Syntax | Default | Description |
| --- | --- | --- | --- |
| `compress` | `off \| on \| fast \| balanced` | `off` | Master switch and profile selector (see below). |

`compress` values:

| Value | Meaning |
| --- | --- |
| `off` | Module disabled (default). |
| `on` | Enabled, **custom** mode — no preset; only explicit `compress_*` directives and built-in defaults apply. |
| `fast` | Preset: high-QPS dynamic content, CPU-frugal. |
| `balanced` | Preset: general-purpose default. |

Profile presets (the tier *names* are stable; the numeric values may be
re-tuned by later evidence without being a breaking change):

| Tier | Enables | gzip | brotli | zstd | `min_length` |
| --- | --- | --- | --- | --- | --- |
| `fast` | gzip, br, zstd | level 4 | level 4, window 18 | level 3 | 256 |
| `balanced` | gzip, br, zstd | level 6 | level 5, window 22 | level 6 | 256 |

A preset only enables codecs compiled into the build. `deflate` is never enabled
by a preset — turn it on explicitly if a client needs raw deflate.

## Per-codec toggles

| Directive | Syntax | Default | Description |
| --- | --- | --- | --- |
| `compress_gzip` | `on \| off` | `off` | Enable the gzip codec. |
| `compress_deflate` | `on \| off` | `off` | Enable the raw deflate codec. |
| `compress_brotli` | `on \| off` | `off` | Enable the Brotli (`br`) codec. |
| `compress_zstd` | `on \| off` | `off` | Enable the Zstandard (`zstd`) codec. |

## Per-codec parameters

Compression-level scales differ per codec, so each has its own directive with the
codec's native range. Levels are validated at configuration time against the
codec's range; an out-of-range value is a configuration error, not a clamp.

| Directive | Syntax | Range | Default | Description |
| --- | --- | --- | --- | --- |
| `compress_gzip_comp_level` | `level` | 1–9 | 6 | gzip compression level. |
| `compress_deflate_comp_level` | `level` | 1–9 | 6 | deflate compression level. |
| `compress_brotli_comp_level` | `level` | 0–11 | 6 | Brotli quality level. |
| `compress_brotli_window` | `size` | 1k–16m | 512k | Brotli sliding-window size. |
| `compress_zstd_comp_level` | `level` | 1–22 (negative fast levels allowed) | 3 | Zstandard compression level. |

## Shared parameters

These apply to all enabled runtime codecs.

| Directive | Syntax | Default | Description |
| --- | --- | --- | --- |
| `compress_types` | `mime-type ...` | `text/html` (always), plus `text/*`, `application/json`, `application/javascript`, … | MIME allowlist for runtime compression. `text/html` is always included; `*` matches all types. |
| `compress_min_length` | `length` | 20 | Minimum response size (bytes) eligible for runtime compression; applied only when Content-Length is known. 256+ is recommended. |
| `compress_vary` | `on \| off` | `on` | Add `Vary: Accept-Encoding`. On by default so shared caches record that the module serves different encodings. |
| `compress_buffers` | `number size` | `16 8k` | Hard per-request output-buffer count and buffer size. When all buffers are downstream-busy, compression resumes after NGINX reclaims one. |
| `compress_priority` | `coding ...` | profile-dependent | Server tie-break order for equally preferred acceptable codings. |

`compress_priority` accepts each of `zstd`, `br`, `gzip`, and `deflate` at most
once. The configured list is a priority prefix; omitted codings are appended in
the active profile's default order. `identity` is always an implicit final
candidate and cannot be configured. A child value replaces the inherited value
as a whole.

Default equal-quality order:

| Profile | Order |
| --- | --- |
| `fast` | `zstd` > `gzip` > `br` > `deflate` > `identity` |
| `on`, `balanced` | `zstd` > `br` > `gzip` > `deflate` > `identity` |

Client quality values are authoritative. The module first removes unavailable
and `q=0` representations, then selects the highest remaining quality, and only
uses `compress_priority` to break an equal-quality tie. If `identity` has a
higher quality than every enabled coding, the response is left uncompressed. An
eligible module response with no acceptable coding and unacceptable identity is
rejected with `406 Not Acceptable`. A missing or empty `Accept-Encoding` is
handled conservatively as identity.

Per-codec `types`, `min_length`, and buffer overrides are not registered in
v0.1; the per-codec controls are enablement, level, and the Brotli window.

## Precompressed static

`compress_static` serves a precompressed sidecar file (`<file>.zst`,
`<file>.br`, `<file>.gz`) when one exists next to the requested file, instead of
compressing at runtime. It is independent of the runtime `compress` switch.

| Directive | Syntax | Default | Description |
| --- | --- | --- | --- |
| `compress_static` | `off \| on \| always` | `off` | Serve precompressed sidecars (see below). |

`compress_static` values:

| Value | Meaning |
| --- | --- |
| `off` | Never serve sidecars (default). |
| `on` | Serve a sidecar only when the client accepts that coding. |
| `always` | Serve the highest-priority existing sidecar even without a matching `Accept-Encoding` (e.g. behind a decompressing proxy). This explicitly bypasses negotiation and emits a configuration warning. |

With `on`, sidecars are ranked by client quality and then `compress_priority`,
serving the first existing representation that is at least as preferred as
identity. With `always`, sidecars are probed only in server-priority order and
the client's header is ignored; use it only behind an intermediary known to
decode the response. Sidecars are served with the matching
`Content-Encoding`, `Last-Modified`, and `ETag`. deflate has no conventional
sidecar extension and is not probed. When both runtime `compress` and
`compress_static` are on, serving a sidecar sets `Content-Encoding` before the
body filters so the runtime compressor skips the response (no double compression).

## Precedence

Effective values are resolved as:

**explicit `compress_*` directive > profile preset > built-in default**,

independent of directive order. For example, `compress balanced; compress_zstd off;`
runs the `balanced` tier with zstd disabled. The former `max` profile is rejected
as an invalid configuration; use explicit per-codec levels for offline tuning.

## Coexistence with built-in gzip

Do not enable runtime `compress` in an effective location that also inherits or
sets built-in `gzip on`. During `nginx -t`, startup, and reload the module emits
one warning per conflicting effective configuration and, at request time, fails
closed for that location: it creates no codec, changes no response header or
body, and declines sidecars. The built-in gzip filter remains authoritative. A
child `gzip off` removes the conflict for that child. `compress off` with only
`compress_static on` is not a conflict.
