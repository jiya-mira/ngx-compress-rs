# Fuzz targets

`cargo-fuzz` (libFuzzer) targets for the protocol core. libFuzzer requires the
nightly toolchain, so this crate is its own workspace and is not built by the
stable CI. Run:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run parse       # Accept-Encoding parser
cargo +nightly fuzz run progress    # validate_progress
```

The same invariants are exercised on stable by the property tests in
`crates/ngx-compress-core/tests/properties.rs`.
