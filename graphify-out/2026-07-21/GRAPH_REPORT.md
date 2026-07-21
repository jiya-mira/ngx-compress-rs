# Graph Report - ngx-compress-rs  (2026-07-21)

## Corpus Check
- 39 files · ~22,993 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 435 nodes · 664 edges · 50 communities (28 shown, 22 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `e17940b1`
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
- compress
- identity.rs
- Operation
- StepResult
- AcceptEncoding
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
- ngx_http_request_t
- verify-backends.sh
- NonNull
- Option
- Result
- ngx_str_t
- StepResult
- StepState
- q_str
- README.md
- c_void
- ngx_array_t
- SystemBrotli
- test-nginx.sh

## God Nodes (most connected - your core abstractions)
1. `compress_chain()` - 13 edges
2. `CompressConfig` - 11 edges
3. `Unsafe boundary refactor (deferred until after M3)` - 10 edges
4. `build()` - 10 edges
5. `edge-tests.sh script` - 10 edges
6. `apply()` - 9 edges
7. `try_serve()` - 9 edges
8. `send_file()` - 9 edges
9. `Design` - 9 edges
10. `5. Configuration schema` - 9 edges

## Surprising Connections (you probably didn't know these)
- `serve()` --calls--> `accept_encoding()`  [INFERRED]
  crates/ngx-compress-module/src/static_file.rs → crates/ngx-compress-module/src/filter.rs
- `eligible()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/filter.rs → crates/ngx-compress-module/src/conf.rs
- `Brotli` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/brotli_codec.rs → crates/ngx-compress-core/src/codec.rs
- `CompressConfig` --references--> `Profile`  [EXTRACTED]
  crates/ngx-compress-module/src/conf.rs → crates/ngx-compress-module/src/profile.rs
- `choose()` --references--> `CodecKey`  [EXTRACTED]
  crates/ngx-compress-module/src/select.rs → crates/ngx-compress-module/src/worker.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`

## Communities (50 total, 22 thin omitted)

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
Cohesion: 0.23
Nodes (18): ContentCoding, add_content_encoding(), core_loc_conf(), handler(), register(), ngx_conf_t, ngx_http_request_t, ngx_int_t (+10 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.11
Nodes (31): c_char, apply(), CompressConfig, merge_opt(), on_level(), Resolved, c_void, ngx_array_t (+23 more)

### Community 6 - "Codec Trait Seam"
Cohesion: 0.07
Nodes (28): 1. Filter architecture, 2. Crate structure, 3. Safety and coexistence policies, 4.1 Naming: our own scheme, not an upstream drop-in, 4.2.1 Profiles (`compress <tier>`), 4.2 Master switch, profiles, and per-codec toggles, 4.3 Per-codec parameters, 4.4 Shared parameters (global default, per-codec override) (+20 more)

### Community 7 - "NGINX Rust Build Helper"
Cohesion: 0.40
Nodes (3): rust script, ngx_rust_make_module(), ngx_rust_make_modules()

### Community 8 - "Docker Build & Smoke"
Cohesion: 0.30
Nodes (11): check(), check_identity(), check_ssi(), check_static(), decode(), log(), no_proxy, setup_www() (+3 more)

### Community 9 - "Docker Lint Runner"
Cohesion: 0.40
Nodes (4): NGINX_BUILD_DIR, NGINX_SOURCE_DIR, no_proxy, lint.sh script

### Community 13 - "6. Local testing plan"
Cohesion: 0.12
Nodes (20): CodecError, Compress, copy_into(), Deflate, delta(), derive_state(), FlateCore, FlateStep (+12 more)

### Community 15 - "Brotli"
Cohesion: 0.18
Nodes (11): BrotliEncoderOperation, BrotliEncoderStateStruct, Brotli, build_state(), map_operation(), ContentCoding, Operation, Result (+3 more)

### Community 16 - "compress"
Cohesion: 0.10
Nodes (20): 1. Prefetch, 2. Safe core, 3. Submit, Acceptance criteria, Boundary model, Copy and clone policy, Core engineering principle, Current priority areas to re-audit after M3 (+12 more)

### Community 17 - "identity.rs"
Cohesion: 0.07
Nodes (30): continue_requests_more_input_after_draining(), empty_finish_completes_without_output(), Identity, passes_bytes_through_and_finishes(), ContentCoding, Operation, Result, StepResult (+22 more)

### Community 18 - "Operation"
Cohesion: 0.13
Nodes (34): AcceptEncoding, Box, CodecKey, accept_encoding(), advance(), append(), body_filter(), builtin_compressible() (+26 more)

### Community 19 - "StepResult"
Cohesion: 0.30
Nodes (11): check_backpressure(), check_disconnect(), check_http2(), check_no_panic(), check_truncated_upstream(), log(), no_proxy, setup() (+3 more)

### Community 21 - "Resolved"
Cohesion: 0.19
Nodes (22): C, available(), boxed(), build(), choose(), construct(), level_i32(), AcceptEncoding (+14 more)

### Community 34 - "verify-backends.sh"
Cohesion: 0.50
Nodes (3): no_proxy, verify-backends.sh script, smoke()

### Community 36 - "Option"
Cohesion: 0.11
Nodes (17): Adopted integration model, Architecture, Component boundaries, Encoding policy, Explicit non-goals for the first production release, Gaps to prove before codec work, M0: Protocol core, M1: Nginx filter foundation (+9 more)

### Community 42 - "q_str"
Cohesion: 0.27
Nodes (10): bench(), encode(), load(), main(), row(), Option, StreamingCodec, q_str() (+2 more)

### Community 53 - "SystemBrotli"
Cohesion: 0.15
Nodes (12): c_void, create(), EncoderOperation, EncoderParameter, map_operation(), SystemBrotli, Drop, EncoderState (+4 more)

### Community 54 - "test-nginx.sh"
Cohesion: 0.50
Nodes (3): no_proxy, test-nginx.sh script, TEST_NGINX_BINARY

## Knowledge Gaps
- **65 isolated node(s):** `Status`, `Goal`, `Copy and clone policy`, `1. Prefetch`, `2. Safe core` (+60 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `operation_for()` connect `Operation` to `SystemBrotli`?**
  _High betweenness centrality (0.055) - this node is a cross-community bridge._
- **Why does `StreamingCodec` connect `identity.rs` to `Brotli`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **What connects `Status`, `Goal`, `Copy and clone policy` to the rest of the system?**
  _65 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Module Configuration` be split into smaller, more focused modules?**
  _Cohesion score 0.10960960960960961 - nodes in this community are weakly interconnected._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.06896551724137931 - nodes in this community are weakly interconnected._
- **Should `6. Local testing plan` be split into smaller, more focused modules?**
  _Cohesion score 0.11931818181818182 - nodes in this community are weakly interconnected._
- **Should `compress` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._