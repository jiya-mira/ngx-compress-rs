//! The system-libbrotli encoder FFI and the `SystemBrotli` codec adapter.

use core::ffi::c_void;
use core::ptr;

use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};

/// Opaque `BrotliEncoderState`.
type EncoderState = c_void;

/// `BrotliEncoderOperation` values.
#[repr(C)]
enum EncoderOperation {
    Process = 0,
    Flush = 1,
    Finish = 2,
}

/// `BrotliEncoderParameter` values we set.
#[repr(C)]
enum EncoderParameter {
    Quality = 1,
    Lgwin = 2,
}

type AllocFunc = Option<unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void>;
type FreeFunc = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

// SAFETY: declarations of the libbrotli C encoder API; resolved at final link.
unsafe extern "C" {
    fn BrotliEncoderCreateInstance(
        alloc_func: AllocFunc,
        free_func: FreeFunc,
        opaque: *mut c_void,
    ) -> *mut EncoderState;
    fn BrotliEncoderDestroyInstance(state: *mut EncoderState);
    fn BrotliEncoderSetParameter(
        state: *mut EncoderState,
        param: EncoderParameter,
        value: u32,
    ) -> i32;
    fn BrotliEncoderCompressStream(
        state: *mut EncoderState,
        op: EncoderOperation,
        available_in: *mut usize,
        next_in: *mut *const u8,
        available_out: *mut usize,
        next_out: *mut *mut u8,
        total_out: *mut usize,
    ) -> i32;
    fn BrotliEncoderIsFinished(state: *mut EncoderState) -> i32;
    fn BrotliEncoderHasMoreOutput(state: *mut EncoderState) -> i32;
}

/// The `br` coding via the system libbrotli encoder. Quality 0–11, window 10–24.
pub struct SystemBrotli {
    state: *mut EncoderState,
    quality: u32,
    window: u32,
}

impl SystemBrotli {
    /// Creates a brotli encoder at the given quality and window bits.
    #[must_use]
    pub fn new(quality: u32, window: u32) -> Self {
        Self {
            state: create(quality, window),
            quality,
            window,
        }
    }
}

fn create(quality: u32, window: u32) -> *mut EncoderState {
    // SAFETY: NULL allocators request libbrotli's default allocator; on success
    // the returned state is configured with the quality and window parameters.
    unsafe {
        let state = BrotliEncoderCreateInstance(None, None, ptr::null_mut());
        if !state.is_null() {
            BrotliEncoderSetParameter(state, EncoderParameter::Quality, quality);
            BrotliEncoderSetParameter(state, EncoderParameter::Lgwin, window);
        }
        state
    }
}

const fn map_operation(operation: Operation) -> EncoderOperation {
    match operation {
        Operation::Continue => EncoderOperation::Process,
        Operation::Flush => EncoderOperation::Flush,
        Operation::Finish => EncoderOperation::Finish,
    }
}

impl StreamingCodec for SystemBrotli {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Brotli
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        if self.state.is_null() {
            return Err(CodecError::Backend);
        }

        let mut available_in = input.len();
        let mut next_in = input.as_ptr();
        let mut available_out = output.len();
        let mut next_out = output.as_mut_ptr();
        let mut total_out = 0;

        // SAFETY: `state` is non-null; brotli reads/advances the in/out cursors
        // within the provided lengths.
        let ok = unsafe {
            BrotliEncoderCompressStream(
                self.state,
                map_operation(operation),
                &raw mut available_in,
                &raw mut next_in,
                &raw mut available_out,
                &raw mut next_out,
                &raw mut total_out,
            )
        };
        if ok == 0 {
            return Err(CodecError::Backend);
        }

        let consumed = input.len() - available_in;
        // SAFETY: `state` is non-null.
        let finished = unsafe { BrotliEncoderIsFinished(self.state) } != 0;
        // SAFETY: `state` is non-null.
        let has_more = unsafe { BrotliEncoderHasMoreOutput(self.state) } != 0;
        let state = if finished {
            StepState::Complete
        } else if consumed < input.len() {
            StepState::NeedsOutput
        } else {
            match operation {
                Operation::Continue => StepState::NeedsInput,
                Operation::Flush | Operation::Finish if has_more => StepState::NeedsOutput,
                Operation::Flush | Operation::Finish => StepState::Complete,
            }
        };

        Ok(StepResult {
            consumed,
            produced: output.len() - available_out,
            state,
        })
    }

    fn reset(&mut self) -> Result<(), CodecError> {
        // SAFETY: destroy the current instance (if any) before rebuilding.
        unsafe {
            if !self.state.is_null() {
                BrotliEncoderDestroyInstance(self.state);
            }
        }
        self.state = create(self.quality, self.window);
        if self.state.is_null() {
            Err(CodecError::Backend)
        } else {
            Ok(())
        }
    }
}

impl Drop for SystemBrotli {
    fn drop(&mut self) {
        // SAFETY: destroy the instance this codec created.
        unsafe {
            if !self.state.is_null() {
                BrotliEncoderDestroyInstance(self.state);
            }
        }
    }
}
