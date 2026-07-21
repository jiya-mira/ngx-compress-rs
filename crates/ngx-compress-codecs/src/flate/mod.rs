//! Shared flate2 stream machinery for the `deflate` and `gzip` codings.

#[cfg(feature = "deflate")]
mod deflate;
#[cfg(feature = "gzip")]
mod gzip;

#[cfg(feature = "deflate")]
pub use deflate::Deflate;
#[cfg(feature = "gzip")]
pub use gzip::Gzip;

use flate2::{Compress, Compression, FlushCompress, Status};
use ngx_compress_core::{CodecError, Operation, StepState};

const fn map_flush(operation: Operation) -> FlushCompress {
    match operation {
        Operation::Continue => FlushCompress::None,
        Operation::Flush => FlushCompress::Sync,
        Operation::Finish => FlushCompress::Finish,
    }
}

fn derive_state(
    operation: Operation,
    input_drained: bool,
    output_full: bool,
    ended: bool,
) -> StepState {
    if ended {
        return StepState::Complete;
    }
    match operation {
        Operation::Continue if input_drained => StepState::NeedsInput,
        Operation::Continue => StepState::NeedsOutput,
        _ if output_full => StepState::NeedsOutput,
        _ => StepState::Complete,
    }
}

fn delta(before: u64, after: u64) -> usize {
    usize::try_from(after.saturating_sub(before)).unwrap_or(usize::MAX)
}

struct FlateStep {
    consumed: usize,
    produced: usize,
    ended: bool,
}

/// Shared flate2 stream: the raw compress-a-slice step used by both codings.
struct FlateCore {
    compress: Compress,
}

impl FlateCore {
    fn new(level: u32, zlib_header: bool) -> Self {
        Self {
            compress: Compress::new(Compression::new(level), zlib_header),
        }
    }

    fn reset(&mut self) {
        // flate2's `reset` runs `deflateReset`, reusing the allocated zlib state
        // for a fresh stream at the same level and framing — the point of
        // worker-local codec reuse. It fully clears prior-stream state, so no
        // bytes carry over between responses.
        self.compress.reset();
    }

    fn run(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<FlateStep, CodecError> {
        let before_in = self.compress.total_in();
        let before_out = self.compress.total_out();
        let status = self
            .compress
            .compress(input, output, map_flush(operation))
            .map_err(|_| CodecError::Backend)?;
        Ok(FlateStep {
            consumed: delta(before_in, self.compress.total_in()),
            produced: delta(before_out, self.compress.total_out()),
            ended: status == Status::StreamEnd,
        })
    }
}
