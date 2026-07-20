#![forbid(unsafe_code)]

mod negotiation;
mod progress;

pub use negotiation::{AcceptEncoding, ContentCoding};
pub use progress::{Operation, ProgressError, StepResult, StepState, validate_progress};
