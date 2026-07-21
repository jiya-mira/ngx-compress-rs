# Graph Report - ngx-compress-rs  (2026-07-21)

## Corpus Check
- 47 files · ~25,583 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 579 nodes · 906 edges · 79 communities (34 shown, 45 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5764e526`
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
- Zstd
- StreamingCodec
- Self
- StreamingCodec
- ngx_conf_t
- SystemBrotli
- test-nginx.sh
- NonNull
- ContentCoding
- Resolved
- ngx_array_t
- NonNull
- Option
- Resolved
- Result
- Self
- Option
- String
- FnOnce
- Self
- ngx_int_t
- ngx_str_t
- ngx_conf_t
- ngx_conf_t
- ngx_conf_t
- Profile
- Resolved
- Snapshot
- T
- ngx_conf_t
- ngx_str_t

## God Nodes (most connected - your core abstractions)
1. `CompressConfig` - 13 edges
2. `Resolved` - 13 edges
3. `try_serve()` - 12 edges
4. `drive_input()` - 12 edges
5. `Unsafe boundary refactor` - 11 edges
6. `CodecKey` - 11 edges
7. `build()` - 10 edges
8. `validate_progress()` - 10 edges
9. `edge-tests.sh script` - 10 edges
10. `compress_chain()` - 9 edges

## Surprising Connections (you probably didn't know these)
- `prefetch_request()` --calls--> `accept_encoding()`  [INFERRED]
  crates/ngx-compress-module/src/static_file.rs → crates/ngx-compress-module/src/filter.rs
- `Brotli` --implements--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/src/brotli_codec.rs → crates/ngx-compress-core/src/codec.rs
- `compress()` --references--> `StreamingCodec`  [EXTRACTED]
  crates/ngx-compress-codecs/tests/roundtrip.rs → crates/ngx-compress-core/src/codec.rs
- `CompressConfig` --references--> `Profile`  [EXTRACTED]
  crates/ngx-compress-module/src/config.rs → crates/ngx-compress-module/src/profile.rs
- `decide()` --references--> `Resolved`  [EXTRACTED]
  crates/ngx-compress-module/src/header.rs → crates/ngx-compress-module/src/config.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`

## Communities (79 total, 45 thin omitted)

### Community 0 - "Accept-Encoding Negotiation"
Cohesion: 0.23
Nodes (11): absent_header_selects_identity_only(), AcceptEncoding, client_quality_overrides_server_order(), ContentCoding, dictionary_coding_requires_server_eligibility(), duplicate_coding_keeps_highest_quality(), explicit_exclusion_overrides_wildcard(), negotiates_common_browser_header_without_allocation() (+3 more)

### Community 1 - "NGINX Module & Filters"
Cohesion: 0.17
Nodes (13): directive(), Module, multi(), postconfiguration_inner(), ngx_command_t, ngx_conf_t, ngx_int_t, ngx_str_t (+5 more)

### Community 2 - "Progress Contract"
Cohesion: 0.18
Nodes (11): BrotliEncoderOperation, BrotliEncoderStateStruct, Brotli, build_state(), map_operation(), ContentCoding, Operation, Result (+3 more)

### Community 3 - "Identity Codec Tests"
Cohesion: 0.17
Nodes (22): ContentCoding, add_content_encoding(), core_loc_conf(), handler(), MappedPath, prefetch_request(), register(), ngx_http_request_t (+14 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.55
Nodes (10): c_char, c_void, ngx_command_t, ngx_conf_t, set_buffers(), set_buffers_inner(), set_directive(), set_directive_inner() (+2 more)

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
Cohesion: 0.10
Nodes (22): CodecError, Compress, copy_into(), Deflate, delta(), derive_state(), FlateCore, FlateStep (+14 more)

### Community 15 - "Brotli"
Cohesion: 0.33
Nodes (3): callback(), FnOnce, T

### Community 16 - "compress"
Cohesion: 0.09
Nodes (21): 1. Prefetch, 2. Safe core, 3. Submit, Acceptance criteria, Boundary model, Completed priority areas, Copy and clone policy, Core engineering principle (+13 more)

### Community 17 - "identity.rs"
Cohesion: 0.09
Nodes (22): continue_requests_more_input_after_draining(), empty_finish_completes_without_output(), Identity, passes_bytes_through_and_finishes(), ContentCoding, Operation, Result, StepResult (+14 more)

### Community 18 - "Operation"
Cohesion: 0.09
Nodes (41): Box, c_void, CodecKey, accept_encoding(), append(), body_filter(), body_filter_inner(), body_filter_with_ctx() (+33 more)

### Community 19 - "StepResult"
Cohesion: 0.30
Nodes (11): check_backpressure(), check_disconnect(), check_http2(), check_no_panic(), check_truncated_upstream(), log(), no_proxy, setup() (+3 more)

### Community 20 - "AcceptEncoding"
Cohesion: 0.18
Nodes (11): builtin_compressible(), compressible(), explicit_types_are_owned_and_case_insensitive(), MimeTypes, Box, Option, Self, String (+3 more)

### Community 21 - "Resolved"
Cohesion: 0.33
Nodes (11): AcceptEncoding, always_keeps_server_priority_without_accept_header(), facts(), on_filters_unacceptable_candidates(), rejects_directory_and_unsupported_method(), ContentCoding, Vec, static_candidates() (+3 more)

### Community 28 - "ngx_command_t"
Cohesion: 0.22
Nodes (10): accepts_unknown_streaming_length(), CompressionPolicy, eligible(), facts(), policy(), rejects_each_ineligible_fact(), ResponseFacts, Option (+2 more)

### Community 31 - "AcceptEncoding"
Cohesion: 0.09
Nodes (33): C, accepts_empty_completed_flush(), accepts_waiting_for_input_when_none_is_available(), Operation, ProgressError, rejects_completed_boundary_with_unconsumed_input(), rejects_continue_loop_without_progress(), rejects_false_output_backpressure() (+25 more)

### Community 32 - "Box"
Cohesion: 0.50
Nodes (8): brotli_roundtrips_across_buffer_sizes(), compress(), deflate_roundtrips_across_buffer_sizes(), gzip_handles_empty_input(), gzip_roundtrips_across_buffer_sizes(), Vec, sample(), zstd_roundtrips_across_buffer_sizes()

### Community 34 - "verify-backends.sh"
Cohesion: 0.50
Nodes (3): no_proxy, verify-backends.sh script, smoke()

### Community 36 - "Option"
Cohesion: 0.11
Nodes (17): Adopted integration model, Architecture, Component boundaries, Encoding policy, Explicit non-goals for the first production release, Gaps to prove before codec work, M0: Protocol core, M1: Nginx filter foundation (+9 more)

### Community 42 - "q_str"
Cohesion: 0.20
Nodes (14): bench(), encode(), load(), main(), row(), Option, q_str(), copy_bytes() (+6 more)

### Community 50 - "Self"
Cohesion: 0.12
Nodes (20): Arc, CompressConfig, merge_opt(), on_level(), Option, Result, Self, T (+12 more)

### Community 53 - "SystemBrotli"
Cohesion: 0.21
Nodes (8): create(), EncoderOperation, EncoderParameter, map_operation(), SystemBrotli, Drop, EncoderState, Self

### Community 54 - "test-nginx.sh"
Cohesion: 0.50
Nodes (3): no_proxy, test-nginx.sh script, TEST_NGINX_BINARY

### Community 61 - "Resolved"
Cohesion: 0.11
Nodes (35): Resolved, decide(), Plan, AcceptEncoding, Box, ContentCoding, Option, StreamingCodec (+27 more)

## Knowledge Gaps
- **66 isolated node(s):** `Status`, `Implemented result`, `Goal`, `Copy and clone policy`, `1. Prefetch` (+61 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **45 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `StreamingCodec` connect `identity.rs` to `Box`, `Progress Contract`, `ngx_command_t`?**
  _High betweenness centrality (0.067) - this node is a cross-community bridge._
- **Why does `Resolved` connect `Resolved` to `Self`, `ngx_command_t`?**
  _High betweenness centrality (0.062) - this node is a cross-community bridge._
- **Why does `accept_encoding()` connect `Operation` to `Identity Codec Tests`, `Resolved`?**
  _High betweenness centrality (0.060) - this node is a cross-community bridge._
- **What connects `Status`, `Implemented result`, `Goal` to the rest of the system?**
  _66 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.06896551724137931 - nodes in this community are weakly interconnected._
- **Should `6. Local testing plan` be split into smaller, more focused modules?**
  _Cohesion score 0.1036036036036036 - nodes in this community are weakly interconnected._
- **Should `compress` be split into smaller, more focused modules?**
  _Cohesion score 0.09090909090909091 - nodes in this community are weakly interconnected._