//! Callback-scoped NGINX buffer views and output-chain management.

use core::{ptr, slice};
use std::time::Instant;

use ngx::ffi::{ngx_buf_t, ngx_chain_get_free_buf, ngx_chain_t, ngx_http_request_t, ngx_palloc};
use ngx_compress_core::{
    DriveError, DriveState, OutputAction, OutputBoundary, OutputProvider, OutputUse, StepError,
    WorkBudget, drive_input,
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
    buffer_count: usize,
    allocated_buffers: &'a mut usize,
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
            let link = free_buf(
                self.request,
                self.free,
                self.buffer_size,
                self.buffer_count,
                self.allocated_buffers,
            )?;
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

/// Copies incoming chain links into request-owned pending state, then drives no
/// more than one callback budget. Returns true when work remains.
///
/// # Safety
///
/// `request` and `chain` must be valid, and `ctx` uniquely borrowed.
// SAFETY: the caller must uphold the pointer and unique-borrow contract above.
impl CompressChain for RequestCtx {
    unsafe fn compress(
        &mut self,
        request: *mut ngx_http_request_t,
        chain: *mut ngx_chain_t,
    ) -> Result<bool, CompressionFailure> {
        unsafe { append_input(self, request, chain)? };
        let mut budget = WorkBudget::per_callback();
        if let Some(pending) = resume_operation(self, request, &mut budget)? {
            return Ok(pending);
        }
        while !self.input.is_null() {
            let current = self.input;
            // SAFETY: `chain` is a valid link and its buffer remains live while the
            // callback-scoped view is used.
            let input = unsafe {
                let buf = (*current)
                    .buf
                    .as_mut()
                    .ok_or(CompressionFailure::InvalidFfiState)?;
                InputBuffer::new(buf).map_err(|()| CompressionFailure::InvalidFfiState)?
            };
            let operation = input.operation();
            let outcome = {
                let started = self.stats.as_ref().map(|_| Instant::now());
                let mut output = NgxOutput {
                    request,
                    out: &mut self.out,
                    free: &mut self.free,
                    buffer_size: self.buffer_size,
                    produced: 0,
                    track_produced: started.is_some(),
                    buffer_count: self.buffer_count,
                    allocated_buffers: &mut self.allocated_buffers,
                };
                let outcome = drive_input(
                    &mut *self.codec,
                    operation,
                    input.bytes(),
                    &mut output,
                    &mut budget,
                );
                if let (Some(stats), Some(started)) = (&mut self.stats, started) {
                    let consumed = outcome
                        .as_ref()
                        .map_or_else(|failure| failure.consumed, |success| success.consumed);
                    stats.record(consumed, output.produced, started.elapsed());
                }
                outcome
            };
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(failure) => {
                    let fully_consumed = input
                        .consume(failure.consumed)
                        .map_err(|()| CompressionFailure::InvalidFfiState)?;
                    if fully_consumed {
                        // SAFETY: current is the request-owned pending head.
                        self.input = unsafe { (*current).next };
                    }
                    if fully_consumed
                        && matches!(failure.error, DriveError::Output(OutputFailure::Exhausted))
                    {
                        self.pending_operation = Some(operation);
                    }
                    return map_failure(failure.error);
                }
            };
            let fully_consumed = input
                .consume(outcome.consumed)
                .map_err(|()| CompressionFailure::InvalidFfiState)?;
            if fully_consumed {
                // SAFETY: current is the request-owned pending head.
                self.input = unsafe { (*current).next };
            }
            if fully_consumed && outcome.state == DriveState::BudgetExhausted {
                self.pending_operation = Some(operation);
            }
            self.done = outcome.state == DriveState::Finished;

            if outcome.state == DriveState::BudgetExhausted {
                return Ok(true);
            }
            if !fully_consumed {
                return Err(CompressionFailure::InvalidCodecProgress);
            }
            if self.done {
                self.input = ptr::null_mut();
                return Ok(false);
            }
        }
        Ok(false)
    }
}

fn resume_operation(
    ctx: &mut RequestCtx,
    request: *mut ngx_http_request_t,
    budget: &mut WorkBudget,
) -> Result<Option<bool>, CompressionFailure> {
    let Some(operation) = ctx.pending_operation else {
        return Ok(None);
    };
    let outcome = {
        let started = ctx.stats.as_ref().map(|_| Instant::now());
        let mut output = NgxOutput {
            request,
            out: &mut ctx.out,
            free: &mut ctx.free,
            buffer_size: ctx.buffer_size,
            produced: 0,
            track_produced: started.is_some(),
            buffer_count: ctx.buffer_count,
            allocated_buffers: &mut ctx.allocated_buffers,
        };
        let outcome = drive_input(&mut *ctx.codec, operation, &[], &mut output, budget);
        if let (Some(stats), Some(started)) = (&mut ctx.stats, started) {
            let consumed = outcome
                .as_ref()
                .map_or_else(|failure| failure.consumed, |success| success.consumed);
            stats.record(consumed, output.produced, started.elapsed());
        }
        outcome
    };
    match outcome {
        Ok(outcome) => {
            ctx.done = outcome.state == DriveState::Finished;
            if outcome.state == DriveState::BudgetExhausted {
                return Ok(Some(true));
            }
            ctx.pending_operation = None;
            if ctx.done {
                ctx.input = ptr::null_mut();
                return Ok(Some(false));
            }
            Ok(None)
        }
        Err(failure) => {
            debug_assert_eq!(failure.consumed, 0);
            map_failure(failure.error).map(Some)
        }
    }
}

fn map_failure(error: DriveError<OutputFailure>) -> Result<bool, CompressionFailure> {
    match error {
        DriveError::Output(OutputFailure::Exhausted) => Ok(true),
        DriveError::Step(StepError::Codec(_)) => Err(CompressionFailure::CodecBackend),
        DriveError::Step(StepError::InvalidProgress(_)) => {
            Err(CompressionFailure::InvalidCodecProgress)
        }
        DriveError::Output(OutputFailure::Allocation) => Err(CompressionFailure::OutputAllocation),
        DriveError::Output(OutputFailure::InvalidFfiState) => {
            Err(CompressionFailure::InvalidFfiState)
        }
    }
}

/// Appends request-pool chain-link copies without copying buffer storage.
unsafe fn append_input(
    ctx: &mut RequestCtx,
    request: *mut ngx_http_request_t,
    mut chain: *mut ngx_chain_t,
) -> Result<(), CompressionFailure> {
    if chain.is_null() {
        return Ok(());
    }
    let mut tail = &raw mut ctx.input;
    // SAFETY: ctx.input is a request-owned chain.
    unsafe {
        while !(*tail).is_null() {
            tail = &raw mut (**tail).next;
        }
        while !chain.is_null() {
            let mut pending = ctx.input;
            let mut duplicate = false;
            while !pending.is_null() {
                if (*pending).buf == (*chain).buf {
                    duplicate = true;
                    break;
                }
                pending = (*pending).next;
            }
            if duplicate {
                chain = (*chain).next;
                continue;
            }
            let link = ngx_palloc((*request).pool, core::mem::size_of::<ngx_chain_t>())
                .cast::<ngx_chain_t>();
            if link.is_null() {
                return Err(CompressionFailure::OutputAllocation);
            }
            (*link).buf = (*chain).buf;
            (*link).next = ptr::null_mut();
            *tail = link;
            tail = &raw mut (*link).next;
            chain = (*chain).next;
        }
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
    buffer_count: usize,
    allocated_buffers: &mut usize,
) -> Result<*mut ngx_chain_t, OutputFailure> {
    if fault::take(Point::OutputAllocation) {
        return Err(OutputFailure::Allocation);
    }
    if (*free).is_null() && *allocated_buffers >= buffer_count {
        return Err(OutputFailure::Exhausted);
    }
    // SAFETY: pool is valid; ngx_chain_get_free_buf reuses or allocates a link.
    unsafe {
        let pool = (*request).pool;
        let link = ngx_chain_get_free_buf(pool, free);
        if link.is_null() {
            return Err(OutputFailure::Allocation);
        }
        let buf = (*link).buf;
        if (*buf).start.is_null() {
            let memory = ngx_palloc(pool, buffer_size).cast::<u8>();
            if memory.is_null() {
                return Err(OutputFailure::Allocation);
            }
            (*buf).start = memory;
            (*buf).end = memory.add(buffer_size);
            (*buf).tag = ptr::addr_of!(ngx_http_compress_module).cast_mut().cast();
            *allocated_buffers += 1;
        }
        ngx_compress_ffi::buffer::prepare_output(&mut *buf);
        Ok(link)
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
