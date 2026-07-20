use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};

/// The `identity` coding: bytes pass through unchanged.
///
/// It holds no state, so it doubles as the reference implementation of
/// [`StreamingCodec`] and as the uniform pass-through path for the body filter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Identity;

impl StreamingCodec for Identity {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Identity
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let moved = input.len().min(output.len());
        output[..moved].copy_from_slice(&input[..moved]);

        let drained = moved == input.len();
        let state = match operation {
            _ if !drained => StepState::NeedsOutput,
            Operation::Continue => StepState::NeedsInput,
            Operation::Flush | Operation::Finish => StepState::Complete,
        };

        Ok(StepResult {
            consumed: moved,
            produced: moved,
            state,
        })
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::Identity;
    use ngx_compress_core::{
        ContentCoding, Operation, StepResult, StepState, StreamingCodec, validate_progress,
    };

    fn stepped(operation: Operation, input: &[u8], capacity: usize) -> (Vec<u8>, StepResult) {
        let mut output = vec![0_u8; capacity];
        let mut codec = Identity;
        let Ok(result) = codec.step(operation, input, &mut output) else {
            unreachable!("identity codec never returns an error");
        };
        assert!(
            validate_progress(operation, input.len(), capacity, result).is_ok(),
            "identity step must satisfy the progress contract"
        );
        output.truncate(result.produced);
        (output, result)
    }

    #[test]
    fn reports_identity_coding() {
        assert_eq!(Identity.coding(), ContentCoding::Identity);
    }

    #[test]
    fn passes_bytes_through_and_finishes() {
        let (output, result) = stepped(Operation::Finish, b"hello world", 64);

        assert_eq!(output, b"hello world");
        assert_eq!(result.state, StepState::Complete);
    }

    #[test]
    fn signals_output_backpressure_when_capacity_is_short() {
        let (output, result) = stepped(Operation::Continue, b"hello world", 5);

        assert_eq!(output, b"hello");
        assert_eq!(result.consumed, 5);
        assert_eq!(result.state, StepState::NeedsOutput);
    }

    #[test]
    fn continue_requests_more_input_after_draining() {
        let (_, result) = stepped(Operation::Continue, b"abc", 64);

        assert_eq!(result.consumed, 3);
        assert_eq!(result.state, StepState::NeedsInput);
    }

    #[test]
    fn empty_finish_completes_without_output() {
        let (output, result) = stepped(Operation::Finish, b"", 64);

        assert!(output.is_empty());
        assert_eq!(result.state, StepState::Complete);
    }
}
