#![forbid(unsafe_code)]

//! Worker-local reuse of codec contexts.
//!
//! Allocating an encoder per request is wasteful: zstd rebuilds a `CCtx` and
//! flate re-runs `deflateInit`. NGINX workers are single-threaded event-loop
//! *processes*, so a `thread_local` free list is naturally per-worker with no
//! cross-worker sharing and no request-path locking (architecture §"Reuse codec
//! contexts within a worker"). A finished request returns its codec here instead
//! of dropping it; the next matching request pops it and [`StreamingCodec::reset`]
//! clears the prior stream before reuse.
//!
//! Concurrency: the worker runs our filter and the pool-cleanup callback inline
//! on its one thread with no `.await` between borrow and release, so the
//! `RefCell` is never borrowed reentrantly.

use core::cell::RefCell;

use ngx_compress_core::{CodecError, StreamingCodec};

use super::CodecKey;

struct Entry {
    key: CodecKey,
    codec: Box<dyn StreamingCodec>,
}

/// Upper bound on idle codecs retained per worker. Kept small because a pooled
/// encoder holds its buffers (a large `compress_brotli_window` is megabytes), so
/// this caps idle memory; it is a first-cut value to be revisited with the M3
/// benchmark.
const MAX_IDLE: usize = 8;

thread_local! {
    static POOL: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

/// Takes a reset, ready-to-use codec matching `key` from this worker's pool, or
/// `None` if none is idle (the caller then builds a fresh one).
pub(in crate::filter) fn acquire(
    key: CodecKey,
) -> Result<Option<Box<dyn StreamingCodec>>, CodecError> {
    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        let Some(index) = pool.iter().position(|entry| entry.key == key) else {
            return Ok(None);
        };
        let mut entry = pool.swap_remove(index);
        // A failed reset consumes and drops the removed entry, so a poisoned
        // codec can never return to the pool or reach another request.
        entry.codec.reset()?;
        Ok(Some(entry.codec))
    })
}

/// Returns a finished codec to this worker's pool for later reuse, dropping it if
/// the pool is at capacity.
pub(in crate::filter) fn release(key: CodecKey, codec: Box<dyn StreamingCodec>) {
    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_IDLE {
            pool.push(Entry { key, codec });
        }
        // Otherwise `codec` drops here, freeing its buffers.
    });
}

#[cfg(test)]
mod tests {
    use super::{CodecKey, acquire, release};
    use ngx_compress_codecs::Identity;
    use ngx_compress_core::{CodecError, ContentCoding, Operation, StepResult, StreamingCodec};

    struct ResetFails;

    impl StreamingCodec for ResetFails {
        fn coding(&self) -> ContentCoding {
            ContentCoding::Identity
        }

        fn step(
            &mut self,
            _operation: Operation,
            _input: &[u8],
            _output: &mut [u8],
        ) -> Result<StepResult, CodecError> {
            Err(CodecError::Backend)
        }

        fn reset(&mut self) -> Result<(), CodecError> {
            Err(CodecError::Backend)
        }
    }

    fn identity_key() -> CodecKey {
        CodecKey::new(ContentCoding::Identity, 0, 0)
    }

    #[test]
    fn acquires_a_codec_after_a_successful_reset() {
        let key = identity_key();
        release(key, Box::new(Identity));

        assert!(matches!(acquire(key), Ok(Some(_))));
    }

    #[test]
    fn discards_a_codec_when_reset_fails() {
        let key = identity_key();
        release(key, Box::new(ResetFails));

        assert_eq!(acquire(key).err(), Some(CodecError::Backend));
    }
}
