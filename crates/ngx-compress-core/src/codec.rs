use crate::{ContentCoding, Operation, StepResult};

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
    fn reset(&mut self);
}
