#![forbid(unsafe_code)]

mod codec;
mod eligibility;
mod mime;
mod negotiation;
mod progress;
mod static_policy;
mod stream;

pub use codec::{CodecError, StepError, StreamingCodec, checked_step};
pub use eligibility::{CompressionPolicy, ResponseFacts, eligible};
pub use mime::{MimeTypes, compressible};
pub use negotiation::{AcceptEncoding, ContentCoding};
pub use progress::{Operation, ProgressError, StepResult, StepState, validate_progress};
pub use static_policy::{StaticCandidate, StaticMode, StaticRequestFacts, static_candidates};
pub use stream::{
    DriveError, DriveOutcome, OutputAction, OutputBoundary, OutputProvider, OutputUse, drive_input,
};
