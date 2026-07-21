use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs one exported callback without allowing a Rust panic to cross the C ABI.
///
/// The caller supplies the value NGINX should receive if internal Rust code
/// panics. `AssertUnwindSafe` is appropriate here because the callback is being
/// abandoned and NGINX receives an explicit failure result.
pub fn callback<T>(fallback: T, run: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(run)).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::callback;

    #[test]
    fn returns_callback_value() {
        assert_eq!(callback(-1, || 42), 42);
    }

    #[test]
    fn maps_panic_to_fallback() {
        assert_eq!(callback(-1, || panic!("boundary failure")), -1);
    }
}
