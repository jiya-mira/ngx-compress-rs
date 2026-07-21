#![forbid(unsafe_code)]

//! Safe header-filter policy: turn owned response facts into a complete plan.

use ngx_compress_core::{CompressionPolicy, eligible};

use super::{CodecKey, CodecSelection, HeaderDecision, Plan, Snapshot};
use crate::Resolved;

/// Applies eligibility, negotiation, and codec selection without raw nginx data.
impl HeaderDecision for Plan {
    fn decide(resolved: &Resolved<'_>, snapshot: &Snapshot) -> Option<Self> {
        let policy = CompressionPolicy {
            enabled: resolved.enabled,
            min_length: resolved.min_length,
            types: resolved.types,
        };
        if !eligible(policy, &snapshot.facts) {
            return None;
        }

        let (codec, key) = CodecKey::choose(resolved, &snapshot.accept_encoding)?;
        let coding = codec.coding();
        Some(Self {
            codec,
            key,
            coding,
            vary: resolved.vary,
            buffer_size: resolved.buffer_size,
        })
    }
}
