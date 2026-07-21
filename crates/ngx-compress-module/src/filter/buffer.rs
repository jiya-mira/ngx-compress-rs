//! Callback-scoped NGINX buffer views and output-chain management.

use core::{ptr, slice};

use ngx::ffi::{ngx_buf_t, ngx_chain_get_free_buf, ngx_chain_t, ngx_http_request_t, ngx_palloc};
use ngx_compress_core::{OutputAction, OutputBoundary, OutputProvider, OutputUse, drive_input};

use crate::registration::ngx_http_compress_module;

use super::RequestCtx;
use super::input::InputBuffer;

/// A validated, callback-scoped view of one writable nginx output buffer.
struct OutputBuffer<'a> {
    raw: &'a mut ngx_buf_t,
    capacity: &'a mut [u8],
}

impl<'a> OutputBuffer<'a> {
    /// Converts a borrowed nginx temp buffer into a lifetime-bound writable view.
    ///
    /// # Safety
    ///
    /// Non-empty `last..end` must belong to one live writable allocation for
    /// `'a`, and this borrow must be the only access to that region.
    // SAFETY: the caller must uphold the allocation, uniqueness, and lifetime contract above.
    unsafe fn new(raw: &'a mut ngx_buf_t) -> Result<Self, ()> {
        let last = raw.last;
        let end = raw.end;
        let capacity = if last == end {
            &mut []
        } else {
            let len = end.addr().checked_sub(last.addr()).ok_or(())?;
            if last.is_null() {
                return Err(());
            }
            // SAFETY: the caller guarantees that validated last..end is one
            // live, uniquely borrowed writable allocation.
            unsafe { slice::from_raw_parts_mut(last, len) }
        };
        Ok(Self { raw, capacity })
    }

    fn capacity(&mut self) -> &mut [u8] {
        self.capacity
    }

    fn commit(&mut self, produced: usize, boundary: OutputBoundary) -> Result<(), ()> {
        if produced > self.capacity.len() {
            return Err(());
        }
        // SAFETY: `produced` was validated against this view's capacity, which
        // is the live allocation beginning at raw.last.
        self.raw.last = unsafe { self.raw.last.add(produced) };
        match boundary {
            OutputBoundary::None => {}
            OutputBoundary::Flush => self.raw.set_flush(1),
            OutputBoundary::Finish => self.raw.set_last_buf(1),
        }
        Ok(())
    }
}

/// NGINX-specific output provider consumed by the safe streaming driver.
struct NgxOutput<'a> {
    request: *mut ngx_http_request_t,
    out: &'a mut *mut ngx_chain_t,
    free: &'a mut *mut ngx_chain_t,
    buffer_size: usize,
}

impl OutputProvider for NgxOutput<'_> {
    type Error = ();

    fn with_output<T>(
        &mut self,
        use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
    ) -> Result<T, Self::Error> {
        // SAFETY: provider construction is callback-scoped and guarantees a
        // valid request plus unique access to its out/free chains.
        unsafe {
            let link = free_buf(self.request, self.free, self.buffer_size);
            if link.is_null() {
                return Err(());
            }
            let buf = (*link).buf.as_mut().ok_or(())?;
            let mut output = OutputBuffer::new(buf)?;
            let used = use_output(output.capacity());
            match used.action {
                OutputAction::Recycle => recycle(self.free, link),
                OutputAction::Emit { produced, boundary } => {
                    output.commit(produced, boundary)?;
                    append(self.out, link);
                }
            }
            Ok(used.value)
        }
    }
}

/// Drives the codec over the whole input chain, appending output buffers to
/// `ctx.out`. Marks each input buffer consumed as it is accepted.
///
/// # Safety
///
/// `request` and `chain` must be valid, and `ctx` uniquely borrowed.
// SAFETY: the caller must uphold the pointer and unique-borrow contract above.
pub(in crate::filter) unsafe fn compress_chain(
    request: *mut ngx_http_request_t,
    ctx: &mut RequestCtx,
    mut chain: *mut ngx_chain_t,
) -> Result<(), ()> {
    while !chain.is_null() {
        // SAFETY: `chain` is a valid link and its buffer remains live while the
        // callback-scoped view is used.
        let input = unsafe {
            let buf = (*chain).buf.as_mut().ok_or(())?;
            InputBuffer::new(buf)?
        };
        let outcome = {
            let mut output = NgxOutput {
                request,
                out: &mut ctx.out,
                free: &mut ctx.free,
                buffer_size: ctx.buffer_size,
            };
            drive_input(
                &mut *ctx.codec,
                input.operation(),
                input.bytes(),
                &mut output,
            )
            .map_err(|_| ())?
        };
        ctx.done = outcome.finished;

        input.consume();
        // SAFETY: input is consumed and `chain` remains a valid link.
        chain = unsafe { (*chain).next };
    }
    Ok(())
}

unsafe fn append(out: &mut *mut ngx_chain_t, link: *mut ngx_chain_t) {
    // SAFETY: append `link` to the tail of the `out` chain.
    unsafe {
        (*link).next = ptr::null_mut();
        let mut tail = out;
        while !(*tail).is_null() {
            tail = &mut (**tail).next;
        }
        *tail = link;
    }
}

unsafe fn recycle(free: &mut *mut ngx_chain_t, link: *mut ngx_chain_t) {
    // SAFETY: prepend an unused buffer link back onto the free chain for reuse.
    unsafe {
        (*link).next = *free;
        *free = link;
    }
}

/// Reuses a buffer from the free chain, or allocates a new temp buffer + link.
// SAFETY: request, free-chain links, and their pool allocation must remain valid.
unsafe fn free_buf(
    request: *mut ngx_http_request_t,
    free: &mut *mut ngx_chain_t,
    buffer_size: usize,
) -> *mut ngx_chain_t {
    // SAFETY: pool is valid; ngx_chain_get_free_buf reuses or allocates a link.
    unsafe {
        let pool = (*request).pool;
        let link = ngx_chain_get_free_buf(pool, free);
        if link.is_null() {
            return ptr::null_mut();
        }
        let buf = (*link).buf;
        if (*buf).start.is_null() {
            let memory = ngx_palloc(pool, buffer_size).cast::<u8>();
            if memory.is_null() {
                return ptr::null_mut();
            }
            (*buf).start = memory;
            (*buf).end = memory.add(buffer_size);
            (*buf).tag = ptr::addr_of!(ngx_http_compress_module).cast_mut().cast();
        }
        (*buf).pos = (*buf).start;
        (*buf).last = (*buf).start;
        (*buf).set_temporary(1);
        link
    }
}
