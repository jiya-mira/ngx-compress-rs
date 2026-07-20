//! The `br` (Brotli) coding, backed by the brotli library's streaming encoder.

use brotli::enc::StandardAlloc;
use brotli::enc::encode::{
    BrotliEncoderOperation, BrotliEncoderParameter, BrotliEncoderStateStruct,
};
use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};

/// The `br` coding. Quality 0–11, window (lgwin) 10–24.
pub struct Brotli {
    state: BrotliEncoderStateStruct<StandardAlloc>,
    quality: u32,
    window: u32,
}

impl Brotli {
    /// Creates a brotli encoder at the given quality and window bits.
    #[must_use]
    pub fn new(quality: u32, window: u32) -> Self {
        Self {
            state: build_state(quality, window),
            quality,
            window,
        }
    }
}

fn build_state(quality: u32, window: u32) -> BrotliEncoderStateStruct<StandardAlloc> {
    // style:allow-non-openssl (brotli is not an OpenSSL-provided codec)
    let mut state = BrotliEncoderStateStruct::new(StandardAlloc::default());
    state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_QUALITY, quality);
    state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_LGWIN, window);
    state
}

const fn map_operation(operation: Operation) -> BrotliEncoderOperation {
    match operation {
        Operation::Continue => BrotliEncoderOperation::BROTLI_OPERATION_PROCESS,
        Operation::Flush => BrotliEncoderOperation::BROTLI_OPERATION_FLUSH,
        Operation::Finish => BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
    }
}

impl StreamingCodec for Brotli {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Brotli
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let mut available_in = input.len();
        let mut input_offset = 0;
        let mut available_out = output.len();
        let mut output_offset = 0;
        let mut total_out = None;

        let ok = self.state.compress_stream(
            map_operation(operation),
            &mut available_in,
            input,
            &mut input_offset,
            &mut available_out,
            output,
            &mut output_offset,
            &mut total_out,
            &mut |_, _, _, _| (),
        );
        if !ok {
            return Err(CodecError::Backend);
        }

        let consumed = input_offset;
        let state = if self.state.is_finished() {
            StepState::Complete
        } else if consumed < input.len() {
            StepState::NeedsOutput
        } else {
            match operation {
                Operation::Continue => StepState::NeedsInput,
                Operation::Flush | Operation::Finish if self.state.has_more_output() => {
                    StepState::NeedsOutput
                }
                Operation::Flush | Operation::Finish => StepState::Complete,
            }
        };

        Ok(StepResult {
            consumed,
            produced: output_offset,
            state,
        })
    }

    fn reset(&mut self) {
        self.state = build_state(self.quality, self.window);
    }
}
