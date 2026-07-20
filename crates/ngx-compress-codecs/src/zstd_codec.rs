//! The `zstd` coding, backed by the zstd library's raw streaming encoder.

use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};
use zstd::stream::raw::{Encoder, Operation as ZstdOperation};
use zstd::zstd_safe::{InBuffer, OutBuffer};

/// The `zstd` coding. Levels 1–22 (negative fast levels also accepted).
pub struct Zstd {
    encoder: Encoder<'static>,
    level: i32,
}

impl Zstd {
    /// Creates a zstd encoder at the given compression level.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the zstd backend cannot create the context.
    pub fn new(level: i32) -> Result<Self, CodecError> {
        // style:allow-non-openssl (zstd is not an OpenSSL-provided codec)
        Encoder::new(level)
            .map(|encoder| Self { encoder, level })
            .map_err(|_| CodecError::Backend)
    }
}

impl StreamingCodec for Zstd {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Zstd
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let mut out_buffer = OutBuffer::around(output);
        // Only feed input through `run`; once it drains, `flush`/`finish` take
        // over. Re-running with empty input mid-finish would restart the frame.
        let consumed = if input.is_empty() {
            0
        } else {
            let mut in_buffer = InBuffer::around(input);
            self.encoder
                .run(&mut in_buffer, &mut out_buffer)
                .map_err(|_| CodecError::Backend)?;
            in_buffer.pos
        };

        let state = if consumed < input.len() {
            // The output filled before all input was accepted.
            StepState::NeedsOutput
        } else {
            match operation {
                Operation::Continue => StepState::NeedsInput,
                Operation::Flush => drain_state(
                    self.encoder
                        .flush(&mut out_buffer)
                        .map_err(|_| CodecError::Backend)?,
                ),
                Operation::Finish => drain_state(
                    self.encoder
                        .finish(&mut out_buffer, true)
                        .map_err(|_| CodecError::Backend)?,
                ),
            }
        };

        Ok(StepResult {
            consumed,
            produced: out_buffer.pos(),
            state,
        })
    }

    fn reset(&mut self) {
        // Reuse the context; on the rare reinit failure, rebuild so the next
        // request starts from a clean encoder rather than a poisoned one.
        if self.encoder.reinit().is_err() {
            if let Ok(encoder) = Encoder::new(self.level) {
                self.encoder = encoder;
            }
        }
    }
}

/// Maps a zstd flush/finish "bytes still buffered" count to a step state.
fn drain_state(remaining: usize) -> StepState {
    if remaining == 0 {
        StepState::Complete
    } else {
        StepState::NeedsOutput
    }
}
