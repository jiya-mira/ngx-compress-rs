ngx_addon_name=ngx_http_compress
ngx_cargo_manifest=$ngx_addon_dir/Cargo.toml

# Generate the Makefile section that drives cargo for the module above.
ngx_rust_make_modules
