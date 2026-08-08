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

    fn consume(self, bytes: usize) -> Result<bool, ()> {
        if bytes > self.bytes.len() {
            return Err(());
        }
        // SAFETY: bytes was checked against the live pos..last view.
        self.raw.pos = unsafe { self.raw.pos.add(bytes) };
        Ok(self.raw.pos == self.raw.last)
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

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;

    use ngx::ffi::ngx_buf_t;

    use super::{InputBuffer, InputView};

    #[test]
    fn advances_only_the_consumed_prefix() {
        let mut storage = *b"abcdef";
        // SAFETY: ngx_buf_t is a C data holder whose zero state is nginx's
        // allocation default; live test pointers are installed below.
        let mut raw = unsafe { MaybeUninit::<ngx_buf_t>::zeroed().assume_init() };
        raw.pos = storage.as_mut_ptr();
        raw.last = raw.pos.wrapping_add(storage.len());

        // SAFETY: pos..last points to live storage for this scope.
        let input = unsafe { InputBuffer::new(&mut raw) }.expect("valid input");
        assert_eq!(input.consume(2), Ok(false));
        // SAFETY: pos was advanced within the same live allocation.
        assert_eq!(unsafe { raw.last.offset_from(raw.pos) }, 4);
    }
}
