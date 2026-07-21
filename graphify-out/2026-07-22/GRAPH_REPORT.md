# Graph Report - ngx-compress-rs  (2026-07-22)

## Corpus Check
- 57 files · ~25,489 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 722 nodes · 907 edges · 157 communities (40 shown, 117 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 8 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `4c4a14ad`
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
- Crate Root B
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
- CodecKey
- CompressConfig
- Resolved
- .new
- prefetch_request
- copy_bytes
- Box
- C
- CodecKey
- ContentCoding
- Operation
- Result
- Self
- StreamingCodec
- Error
- FnOnce
- Result
- StreamingCodec
- T
- c_void
- ngx_command_t
- ngx_conf_t
- Option
- Result
- Self
- T
- ngx_http_request_t
- ngx_int_t
- Option
- Result
- Self
- Box
- ContentCoding
- Option
- StreamingCodec
- Option
- Self
- ngx_command_t
- ngx_conf_t
- ngx_int_t
- ngx_str_t
- AcceptEncoding
- Box
- ContentCoding
- Option
- StreamingCodec
- ngx_http_request_t
- ngx_int_t
- Option
- Result
- Self
- Box
- ContentCoding
- Operation
- Option
- Result
- Self
- StreamingCodec
- Error
- FnOnce
- ngx_buf_t
- ngx_chain_t
- ngx_conf_t
- ngx_str_t
- OutputBoundary
- OutputProvider
- OutputUse
- Snapshot
- StepState
- T
- Vec
- FnOnce
- ngx_chain_t
- AcceptEncoding
- C
- ContentCoding
- ngx_str_t

## God Nodes (most connected - your core abstractions)
1. `Resolved` - 13 edges
2. `drive_input()` - 12 edges
3. `try_serve()` - 11 edges
4. `Unsafe boundary refactor` - 11 edges
5. `build()` - 10 edges
6. `CodecKey` - 10 edges
7. `validate_progress()` - 10 edges
8. `edge-tests.sh script` - 10 edges
9. `choose()` - 9 edges
10. `Gzip` - 9 edges

## Surprising Connections (you probably didn't know these)
- `compress_chain()` --calls--> `drive_input()`  [INFERRED]
  crates/ngx-compress-module/src/filter/buffer.rs → crates/ngx-compress-core/src/stream/mod.rs
- `header_filter_inner()` --calls--> `install_ctx()`  [INFERRED]
  crates/ngx-compress-module/src/filter/runtime.rs → crates/ngx-compress-module/src/filter/context.rs
- `body_filter_inner()` --calls--> `with_request_ctx()`  [INFERRED]
  crates/ngx-compress-module/src/filter/runtime.rs → crates/ngx-compress-module/src/filter/context.rs
- `NgxOutput` --implements--> `OutputProvider`  [EXTRACTED]
  crates/ngx-compress-module/src/filter/buffer.rs → crates/ngx-compress-core/src/stream/mod.rs
- `drives_multiple_output_buffers_without_server_pointers()` --calls--> `drive_input()`  [INFERRED]
  crates/ngx-compress-core/src/stream/tests.rs → crates/ngx-compress-core/src/stream/mod.rs

## Import Cycles
- 1-file cycle: `crates/ngx-compress-codecs/src/identity.rs -> crates/ngx-compress-codecs/src/identity.rs`

## Communities (157 total, 117 thin omitted)

### Community 0 - "Accept-Encoding Negotiation"
Cohesion: 0.23
Nodes (11): absent_header_selects_identity_only(), AcceptEncoding, client_quality_overrides_server_order(), ContentCoding, dictionary_coding_requires_server_eligibility(), duplicate_coding_keeps_highest_quality(), explicit_exclusion_overrides_wildcard(), negotiates_common_browser_header_without_allocation() (+3 more)

### Community 1 - "NGINX Module & Filters"
Cohesion: 0.14
Nodes (23): c_char, c_void, ngx_command_t, ngx_conf_t, set_buffers(), set_buffers_inner(), set_directive(), set_directive_inner() (+15 more)

### Community 2 - "Progress Contract"
Cohesion: 0.18
Nodes (15): append(), compress_chain(), free_buf(), NgxOutput, OutputBuffer, OutputBuffer<'a>, recycle(), Error (+7 more)

### Community 3 - "Identity Codec Tests"
Cohesion: 0.22
Nodes (16): add_content_encoding(), core_loc_conf(), MappedPath, ContentCoding, ngx_http_request_t, ngx_int_t, ngx_str_t, Option (+8 more)

### Community 4 - "FFI Filter Chain"
Cohesion: 0.28
Nodes (8): install(), next_body(), next_header(), ngx_chain_t, ngx_http_request_t, ngx_int_t, ngx_http_output_body_filter_pt, ngx_http_output_header_filter_pt

### Community 5 - "Module Configuration"
Cohesion: 0.25
Nodes (6): CompressConfig, on_level(), Preset, Profile, Option, StaticMode

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
Cohesion: 0.06
Nodes (32): Compress, Deflate, CodecError, ContentCoding, Operation, Result, Self, StepResult (+24 more)

### Community 15 - "Brotli"
Cohesion: 0.33
Nodes (3): callback(), FnOnce, T

### Community 16 - "compress"
Cohesion: 0.09
Nodes (21): 1. Prefetch, 2. Safe core, 3. Submit, Acceptance criteria, Boundary model, Completed priority areas, Copy and clone policy, Core engineering principle (+13 more)

### Community 17 - "identity.rs"
Cohesion: 0.06
Nodes (33): BrotliEncoderOperation, BrotliEncoderStateStruct, Brotli, build_state(), map_operation(), ContentCoding, Operation, Result (+25 more)

### Community 18 - "Operation"
Cohesion: 0.14
Nodes (25): cleanup(), install_ctx(), Box, c_void, CodecKey, ngx_http_request_t, Option, RequestCtx (+17 more)

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
Cohesion: 0.27
Nodes (10): accepts_unknown_streaming_length(), CompressionPolicy, eligible(), facts(), policy(), rejects_each_ineligible_fact(), ResponseFacts, Option (+2 more)

### Community 31 - "AcceptEncoding"
Cohesion: 0.30
Nodes (11): accepts_empty_completed_flush(), accepts_waiting_for_input_when_none_is_available(), Operation, ProgressError, rejects_completed_boundary_with_unconsumed_input(), rejects_continue_loop_without_progress(), rejects_false_output_backpressure(), Result (+3 more)

### Community 32 - "Box"
Cohesion: 0.42
Nodes (9): brotli_roundtrips_across_buffer_sizes(), compress(), deflate_roundtrips_across_buffer_sizes(), gzip_handles_empty_input(), gzip_roundtrips_across_buffer_sizes(), StreamingCodec, Vec, sample() (+1 more)

### Community 34 - "verify-backends.sh"
Cohesion: 0.50
Nodes (3): no_proxy, verify-backends.sh script, smoke()

### Community 36 - "Option"
Cohesion: 0.11
Nodes (17): Adopted integration model, Architecture, Component boundaries, Encoding policy, Explicit non-goals for the first production release, Gaps to prove before codec work, M0: Protocol core, M1: Nginx filter foundation (+9 more)

### Community 42 - "q_str"
Cohesion: 0.36
Nodes (9): bench(), encode(), load(), main(), row(), Option, StreamingCodec, String (+1 more)

### Community 49 - "StreamingCodec"
Cohesion: 0.08
Nodes (31): drive_input(), DriveError, DriveOutcome, OutputAction, OutputBoundary, OutputProvider, OutputUse, C (+23 more)

### Community 50 - "Self"
Cohesion: 0.20
Nodes (8): CompressConfig, merge_opt(), Option, Result, Self, Merge, MergeConfigError, T

### Community 53 - "SystemBrotli"
Cohesion: 0.13
Nodes (14): c_void, CodecError, create(), EncoderOperation, EncoderParameter, map_operation(), SystemBrotli, Drop (+6 more)

### Community 54 - "test-nginx.sh"
Cohesion: 0.50
Nodes (3): no_proxy, test-nginx.sh script, TEST_NGINX_BINARY

### Community 79 - "CodecKey"
Cohesion: 0.10
Nodes (28): accept_encoding(), CodecKey, Plan, RequestCtx, AcceptEncoding, Box, ContentCoding, ngx_chain_t (+20 more)

### Community 80 - "CompressConfig"
Cohesion: 0.16
Nodes (14): q_str(), CompressConfig, Profile, Option, Self, StaticMode, set_flag(), set_static() (+6 more)

### Community 81 - "Resolved"
Cohesion: 0.17
Nodes (23): Arc, C, ContentCoding, decide(), Option, Snapshot, available(), boxed() (+15 more)

### Community 82 - ".new"
Cohesion: 0.26
Nodes (7): InputBuffer, InputBuffer<'a>, operation_for(), ngx_buf_t, Operation, Result, Self

### Community 83 - "prefetch_request"
Cohesion: 0.27
Nodes (10): handler(), prefetch_request(), register(), ngx_conf_t, ngx_http_request_t, ngx_int_t, Option, Result (+2 more)

### Community 84 - "copy_bytes"
Cohesion: 0.48
Nodes (6): copy_bytes(), copy_string(), ngx_str_t, Option, String, Vec

## Knowledge Gaps
- **70 isolated node(s):** `Profile`, `CompressConfig`, `Profile`, `Module`, `Status` (+65 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **117 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `RequestCtx` connect `CodecKey` to `Progress Contract`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Why does `Resolved` connect `Resolved` to `ngx_command_t`, `Module Configuration`?**
  _High betweenness centrality (0.055) - this node is a cross-community bridge._
- **Are the 4 inferred relationships involving `drive_input()` (e.g. with `drives_multiple_output_buffers_without_server_pointers()` and `emits_empty_finish_boundary()`) actually correct?**
  _`drive_input()` has 4 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Profile`, `CompressConfig`, `Profile` to the rest of the system?**
  _70 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `NGINX Module & Filters` be split into smaller, more focused modules?**
  _Cohesion score 0.14245014245014245 - nodes in this community are weakly interconnected._
- **Should `Codec Trait Seam` be split into smaller, more focused modules?**
  _Cohesion score 0.06896551724137931 - nodes in this community are weakly interconnected._
- **Should `6. Local testing plan` be split into smaller, more focused modules?**
  _Cohesion score 0.058279370952821465 - nodes in this community are weakly interconnected._