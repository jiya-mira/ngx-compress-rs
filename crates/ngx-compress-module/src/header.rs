#![forbid(unsafe_code)]

//! Safe header-filter policy: turn owned response facts into a complete plan.

use ngx_compress_core::{
    AcceptEncoding, CompressionPolicy, ContentCoding, ResponseFacts, StreamingCodec, eligible,
};

use crate::config::Resolved;
use crate::select;
use crate::worker::CodecKey;

/// Owned values prefetched from one nginx request/response.
pub(crate) struct Snapshot {
    pub facts: ResponseFacts,
    pub accept_encoding: AcceptEncoding,
}

/// Complete safe-core decision consumed by the FFI submit layer.
pub(crate) struct Plan {
    pub codec: Box<dyn StreamingCodec>,
    pub key: CodecKey,
    pub coding: ContentCoding,
    pub vary: bool,
    pub buffer_size: usize,
}

/// Applies eligibility, negotiation, and codec selection without raw nginx data.
pub(crate) fn decide(resolved: &Resolved<'_>, snapshot: &Snapshot) -> Option<Plan> {
    let policy = CompressionPolicy {
        enabled: resolved.enabled,
        min_length: resolved.min_length,
        types: resolved.types,
    };
    if !eligible(policy, &snapshot.facts) {
        return None;
    }

    let (codec, key) = select::choose(resolved, &snapshot.accept_encoding)?;
    let coding = codec.coding();
    Some(Plan {
        codec,
        key,
        coding,
        vary: resolved.vary,
        buffer_size: resolved.buffer_size,
    })
}
