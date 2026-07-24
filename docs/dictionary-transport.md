# Compression Dictionary Transport design direction

Status: planning record for the post-v0.1 RFC 9842 milestone. This is not part
of the current support contract. Revalidate the operational details,
interoperability, and resource limits before implementation, but do not reopen
the settled user-facing principles without new evidence.

Scope note: dictionary *sourcing* is deliberately bounded to two safe origins —
lazily generating a dictionary from static, cacheable resources, and loading an
external dictionary file/link. Sampling or training dictionaries from dynamic
responses, and progressively regenerated dictionary generations, are out of scope
(see [Explicit non-goals](#explicit-non-goals)). This narrows an earlier
direction; the reasoning is that dynamic response distributions are hard to
predict, and a service that wants a persistent, valuable dictionary is better
placed to supply one itself.

## Goals

- Implement the RFC 9842 *protocol* completely — both `dcb` and `dcz`,
  `Use-As-Dictionary` / `Available-Dictionary` / `Dictionary-ID` negotiation,
  SHA-256 validation, `Vary`, and ordinary-encoding fallback. Completeness is a
  property of the wire protocol, not of the dictionary-sourcing strategy.
- Bound dictionary *sourcing* to two safe origins: lazily generating one shared
  dictionary from a location's static, cacheable resources, and loading an
  operator- or origin-supplied external dictionary file/link. The module never
  samples or trains dictionaries from dynamic responses.
- Make dictionary transport useful without requiring users to understand
  dictionary hashes, IDs, public URLs, manifests, or version naming.
- Let NGINX configuration define the safe boundary per location while the module
  manages advertisement, content-addressed lookup, generation coexistence, and
  fallback.
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
| `lazy` | Generate one shared dictionary from the static, cacheable resources in this scope: build it asynchronously, advertise it (`Use-As-Dictionary` with the location's match pattern), and serve `dcb`/`dcz` against it. It never samples dynamic responses. |
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
manager or dictionary name is required. This per-location partition holds
whether a manager's dictionary is a lazily-bound static resource or an external
file — the relationship is always owned per location, never globally.

Enabling `lazy` at any scope broader than a single location — `http` or
`server` — is therefore broad but self-limiting: eligible static locations get a
dictionary, while dynamic or non-cacheable locations simply stay in the
no-dictionary state and serve ordinary encodings — the same way `gzip on` at
`http` or `server` acts only on eligible responses. No per-location opt-in is
required, and no configuration knob distinguishes static from dynamic locations.
When the feature ships, state this inherited-scope behavior explicitly in the
user-facing directive documentation, so an operator is not surprised that an
`http`- or `server`-level `lazy` produces dictionaries only where static
resources exist.

A manager may later partition by request information known before the response,
such as a representable URL sub-pattern or `match-dest`. It must not create
overlapping dictionaries based only on response MIME when the browser cannot
make the same choice before receiving the response.

## Compile configuration into a per-location dictionary plan

During configuration finalization, walk the merged `http`/`server`/`location`
tree and compile a Rust-owned dictionary plan. The request path should look up
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

## Lazy static dictionaries and materialization

A `lazy` manager operates only on static, cacheable resources; it never samples
dynamic responses. An eligible resource is cacheable and free of personalized or
sensitive material — never a response that depends on authorization or carries
`private`, `no-store`, or `Set-Cookie`.

The manager generates one high-quality shared dictionary trained from the
location's eligible static resources, rather than trying to use a resource's
"previous version" as its dictionary. Without operator input this layer cannot
locate a previous version: versioned assets have distinct, content-hashed URLs
(`app.4f3a2b.js`), and the mapping from one version to its successor lives in the
build tool and HTML references, not in anything NGINX sees. The RFC "use a prior
response as a dictionary" model is origin-declared; a generic proxy layer cannot
infer it. Training one shared dictionary over the known static corpus is both
realistic to implement autonomously and effective. Training over static content
is bounded and drift-free — distinct from the dynamic-response training that is a
non-goal.

Dictionary readiness and encoded-representation readiness are separate:

1. Before a dictionary is ready, serve ordinary `br`, `zstd`, `gzip`, or
   `identity` and advertise nothing.
2. Train the shared dictionary asynchronously from the eligible static corpus,
   publish it atomically as a content-addressed (SHA-256) same-origin resource,
   and advertise it with `Use-As-Dictionary` for the location's match pattern.
3. When a client returns advertising that dictionary (`Available-Dictionary`)
   for a matching resource whose `dcb`/`dcz` representation is not yet built,
   serve an ordinary encoding and enqueue generation without delaying the
   request.
4. Serve the generated `dcb`/`dcz` representation on later matching requests.

The external-file case is the same flow with step 2's training replaced by the
supplied dictionary; only the per-resource representations are generated on
demand.

Concurrent workers must use single-flight generation and atomic publication so
they do not duplicate work or expose partial artifacts.

## Per-location lifecycle policy

Every location manager independently owns its dictionary relationship, whether
the dictionary is a lazily-bound static resource or an external file. It settles
on one of:

```text
observe -> no useful dictionary
        -> generate one dictionary from the location's static corpus, then freeze
        -> use the configured external dictionary
```

The generated case trains once and freezes; it rebuilds only when the underlying
static corpus changes — for example a redeploy, detected through resource
content-hash/ETag churn — never on a continuous progressive schedule. The
external case is fixed by configuration. There is no *progressive* regeneration
and no drift/benefit retraining loop.

The no-dictionary decision uses simple eligibility rather than another user
directive: low-volume, heterogeneous, unsafe, non-cacheable, or measurably
low-benefit locations remain in the no-dictionary state even when they inherit
`lazy`.

## Immutable identities and reloads

Even without progressive retraining, multiple generations coexist naturally: the
static corpus changes (a redeploy) and its dictionary is regenerated, or an
external dictionary file is replaced. Each is a new immutable, SHA-256-addressed
identity;
bytes under an existing identity are never mutated:

```text
generation A -> SHA-256 A
generation B -> SHA-256 B
```

New responses may recommend B while clients that still advertise A continue to
receive A-based representations. Old generations remain available for their
cache lifetime or a stricter bounded retention policy, then expire.

Use a stable fingerprint of origin plus effective-location semantics to decide
whether a manager can reuse persisted artifacts across reloads. A changed
boundary creates a new manager generation. If an advertised dictionary is not
available after reload, the request falls back to an ordinary encoding; the
module never guesses from an ID without validating the advertised hash.

## Internal storage and identity

Dictionary and representation artifacts are internal implementation details:

- dictionaries are addressed by SHA-256;
- generated dictionary URLs are same-origin and content-addressed;
- `Dictionary-ID`, when used, is generated by the module and never replaces
  hash validation;
- a representation key includes resource identity, dictionary hash, coding,
  and encoder parameters;
- storage, active generations, and queued work are bounded.

This scoped artifact store is part of the dictionary subsystem, not a
general-purpose dynamic response cache and not a replacement for
`proxy_cache`.

## Reverse-proxy and downstream dictionaries

When the origin already produces `dcb`/`dcz`, the module forwards the
negotiation and the compressed response unchanged — it adds no dictionary logic.

When the module itself compresses at the edge and a client advertises a
dictionary (`Available-Dictionary: <sha256>`), it needs the dictionary bytes to
encode. The dictionary resource can be stored by ordinary `proxy_cache` like any
URL, but `proxy_cache` is keyed by URL, so the module still owns a thin
SHA-256-indexed lookup mapping the advertised hash to those bytes for the
encoder. It builds no second cache subsystem and adds no dictionary routing
where an existing NGINX mechanism already serves the dictionary resource.

## Dependencies and open implementation questions

The asynchronous execution milestone is a prerequisite for lazy generation. No
NGINX request, pool, buffer, chain, or borrowed header pointer may cross into a
compression or representation-generation thread.

One design decision is deliberately deferred:

- **Dictionary URL namespace.** RFC 9842 defines dictionary discovery through
  an ordinary same-origin URL, but does not reserve a path or a well-known URI.
  A module-generated standalone dictionary therefore unavoidably consumes
  externally reachable URI space. The current direction is a disabled-by-default,
  module-owned, content-addressed prefix with strict request matching and
  configuration-time conflict detection. This can reduce and expose collisions,
  but cannot make them impossible. Do not commit to a concrete prefix or add a
  user-facing override until implementation research shows which compromise is
  least harmful.

Revisit these questions before implementation:

- NGINX thread pools versus a narrowly owned module executor;
- multi-worker coordination, crash recovery, and persistent artifact layout;
- integration with proxied/static responses without depending on private
  `proxy_cache` structures;
- static-corpus dictionary construction: sampling bounds, the training
  algorithm, corpus-change detection, and benefit estimation before advertising
  a generated dictionary;
- safe default resource limits for stored artifacts and queued generation work;
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
- sampling, training, or generating dictionaries from dynamic responses —
  dynamic use cases are served by an operator- or origin-supplied external
  dictionary (file/link);
- shipping an incomplete protocol as the milestone: a complete implementation
  speaks both `dcb` and `dcz` with full negotiation, validation, and fallback,
  even though dictionary sourcing is bounded to static and external origins.

## Standards and implementation references

- [RFC 9842: Compression Dictionary Transport](https://www.rfc-editor.org/rfc/rfc9842.html)
- [Cloudflare shared-dictionary lifecycle direction](https://blog.cloudflare.com/shared-dictionaries/)
- [Google Search dictionary deployment](https://developer.chrome.com/blog/search-compression-dictionaries)
