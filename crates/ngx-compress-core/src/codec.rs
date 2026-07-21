use crate::{ContentCoding, Operation, ProgressError, StepResult, validate_progress};

/// An unrecoverable failure reported by a codec adapter.
///
/// Compression backends can fail (internal library error, invalid state). A
/// codec surfaces the failure here instead of panicking so the FFI boundary can
/// map it to a documented NGINX status rather than unwinding across the C ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// The underlying compression backend reported an error and the stream
    /// cannot continue.
    Backend,
}

/// A codec failure or a violation of the streaming progress contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepError {
    /// The codec backend failed while advancing the stream.
    Codec(CodecError),
    /// The codec returned byte counts or state that violate the contract.
    InvalidProgress(ProgressError),
}

/// A streaming encoder adapter over an established compression library.
///
/// This trait is the stable extension seam described in the crate design: a new
/// content coding is added by implementing it, whether in-tree behind a Cargo
/// feature or out-of-tree in a separate crate. Every [`StreamingCodec::step`]
/// must satisfy the progress contract enforced by
/// [`validate_progress`](crate::validate_progress) — it reports the input it
/// consumed, the output it produced, and the state it is now in.
pub trait StreamingCodec {
    /// The content coding this adapter emits.
    fn coding(&self) -> ContentCoding;

    /// Advances the encoder by one step, reading from `input` and writing into
    /// `output`. On success the returned [`StepResult`] must never claim to
    /// consume more than `input.len()` or produce more than `output.len()`.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] when the backend fails unrecoverably.
    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError>;

    /// Returns the adapter to its initial state so a worker can reuse the
    /// context across requests without reallocating it.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] when the backend cannot restore a clean initial
    /// state. Callers must discard the codec instead of reusing it after an
    /// error.
    fn reset(&mut self) -> Result<(), CodecError>;
}

/// Advances a codec and validates its result before the caller can trust its
/// consumed/produced byte counts.
///
/// # Errors
///
/// Returns [`StepError::Codec`] for backend failures or
/// [`StepError::InvalidProgress`] when the adapter violates the streaming
/// contract.
pub fn checked_step<C: StreamingCodec + ?Sized>(
    codec: &mut C,
    operation: Operation,
    input: &[u8],
    output: &mut [u8],
) -> Result<StepResult, StepError> {
    let input_available = input.len();
    let output_capacity = output.len();
    let result = codec
        .step(operation, input, output)
        .map_err(StepError::Codec)?;
    validate_progress(operation, input_available, output_capacity, result)
        .map_err(StepError::InvalidProgress)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{CodecError, StepError, StreamingCodec, checked_step};
    use crate::{ContentCoding, Operation, ProgressError, StepResult, StepState};

    struct InvalidCodec;

    struct ValidCodec;

    impl StreamingCodec for ValidCodec {
        fn coding(&self) -> ContentCoding {
            ContentCoding::Identity
        }

        fn step(
            &mut self,
            _operation: Operation,
            input: &[u8],
            output: &mut [u8],
        ) -> Result<StepResult, CodecError> {
            let consumed = input.len().min(output.len());
            Ok(StepResult {
                consumed,
                produced: consumed,
                state: if consumed == input.len() {
                    StepState::NeedsInput
                } else {
                    StepState::NeedsOutput
                },
            })
        }

        fn reset(&mut self) -> Result<(), CodecError> {
            Ok(())
        }
    }

    impl StreamingCodec for InvalidCodec {
        fn coding(&self) -> ContentCoding {
            ContentCoding::Gzip
        }

        fn step(
            &mut self,
            _operation: Operation,
            input: &[u8],
            _output: &mut [u8],
        ) -> Result<StepResult, CodecError> {
            Ok(StepResult {
                consumed: input.len() + 1,
                produced: 0,
                state: StepState::NeedsInput,
            })
        }

        fn reset(&mut self) -> Result<(), CodecError> {
            Ok(())
        }
    }

    #[test]
    fn accepts_valid_codec_progress() {
        let mut codec = ValidCodec;
        let mut output = [0_u8; 16];

        let result = checked_step(&mut codec, Operation::Continue, b"input", &mut output);

        assert_eq!(
            result,
            Ok(StepResult {
                consumed: 5,
                produced: 5,
                state: StepState::NeedsInput,
            })
        );
    }

    #[test]
    fn rejects_invalid_codec_progress_before_the_caller_uses_it() {
        let mut codec = InvalidCodec;
        let mut output = [0_u8; 16];

        let result = checked_step(&mut codec, Operation::Continue, b"input", &mut output);

        assert_eq!(
            result,
            Err(StepError::InvalidProgress(ProgressError::ConsumedPastInput))
        );
    }
}
