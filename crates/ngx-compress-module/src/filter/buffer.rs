//! Callback-scoped NGINX buffer views and output-chain management.

use core::{ptr, slice};
use std::time::Instant;

use ngx::ffi::{ngx_buf_t, ngx_chain_get_free_buf, ngx_chain_t, ngx_http_request_t, ngx_palloc};
use ngx_compress_core::{
    DriveError, OutputAction, OutputBoundary, OutputProvider, OutputUse, StepError, drive_input,
};

use crate::{fault, fault::Point, registration::ngx_http_compress_module};

use super::{CompressChain, CompressionFailure, InputBuffer, InputView, OutputFailure, RequestCtx};

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
    unsafe fn new(raw: &'a mut ngx_buf_t) -> Result<Self, OutputFailure> {
        let last = raw.last;
        let end = raw.end;
        let capacity = if last == end {
            &mut []
        } else {
            let len = end
                .addr()
                .checked_sub(last.addr())
                .ok_or(OutputFailure::InvalidFfiState)?;
            if last.is_null() {
                return Err(OutputFailure::InvalidFfiState);
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

    fn commit(&mut self, produced: usize, boundary: OutputBoundary) -> Result<(), OutputFailure> {
        if produced > self.capacity.len() {
            return Err(OutputFailure::InvalidFfiState);
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
    produced: usize,
    track_produced: bool,
}

impl OutputProvider for NgxOutput<'_> {
    type Error = OutputFailure;

    fn with_output<T>(
        &mut self,
        use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
    ) -> Result<T, Self::Error> {
        // SAFETY: provider construction is callback-scoped and guarantees a
        // valid request plus unique access to its out/free chains.
        unsafe {
            let link = free_buf(self.request, self.free, self.buffer_size);
            if link.is_null() {
                return Err(OutputFailure::Allocation);
            }
            let buf = (*link).buf.as_mut().ok_or(OutputFailure::InvalidFfiState)?;
            let mut output = OutputBuffer::new(buf)?;
            let used = use_output(output.capacity());
            match used.action {
                OutputAction::Recycle => recycle(self.free, link),
                OutputAction::Emit { produced, boundary } => {
                    output.commit(produced, boundary)?;
                    if self.track_produced {
                        self.produced = self.produced.saturating_add(produced);
                    }
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
impl CompressChain for RequestCtx {
    unsafe fn compress(
        &mut self,
        request: *mut ngx_http_request_t,
        mut chain: *mut ngx_chain_t,
    ) -> Result<(), CompressionFailure> {
        while !chain.is_null() {
            // SAFETY: `chain` is a valid link and its buffer remains live while the
            // callback-scoped view is used.
            let input = unsafe {
                let buf = (*chain)
                    .buf
                    .as_mut()
                    .ok_or(CompressionFailure::InvalidFfiState)?;
                InputBuffer::new(buf).map_err(|()| CompressionFailure::InvalidFfiState)?
            };
            let outcome = {
                let started = self.stats.as_ref().map(|_| Instant::now());
                let mut output = NgxOutput {
                    request,
                    out: &mut self.out,
                    free: &mut self.free,
                    buffer_size: self.buffer_size,
                    produced: 0,
                    track_produced: started.is_some(),
                };
                let outcome = drive_input(
                    &mut *self.codec,
                    input.operation(),
                    input.bytes(),
                    &mut output,
                )
                .map_err(|error| match error {
                    DriveError::Step(StepError::Codec(_)) => CompressionFailure::CodecBackend,
                    DriveError::Step(StepError::InvalidProgress(_)) => {
                        CompressionFailure::InvalidCodecProgress
                    }
                    DriveError::Output(OutputFailure::Allocation) => {
                        CompressionFailure::OutputAllocation
                    }
                    DriveError::Output(OutputFailure::InvalidFfiState) => {
                        CompressionFailure::InvalidFfiState
                    }
                })?;
                if let (Some(stats), Some(started)) = (&mut self.stats, started) {
                    stats.record(outcome.consumed, output.produced, started.elapsed());
                }
                outcome
            };
            self.done = outcome.finished;

            input.consume();
            // SAFETY: input is consumed and `chain` remains a valid link.
            chain = unsafe { (*chain).next };
        }
        Ok(())
    }
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
    if fault::take(Point::OutputAllocation) {
        return ptr::null_mut();
    }
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
        ngx_compress_ffi::buffer::prepare_output(&mut *buf);
        link
    }
}

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;

    use ngx::ffi::ngx_buf_t;
    use ngx_compress_core::OutputBoundary;

    use super::OutputBuffer;

    fn raw_buffer(storage: &mut [u8]) -> ngx_buf_t {
        // SAFETY: ngx_buf_t is a C data holder whose all-zero state is nginx's
        // allocation default; the live storage pointers are installed below.
        let mut raw = unsafe { MaybeUninit::<ngx_buf_t>::zeroed().assume_init() };
        raw.start = storage.as_mut_ptr();
        raw.end = raw.start.wrapping_add(storage.len());
        raw.pos = raw.start;
        raw.last = raw.start;
        raw
    }

    fn commit(raw: &mut ngx_buf_t, boundary: OutputBoundary) {
        // SAFETY: raw points into the live test storage for this scope.
        let Ok(mut output) = (unsafe { OutputBuffer::new(raw) }) else {
            panic!("valid test buffer");
        };
        assert!(output.commit(1, boundary).is_ok());
    }

    #[test]
    fn commit_sets_only_the_current_boundary() {
        let mut storage = [0_u8; 8];
        let mut raw = raw_buffer(&mut storage);

        ngx_compress_ffi::buffer::prepare_output(&mut raw);
        commit(&mut raw, OutputBoundary::None);
        assert_eq!(raw.flush(), 0);
        assert_eq!(raw.sync(), 0);
        assert_eq!(raw.last_buf(), 0);
        assert_eq!(raw.last_in_chain(), 0);

        ngx_compress_ffi::buffer::prepare_output(&mut raw);
        commit(&mut raw, OutputBoundary::Flush);
        assert_eq!(raw.flush(), 1);
        assert_eq!(raw.sync(), 0);
        assert_eq!(raw.last_buf(), 0);
        assert_eq!(raw.last_in_chain(), 0);

        ngx_compress_ffi::buffer::prepare_output(&mut raw);
        commit(&mut raw, OutputBoundary::Finish);
        assert_eq!(raw.flush(), 0);
        assert_eq!(raw.sync(), 0);
        assert_eq!(raw.last_buf(), 1);
        assert_eq!(raw.last_in_chain(), 0);
    }
}
