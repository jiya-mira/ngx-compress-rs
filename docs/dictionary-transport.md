# Compression Dictionary Transport design direction

Status: planning record for the post-v0.1 RFC 9842 milestone. This is not part
of the current support contract. Revalidate the operational details,
interoperability, and resource limits before implementation, but do not reopen
the settled user-facing principles without new evidence.

## Goals

- Implement RFC 9842 completely, including both `dcb` and `dcz`, rather than
  publishing a static-only protocol subset.
- Make dictionary transport useful without requiring users to understand
  dictionary hashes, IDs, public URLs, manifests, or version naming.
- Let NGINX configuration define the safe collection boundary while the module
  manages dictionary generation, immutable generations, advertisement, lookup,
  and fallback.
- Never block a request while waiting for a dictionary or a dictionary-compressed
  representation to be generated.

## One directive

The intended configuration surface is one inherited directive:

```nginx
compress_dictionary off;
compress_dictionary lazy;
compress_dictionary /path/to/dictionary.bin;
```

It is valid in `http`, `server`, and `location` contexts and follows the normal
parent-to-child merge cascade.

| Value | Meaning |
| --- | --- |
| `off` | Disable dictionary transport in this scope; also overrides an inherited mode. |
| `lazy` | Enable automatic collection, asynchronous generation, advertisement, and generation management. |
| file path | Use an externally generated raw dictionary. The module computes its identity and exposes it through an internal same-origin content-addressed URL. |

`lazy` is the single reserved automatic-mode keyword. Do not expose separate
`dynamic`, group-name, manifest, dictionary-ID, dictionary-URL, or version-name
directives unless implementation evidence proves that an additional operator
decision is unavoidable.

## Configuration scope is not dictionary identity

An `http`-level directive enables the policy throughout its inherited scope; it
does not create one global dictionary. RFC origin isolation and the effective
NGINX location provide the primary internal partition:

```text
origin + stable effective-location identity = dictionary manager
```

For example:

```nginx
http {
    compress_dictionary lazy;

    server {
        location /private/ {
            compress_dictionary off;
        }

        location /special-api/ {
            compress_dictionary /etc/nginx/special-api.dict;
        }
    }
}
```

The inherited `lazy` mode creates independent managers for eligible locations.
The private location collects nothing, and the special API location uses the
external dictionary instead of its inherited lazy manager. No user-visible
manager or dictionary name is required.

A manager may later partition by request information known before the response,
such as a representable URL sub-pattern or `match-dest`. It must not create
overlapping dictionaries based only on response MIME when the browser cannot
make the same choice before receiving the response.

## Compile configuration into a collection plan

During configuration finalization, walk the merged `http`/`server`/`location`
tree and compile a Rust-owned collection plan. The request path should look up
an already-resolved plan rather than reinterpret the configuration.

The plan combines:

- the inherited `compress_dictionary` mode;
- origin and effective location;
- the existing compression MIME allow-list and minimum length;
- response eligibility and future `compress_proxied` policy;
- cacheability and security exclusions;
- a safe RFC URLPattern representation for browser matching.

Prefix and exact locations have a direct path to a safe match pattern. Regex,
named, rewritten, and otherwise non-representable locations need an explicit
design decision before implementation; do not broaden them silently to `/*`.

## Lazy collection and materialization

Each lazy manager starts in an observation state. It collects only bounded,
Rust-owned samples from responses that are suitable for shared compression.
At minimum, automatic collection excludes personalized, uncontrolled, private,
or non-cacheable content, including responses dependent on authorization or
carrying `private`, `no-store`, or `Set-Cookie`.

Dictionary readiness and encoded-representation readiness are separate:

1. Before a dictionary is ready, serve ordinary `br`, `zstd`, `gzip`, or
   `identity` and do not advertise a dictionary.
2. Generate the dictionary asynchronously from eligible samples.
3. Only after the dictionary is complete and atomically published, expose its
   content-addressed URL and the RFC discovery/usage headers.
4. When a client advertises that dictionary but a matching `dcb`/`dcz`
   representation is not ready, serve an ordinary encoding and enqueue
   generation without delaying the request.
5. Serve the generated representation on later matching requests.

Concurrent workers must use single-flight generation and atomic publication so
they do not duplicate work or expose partial artifacts.

## Per-location lifecycle policy

Every location manager independently chooses one of three outcomes:

```text
observe -> no useful dictionary
        -> generate once and freeze
        -> maintain progressive immutable generations
```

Generate-once is appropriate when the sampled population is stable, immutable,
or naturally tied to a deployment/reload epoch. Progressive generation is
appropriate when the content remains structurally similar but its distribution
changes without a corresponding NGINX reload.

The decision uses observed evidence rather than another user directive:

- response and strong-ETag/content-hash churn;
- similarity between new samples and the active dictionary;
- measured `dcb`/`dcz` benefit relative to ordinary compression;
- sufficient new sample weight;
- hysteresis that prevents frequent low-value rebuilding.

Low-volume, heterogeneous, unsafe, or low-benefit locations remain in the
no-dictionary state even when they inherit `lazy`.

## Immutable generations and reloads

A progressive update creates a new immutable dictionary; it never mutates bytes
under an existing identity:

```text
generation A -> SHA-256 A
generation B -> SHA-256 B
generation C -> SHA-256 C
```

New responses may recommend B while clients that still advertise A continue to
receive A-based representations. Old generations remain available for their
cache lifetime or a stricter bounded retention policy, then expire.

Use a stable fingerprint of origin plus effective location semantics to decide
whether a manager can reuse persisted artifacts across reloads. A changed
collection boundary creates a new manager generation. If an old dictionary is
not available after reload, the request falls back to an ordinary encoding;
the module never guesses from an ID without validating the advertised hash.

## Internal storage and identity

Dictionary and representation artifacts are internal implementation details:

- dictionaries are addressed by SHA-256;
- generated dictionary URLs are same-origin and content-addressed;
- `Dictionary-ID`, when used, is generated by the module and never replaces
  hash validation;
- a representation key includes resource identity, dictionary hash, coding,
  and encoder parameters;
- storage, active generations, queued work, and copied sample bytes are bounded.

This scoped artifact store is part of the dictionary subsystem, not a
general-purpose dynamic response cache and not a replacement for
`proxy_cache`.

## Dependencies and open implementation questions

The asynchronous execution milestone is a prerequisite for lazy generation. No
NGINX request, pool, buffer, chain, or borrowed header pointer may cross into a
compression or dictionary-generation thread.

Revisit these questions before implementation:

- NGINX thread pools versus a narrowly owned module executor;
- multi-worker coordination, crash recovery, and persistent artifact layout;
- integration with proxied/static responses without depending on private
  `proxy_cache` structures;
- dictionary construction algorithms, bounded sampling, benefit estimation,
  and safe default resource limits;
- representable URLPattern derivation for complex location configurations;
- cache freshness, old-generation retention, and reload behavior;
- browser interoperability and failure behavior across HTTP/1.1, HTTP/2, and
  HTTP/3.

## Explicit non-goals

- a generic public fallback dictionary;
- user-named dictionary groups or versions;
- in-place mutation of an advertised dictionary;
- holding the current request open for background generation;
- collecting sensitive or user-specific response material;
- publishing a static-only `dcb`/`dcz` feature as the production milestone.

## Standards and implementation references

- [RFC 9842: Compression Dictionary Transport](https://www.rfc-editor.org/rfc/rfc9842.html)
- [Cloudflare shared-dictionary lifecycle direction](https://blog.cloudflare.com/shared-dictionaries/)
- [Google Search dictionary deployment](https://developer.chrome.com/blog/search-compression-dictionaries)
