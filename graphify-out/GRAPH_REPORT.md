# Graph Report - ngx-compress-rs  (2026-07-21)

## Corpus Check
- 18 files · ~8,007 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 130 nodes · 169 edges · 15 communities (14 shown, 1 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `2df7399c`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Accept-Encoding Negotiation
- NGINX Module & Filters
- Progress Contract
- Identity Codec Tests
- FFI Filter Chain
- Module Configuration
- Codec Trait Seam
- NGINX Rust Build Helper
- Docker Build & Smoke
- Docker Lint Runner
- 6. Local testing plan
- Result

## God Nodes (most connected - your core abstractions)
1. `stepped()` - 9 edges
2. `validate_progress()` - 9 edges
3. `Design` - 8 edges
4. `ContentCoding` - 7 edges
5. `5. Configuration schema` - 7 edges
6. `6. Local testing plan` - 7 edges
7. `AcceptEncoding` - 6 edges
8. `Identity` - 5 edges
9. `CompressConfig` - 5 edges
10. `Module` - 5 edges

## Surprising Connections (you probably didn't know these)
- `Identity` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/identity.rs → crates/ngx-compress-core/src/codec.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`
- 1-file cycle: `crates/ngx-compress-module/src/http.rs -> crates/ngx-compress-module/src/http.rs`

## Communities (15 total, 1 thin omitted)

### Community 0 - "Accept-Encoding Negotiation"
Cohesion: 0.23
Nodes (11): absent_header_selects_identity_only(), AcceptEncoding, client_quality_overrides_server_order(), ContentCoding, dictionary_coding_requires_server_eligibility(), duplicate_coding_keeps_highest_quality(), explicit_exclusion_overrides_wildcard(), negotiates_common_browser_header_without_allocation() (+3 more)

### Community 1 - "NGINX Module & Filters"
Cohesion: 0.15
Nodes (14): c_char, c_void, body_filter(), header_filter(), Module, ngx_chain_t, ngx_http_request_t, ngx_int_t (+6 more)

### Community 2 - "Progress Contract"
Cohesion: 0.23
Nodes (11): StreamingCodec, accepts_empty_completed_flush(), accepts_waiting_for_input_when_none_is_available(), Operation, ProgressError, rejects_continue_loop_without_progress(), rejects_false_output_backpressure(), Result (+3 more)

### Community 3 - "Identity Codec Tests"
Cohesion: 0.21
Nodes (9): continue_requests_more_input_after_draining(), empty_finish_completes_without_output(), Identity, passes_bytes_through_and_finishes(), signals_output_backpressure_when_capacity_is_short(), stepped(), Operation, StepResult (+1 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.25
Nodes (6): CompressConfig, Option, Self, Merge, MergeConfigError, Result

### Community 6 - "Codec Trait Seam"
Cohesion: 0.11
Nodes (18): 1. Filter architecture, 2. Crate structure, 3. Safety and coexistence policies, 4. Content negotiation and server priority, 6. Local testing plan, 7. Deferred / open items, Body filter, Design (+10 more)

### Community 7 - "NGINX Rust Build Helper"
Cohesion: 0.40
Nodes (3): rust script, ngx_rust_make_module(), ngx_rust_make_modules()

### Community 8 - "Docker Build & Smoke"
Cohesion: 0.53
Nodes (5): log(), no_proxy, build-and-test.sh script, smoke(), write_conf()

### Community 9 - "Docker Lint Runner"
Cohesion: 0.40
Nodes (4): NGINX_BUILD_DIR, NGINX_SOURCE_DIR, no_proxy, lint.sh script

### Community 13 - "6. Local testing plan"
Cohesion: 0.29
Nodes (7): 4.1 Naming: prefix to avoid collisions, 4.2 Master switch and per-codec toggles, 4.3 Per-codec parameters, 4.4 Shared parameters (global default, per-codec override), 4.5 Typed configuration model, 4.6 Example, 5. Configuration schema

## Knowledge Gaps
- **25 isolated node(s):** `Header filter`, `Body filter`, `Layout`, `Rationale`, `When a codec earns its own crate` (+20 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **1 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Identity` connect `Identity Codec Tests` to `Progress Contract`?**
  _High betweenness centrality (0.057) - this node is a cross-community bridge._
- **Why does `ContentCoding` connect `Accept-Encoding Negotiation` to `Identity Codec Tests`?**
  _High betweenness centrality (0.037) - this node is a cross-community bridge._
- **Why does `StreamingCodec` connect `Progress Contract` to `Identity Codec Tests`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._
- **What connects `Header filter`, `Body filter`, `Layout` to the rest of the system?**
  _25 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.10526315789473684 - nodes in this community are weakly interconnected._