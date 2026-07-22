#![forbid(unsafe_code)]

//! Safe header-filter policy: turn owned response facts into a complete plan.

use ngx_compress_core::{CompressionPolicy, eligible};

use super::{CodecKey, CodecSelection, CodecSelectionFailure, HeaderDecision, Plan, Snapshot};
use crate::Resolved;

/// Applies eligibility, negotiation, and codec selection without raw nginx data.
impl HeaderDecision for Plan {
    fn decide(
        resolved: &Resolved<'_>,
        snapshot: &Snapshot,
    ) -> Result<Option<Self>, CodecSelectionFailure> {
        let policy = CompressionPolicy {
            enabled: resolved.enabled,
            min_length: resolved.min_length,
            types: resolved.types,
        };
        if !eligible(policy, &snapshot.facts) {
            return Ok(None);
        }

        let Some(selected) = CodecKey::choose(resolved, &snapshot.accept_encoding)? else {
            return Ok(None);
        };
        let coding = selected.codec.coding();
        Ok(Some(Self {
            codec: selected.codec,
            key: selected.key,
            coding,
            vary: resolved.vary,
            buffer_size: resolved.buffer_size,
            reset_recovered: selected.reset_recovered,
        }))
    }
}
