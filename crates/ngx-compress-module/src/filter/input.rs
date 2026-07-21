//! Callback-scoped readable NGINX input-buffer views.

use core::slice;

use ngx_compress_core::Operation;

use super::{InputBuffer, InputView};

impl<'a> InputView<'a> for InputBuffer<'a> {
    /// Converts a borrowed nginx buffer into a lifetime-bound readable view.
    ///
    /// # Safety
    ///
    /// Non-empty `pos..last` must belong to one live allocation for `'a`.
    // SAFETY: the caller must uphold the allocation and lifetime contract above.
    unsafe fn new(raw: &'a mut ngx::ffi::ngx_buf_t) -> Result<Self, ()> {
        let operation = operation_for(raw);
        let pos = raw.pos;
        let last = raw.last;
        let bytes = if pos == last {
            &[]
        } else {
            let len = last.addr().checked_sub(pos.addr()).ok_or(())?;
            if pos.is_null() {
                return Err(());
            }
            // SAFETY: the caller guarantees that validated pos..last is one
            // live readable allocation tied to the borrowed nginx buffer.
            unsafe { slice::from_raw_parts(pos, len) }
        };
        Ok(Self {
            raw,
            bytes,
            operation,
        })
    }

    fn operation(&self) -> Operation {
        self.operation
    }

    fn bytes(&self) -> &[u8] {
        self.bytes
    }

    fn consume(self) {
        self.raw.pos = self.raw.last;
    }
}

fn operation_for(buf: &ngx::ffi::ngx_buf_t) -> Operation {
    if buf.last_buf() != 0 {
        Operation::Finish
    } else if buf.flush() != 0 || buf.last_in_chain() != 0 {
        Operation::Flush
    } else {
        Operation::Continue
    }
}
