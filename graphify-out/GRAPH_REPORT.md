# Graph Report - ngx-compress-rs  (2026-07-21)

## Corpus Check
- 34 files · ~16,299 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 337 nodes · 510 edges · 55 communities (26 shown, 29 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `6ebca37e`
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
- identity.rs
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
- NonNull
- Option
- Result
- Operation
- StepResult
- StepState
- q_str
- README.md
- c_void
- ngx_array_t
- ngx_int_t
- ngx_str_t
- NonNull
- Option
- Result
- SystemBrotli
- test-nginx.sh

## God Nodes (most connected - your core abstractions)
1. `compress_chain()` - 13 edges
2. `edge-tests.sh script` - 10 edges
3. `Design` - 9 edges
4. `Gzip` - 9 edges
5. `Resolved` - 9 edges
6. `SystemBrotli` - 9 edges
7. `stepped()` - 9 edges
8. `validate_progress()` - 9 edges
9. `compressible()` - 8 edges
10. `CompressConfig` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Brotli` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/brotli_codec.rs → crates/ngx-compress-core/src/codec.rs
- `Zstd` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/zstd_codec.rs → crates/ngx-compress-core/src/codec.rs
- `available()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/select.rs → crates/ngx-compress-module/src/conf.rs
- `build()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/select.rs → crates/ngx-compress-module/src/conf.rs
- `choose()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/select.rs → crates/ngx-compress-module/src/conf.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`

## Communities (55 total, 29 thin omitted)

### Community 0 - "Accept-Encoding Negotiation"
Cohesion: 0.23
Nodes (11): absent_header_selects_identity_only(), AcceptEncoding, client_quality_overrides_server_order(), ContentCoding, dictionary_coding_requires_server_eligibility(), duplicate_coding_keeps_highest_quality(), explicit_exclusion_overrides_wildcard(), negotiates_common_browser_header_without_allocation() (+3 more)

### Community 1 - "NGINX Module & Filters"
Cohesion: 0.15
Nodes (12): directive(), Module, multi(), ngx_command_t, ngx_conf_t, ngx_int_t, ngx_str_t, HttpModule (+4 more)

### Community 2 - "Progress Contract"
Cohesion: 0.26
Nodes (10): accepts_empty_completed_flush(), accepts_waiting_for_input_when_none_is_available(), Operation, ProgressError, rejects_continue_loop_without_progress(), rejects_false_output_backpressure(), Result, StepResult (+2 more)

### Community 3 - "Identity Codec Tests"
Cohesion: 0.25
Nodes (11): Identity, ContentCoding, brotli_roundtrips_across_buffer_sizes(), compress(), deflate_roundtrips_across_buffer_sizes(), gzip_handles_empty_input(), gzip_roundtrips_across_buffer_sizes(), Vec (+3 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.15
Nodes (22): c_char, c_void, apply(), CompressConfig, merge_opt(), on_level(), Result, Self (+14 more)

### Community 6 - "Codec Trait Seam"
Cohesion: 0.07
Nodes (26): 1. Filter architecture, 2. Crate structure, 3. Safety and coexistence policies, 4.1 Naming: our own scheme, not an upstream drop-in, 4.2 Master switch and per-codec toggles, 4.3 Per-codec parameters, 4.4 Shared parameters (global default, per-codec override), 4.5 Typed configuration model (+18 more)

### Community 7 - "NGINX Rust Build Helper"
Cohesion: 0.40
Nodes (3): rust script, ngx_rust_make_module(), ngx_rust_make_modules()

### Community 8 - "Docker Build & Smoke"
Cohesion: 0.33
Nodes (10): check(), check_identity(), check_ssi(), decode(), log(), no_proxy, setup_www(), build-and-test.sh script (+2 more)

### Community 9 - "Docker Lint Runner"
Cohesion: 0.40
Nodes (4): NGINX_BUILD_DIR, NGINX_SOURCE_DIR, no_proxy, lint.sh script

### Community 13 - "6. Local testing plan"
Cohesion: 0.11
Nodes (20): CodecError, Compress, ContentCoding, copy_into(), Deflate, delta(), derive_state(), FlateCore (+12 more)

### Community 15 - "Brotli"
Cohesion: 0.18
Nodes (11): BrotliEncoderOperation, BrotliEncoderStateStruct, Brotli, build_state(), map_operation(), ContentCoding, Operation, Result (+3 more)

### Community 16 - "Zstd"
Cohesion: 0.17
Nodes (10): drain_state(), ContentCoding, Operation, Result, Self, StepResult, StepState, Zstd (+2 more)

### Community 17 - "identity.rs"
Cohesion: 0.24
Nodes (9): continue_requests_more_input_after_draining(), empty_finish_completes_without_output(), passes_bytes_through_and_finishes(), Operation, Result, StepResult, Vec, signals_output_backpressure_when_capacity_is_short() (+1 more)

### Community 18 - "Operation"
Cohesion: 0.15
Nodes (30): AcceptEncoding, Box, accept_encoding(), advance(), append(), body_filter(), builtin_compressible(), clear_content_length() (+22 more)

### Community 19 - "StepResult"
Cohesion: 0.30
Nodes (11): check_backpressure(), check_disconnect(), check_http2(), check_no_panic(), check_truncated_upstream(), log(), no_proxy, setup() (+3 more)

### Community 21 - "Resolved"
Cohesion: 0.38
Nodes (10): C, Resolved, available(), boxed(), build(), choose(), AcceptEncoding, Box (+2 more)

### Community 34 - "verify-backends.sh"
Cohesion: 0.50
Nodes (3): no_proxy, verify-backends.sh script, smoke()

### Community 53 - "SystemBrotli"
Cohesion: 0.23
Nodes (8): create(), EncoderOperation, EncoderParameter, map_operation(), SystemBrotli, Drop, EncoderState, Self

### Community 54 - "test-nginx.sh"
Cohesion: 0.50
Nodes (3): no_proxy, test-nginx.sh script, TEST_NGINX_BINARY

## Knowledge Gaps
- **33 isolated node(s):** `test-nginx.sh script`, `no_proxy`, `TEST_NGINX_BINARY`, `no_proxy`, `no_proxy` (+28 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **29 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `StreamingCodec` connect `Identity Codec Tests` to `Zstd`, `Progress Contract`, `Brotli`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._
- **Why does `CodecError` connect `Zstd` to `identity.rs`, `Progress Contract`, `Brotli`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._
- **Why does `operation_for()` connect `Operation` to `6. Local testing plan`?**
  _High betweenness centrality (0.018) - this node is a cross-community bridge._
- **What connects `test-nginx.sh script`, `no_proxy`, `TEST_NGINX_BINARY` to the rest of the system?**
  _33 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.07407407407407407 - nodes in this community are weakly interconnected._
- **Should `6. Local testing plan` be split into smaller, more focused modules?**
  _Cohesion score 0.11092436974789915 - nodes in this community are weakly interconnected._