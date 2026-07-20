# ngx-compress-rs

`ngx-compress-rs` is a planned NGINX HTTP response compression module implemented in Rust on top of the official `nginx/ngx-rust` bindings.

The project aims to provide one rigorously tested implementation for modern HTTP content codings:

- `gzip`
- `deflate`
- `br`
- `zstd`
- `identity`
- `dcb` and `dcz` in a later dictionary-compression milestone

## Current status

The repository currently contains the allocation-free `Accept-Encoding` negotiation model and the progress invariants that future streaming encoders must satisfy. It does not yet build an Nginx module.

## Design priorities

1. A stuck or malformed upstream response must never spin an Nginx worker.
2. Compression state is explicit and validated after every encoder step.
3. NGINX ABI-specific `unsafe` code remains isolated from protocol and policy code.
4. Codec implementations come from established libraries and are selected through typed adapters.
5. Dynamic-module artifacts are built and tested against each supported Nginx build signature.

See [docs/architecture.md](docs/architecture.md) for the proposed component boundaries and milestones.

## Development

```shell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
