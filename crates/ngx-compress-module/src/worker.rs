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

use ngx_compress_core::{ContentCoding, StreamingCodec};

/// Identifies an interchangeable codec: same coding and same parameters, so a
/// pooled instance is a drop-in for a fresh one after `reset`. style:allow-pub-crate
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CodecKey {
    coding: ContentCoding,
    // Compression level; zstd allows negative fast levels, hence `i32`.
    level: i32,
    // Brotli window bits; `0` for codings without a window parameter.
    window: u32,
}

impl CodecKey {
    /// style:allow-pub-crate
    pub(crate) fn new(coding: ContentCoding, level: i32, window: u32) -> Self {
        Self {
            coding,
            level,
            window,
        }
    }
}

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
/// `None` if none is idle (the caller then builds a fresh one). style:allow-pub-crate
pub(crate) fn acquire(key: CodecKey) -> Option<Box<dyn StreamingCodec>> {
    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        let index = pool.iter().position(|entry| entry.key == key)?;
        let mut entry = pool.swap_remove(index);
        // Clear the previous stream's state before handing it back out.
        entry.codec.reset();
        Some(entry.codec)
    })
}

/// Returns a finished codec to this worker's pool for later reuse, dropping it if
/// the pool is at capacity. style:allow-pub-crate
pub(crate) fn release(key: CodecKey, codec: Box<dyn StreamingCodec>) {
    POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_IDLE {
            pool.push(Entry { key, codec });
        }
        // Otherwise `codec` drops here, freeing its buffers.
    });
}
