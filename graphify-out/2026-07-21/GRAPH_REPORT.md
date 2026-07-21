# Graph Report - ngx-compress-rs  (2026-07-21)

## Corpus Check
- 26 files · ~13,025 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 288 nodes · 452 edges · 35 communities (20 shown, 15 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `806f5514`
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
- Brotli
- Zstd
- compress
- Operation
- StepResult
- Vec
- Resolved
- c_void
- ngx_chain_t
- ngx_http_request_t
- ngx_int_t
- Option
- Self
- ngx_command_t
- ngx_conf_t
- Result
- AcceptEncoding
- Box
- StreamingCodec
- verify-backends.sh

## God Nodes (most connected - your core abstractions)
1. `compress_chain()` - 12 edges
2. `Resolved` - 10 edges
3. `Gzip` - 9 edges
4. `stepped()` - 9 edges
5. `validate_progress()` - 9 edges
6. `CompressConfig` - 8 edges
7. `compressible()` - 8 edges
8. `choose()` - 8 edges
9. `build()` - 8 edges
10. `Brotli` - 8 edges

## Surprising Connections (you probably didn't know these)
- `eligible()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/filter.rs → crates/ngx-compress-module/src/conf.rs
- `Brotli` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/brotli_codec.rs → crates/ngx-compress-core/src/codec.rs
- `Deflate` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/flate.rs → crates/ngx-compress-core/src/codec.rs
- `Gzip` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/flate.rs → crates/ngx-compress-core/src/codec.rs
- `Zstd` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/zstd_codec.rs → crates/ngx-compress-core/src/codec.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`

## Communities (35 total, 15 thin omitted)

### Community 0 - "Accept-Encoding Negotiation"
Cohesion: 0.23
Nodes (11): absent_header_selects_identity_only(), AcceptEncoding, client_quality_overrides_server_order(), ContentCoding, dictionary_coding_requires_server_eligibility(), duplicate_coding_keeps_highest_quality(), explicit_exclusion_overrides_wildcard(), negotiates_common_browser_header_without_allocation() (+3 more)

### Community 1 - "NGINX Module & Filters"
Cohesion: 0.15
Nodes (12): directive(), Module, multi(), ngx_command_t, ngx_conf_t, ngx_int_t, ngx_str_t, HttpModule (+4 more)

### Community 2 - "Progress Contract"
Cohesion: 0.33
Nodes (10): accepts_empty_completed_flush(), accepts_waiting_for_input_when_none_is_available(), Operation, ProgressError, rejects_continue_loop_without_progress(), rejects_false_output_backpressure(), Result, StepResult (+2 more)

### Community 3 - "Identity Codec Tests"
Cohesion: 0.24
Nodes (9): continue_requests_more_input_after_draining(), empty_finish_completes_without_output(), passes_bytes_through_and_finishes(), Operation, Result, StepResult, Vec, signals_output_backpressure_when_capacity_is_short() (+1 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.15
Nodes (23): c_char, apply(), CompressConfig, merge_opt(), on_level(), c_void, ngx_array_t, ngx_command_t (+15 more)

### Community 6 - "Codec Trait Seam"
Cohesion: 0.08
Nodes (25): 1. Filter architecture, 2. Crate structure, 3. Safety and coexistence policies, 4.1 Naming: prefix to avoid collisions, 4.2 Master switch and per-codec toggles, 4.3 Per-codec parameters, 4.4 Shared parameters (global default, per-codec override), 4.5 Typed configuration model (+17 more)

### Community 7 - "NGINX Rust Build Helper"
Cohesion: 0.40
Nodes (3): rust script, ngx_rust_make_module(), ngx_rust_make_modules()

### Community 8 - "Docker Build & Smoke"
Cohesion: 0.36
Nodes (9): check(), check_identity(), decode(), log(), no_proxy, setup_www(), build-and-test.sh script, smoke() (+1 more)

### Community 9 - "Docker Lint Runner"
Cohesion: 0.40
Nodes (4): NGINX_BUILD_DIR, NGINX_SOURCE_DIR, no_proxy, lint.sh script

### Community 13 - "6. Local testing plan"
Cohesion: 0.11
Nodes (19): Compress, copy_into(), Deflate, delta(), derive_state(), FlateCore, FlateStep, Gzip (+11 more)

### Community 15 - "Brotli"
Cohesion: 0.18
Nodes (11): BrotliEncoderOperation, BrotliEncoderStateStruct, Brotli, build_state(), map_operation(), ContentCoding, Operation, Result (+3 more)

### Community 16 - "Zstd"
Cohesion: 0.18
Nodes (9): drain_state(), ContentCoding, Operation, Result, Self, StepResult, StepState, Zstd (+1 more)

### Community 17 - "compress"
Cohesion: 0.25
Nodes (11): Identity, ContentCoding, brotli_roundtrips_across_buffer_sizes(), compress(), deflate_roundtrips_across_buffer_sizes(), gzip_handles_empty_input(), gzip_roundtrips_across_buffer_sizes(), Vec (+3 more)

### Community 18 - "Operation"
Cohesion: 0.13
Nodes (33): AcceptEncoding, Box, accept_encoding(), advance(), append(), body_filter(), builtin_compressible(), cleanup() (+25 more)

### Community 21 - "Resolved"
Cohesion: 0.35
Nodes (11): C, ContentCoding, Resolved, available(), boxed(), build(), choose(), AcceptEncoding (+3 more)

### Community 34 - "verify-backends.sh"
Cohesion: 0.50
Nodes (3): no_proxy, verify-backends.sh script, smoke()

## Knowledge Gaps
- **26 isolated node(s):** `no_proxy`, `no_proxy`, `Header filter`, `Body filter`, `Layout` (+21 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **15 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `StreamingCodec` connect `compress` to `Zstd`, `6. Local testing plan`, `Brotli`?**
  _High betweenness centrality (0.082) - this node is a cross-community bridge._
- **Why does `CodecError` connect `6. Local testing plan` to `Zstd`, `Identity Codec Tests`, `Brotli`?**
  _High betweenness centrality (0.056) - this node is a cross-community bridge._
- **Why does `Brotli` connect `Brotli` to `compress`?**
  _High betweenness centrality (0.027) - this node is a cross-community bridge._
- **What connects `no_proxy`, `no_proxy`, `Header filter` to the rest of the system?**
  _26 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Module Configuration` be split into smaller, more focused modules?**
  _Cohesion score 0.1476923076923077 - nodes in this community are weakly interconnected._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.07692307692307693 - nodes in this community are weakly interconnected._
- **Should `6. Local testing plan` be split into smaller, more focused modules?**
  _Cohesion score 0.11229946524064172 - nodes in this community are weakly interconnected._