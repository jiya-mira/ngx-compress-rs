//! Build script for the NGINX module crate.
//!
//! Due to a cargo limitation, a crate that consumes `nginx-sys`-derived cfg
//! values must depend on `nginx-sys` directly and reproduce this script. It
//! reads the `DEP_NGINX_*` variables that `nginx-sys` exports and forwards them
//! to the compiler as `ngx_feature`, `ngx_os`, and version cfg flags.

const VERSION_CHECKS: &[(u64, &str)] = &[(1_021_001, "nginx1_21_1"), (1_025_001, "nginx1_25_1")];

fn main() {
    println!("cargo::rerun-if-env-changed=DEP_NGINX_FEATURES_CHECK");
    println!(
        "cargo::rustc-check-cfg=cfg(ngx_feature, values({}))",
        std::env::var("DEP_NGINX_FEATURES_CHECK").unwrap_or_else(|_| "any()".to_string())
    );

    println!("cargo::rerun-if-env-changed=DEP_NGINX_FEATURES");
    if let Ok(features) = std::env::var("DEP_NGINX_FEATURES") {
        // Comma-separated list produced by the nginx-sys build script. style:allow-delimited-split
        features
            .split(',')
            .map(str::trim)
            .for_each(|feature| println!("cargo::rustc-cfg=ngx_feature=\"{feature}\""));
    }

    println!("cargo::rerun-if-env-changed=DEP_NGINX_OS_CHECK");
    println!(
        "cargo::rustc-check-cfg=cfg(ngx_os, values({}))",
        std::env::var("DEP_NGINX_OS_CHECK").unwrap_or_else(|_| "any()".to_string())
    );

    println!("cargo::rerun-if-env-changed=DEP_NGINX_OS");
    if let Ok(os) = std::env::var("DEP_NGINX_OS") {
        println!("cargo::rustc-cfg=ngx_os=\"{os}\"");
    }

    // Build-script side-effect loops (println! only). style:allow-for-in
    for check in VERSION_CHECKS {
        println!("cargo::rustc-check-cfg=cfg({})", check.1);
    }

    println!("cargo::rerun-if-env-changed=DEP_NGINX_VERSION_NUMBER");
    if let Ok(Ok(version)) =
        std::env::var("DEP_NGINX_VERSION_NUMBER").map(|value| value.parse::<u64>())
    {
        // style:allow-for-in
        for check in VERSION_CHECKS.iter().filter(|check| version >= check.0) {
            println!("cargo::rustc-cfg={}", check.1);
        }
    }

    println!("cargo::rerun-if-env-changed=DEP_NGINX_BUILD_DIR");
    if let Ok(build_dir) = std::env::var("DEP_NGINX_BUILD_DIR") {
        println!("cargo::rustc-env=DEP_NGINX_BUILD_DIR={build_dir}");
    }

    // macOS links the cdylib with unresolved NGINX symbols left for runtime.
    if cfg!(target_os = "macos") {
        println!("cargo::rustc-link-arg=-undefined");
        println!("cargo::rustc-link-arg=dynamic_lookup");
    }
}
