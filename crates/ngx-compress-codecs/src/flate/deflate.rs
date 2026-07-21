//! RFC 9110 `deflate` coding backed by a zlib-framed flate2 stream.

use ngx_compress_core::{CodecError, ContentCoding, Operation, StepResult, StreamingCodec};

use super::{FlateCore, derive_state};

pub struct Deflate {
    core: FlateCore,
}

impl Deflate {
    /// Creates a deflate encoder at the given zlib level (1–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        Self {
            core: FlateCore::new(level, true),
        }
    }
}

impl StreamingCodec for Deflate {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Deflate
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let step = self.core.run(operation, input, output)?;
        let state = derive_state(
            operation,
            step.consumed == input.len(),
            step.produced == output.len(),
            step.ended,
        );
        Ok(StepResult {
            consumed: step.consumed,
            produced: step.produced,
            state,
        })
    }

    fn reset(&mut self) -> Result<(), CodecError> {
        self.core.reset();
        Ok(())
    }
}
