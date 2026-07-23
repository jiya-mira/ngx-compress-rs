//! Reinitialization of module-owned NGINX output buffers.

use ngx::ffi::ngx_buf_t;

/// Returns a reusable temporary buffer to a neutral per-emission state.
pub fn prepare_output(buf: &mut ngx_buf_t) {
    buf.pos = buf.start;
    buf.last = buf.start;
    buf.set_temporary(1);
    buf.set_flush(0);
    buf.set_sync(0);
    buf.set_last_buf(0);
    buf.set_last_in_chain(0);
}

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;

    use ngx::ffi::ngx_buf_t;

    use super::prepare_output;

    #[test]
    fn clears_stale_boundary_flags_on_reuse() {
        let mut storage = [0_u8; 8];
        // SAFETY: ngx_buf_t is a C data holder whose all-zero state is nginx's
        // allocation default; the live storage pointers are installed below.
        let mut raw = unsafe { MaybeUninit::<ngx_buf_t>::zeroed().assume_init() };
        raw.start = storage.as_mut_ptr();
        raw.end = raw.start.wrapping_add(storage.len());
        raw.pos = raw.end;
        raw.last = raw.end;
        raw.set_flush(1);
        raw.set_sync(1);
        raw.set_last_buf(1);
        raw.set_last_in_chain(1);

        prepare_output(&mut raw);

        assert_eq!(raw.pos, raw.start);
        assert_eq!(raw.last, raw.start);
        assert_eq!(raw.temporary(), 1);
        assert_eq!(raw.flush(), 0);
        assert_eq!(raw.sync(), 0);
        assert_eq!(raw.last_buf(), 0);
        assert_eq!(raw.last_in_chain(), 0);
    }
}
