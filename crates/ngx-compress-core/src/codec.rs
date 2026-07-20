use crate::{ContentCoding, Operation, StepResult};

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
    /// `output`. The returned [`StepResult`] must never claim to consume more
    /// than `input.len()` or produce more than `output.len()`.
    fn step(&mut self, operation: Operation, input: &[u8], output: &mut [u8]) -> StepResult;

    /// Returns the adapter to its initial state so a worker can reuse the
    /// context across requests without reallocating it.
    fn reset(&mut self);
}
