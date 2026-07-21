use ngx::ffi::ngx_str_t;

/// Copies an nginx byte string into Rust ownership.
///
/// # Safety
///
/// For a non-empty value, `data` must point to `len` live readable bytes.
#[must_use]
// SAFETY: the caller must uphold the pointer and length contract above.
pub unsafe fn copy_bytes(value: &ngx_str_t) -> Option<Vec<u8>> {
    if value.len == 0 {
        return Some(Vec::new());
    }
    if value.data.is_null() {
        return None;
    }
    // SAFETY: caller guarantees the non-empty ngx_str allocation contract.
    Some(unsafe { core::slice::from_raw_parts(value.data, value.len) }.to_vec())
}

/// Copies a UTF-8 nginx string into Rust ownership.
///
/// # Safety
///
/// For a non-empty value, `data` must point to `len` live readable bytes.
#[must_use]
// SAFETY: the caller must uphold the pointer and length contract above.
pub unsafe fn copy_string(value: &ngx_str_t) -> Option<String> {
    // SAFETY: forwards the caller's ngx_str allocation guarantee.
    String::from_utf8(unsafe { copy_bytes(value)? }).ok()
}
