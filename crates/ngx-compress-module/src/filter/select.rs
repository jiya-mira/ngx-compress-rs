#![forbid(unsafe_code)]

//! Server-side codec selection: honor the client's `Accept-Encoding` quality
//! values, break ties by the default priority order, and build the codec.

use super::{CodecKey, CodecPool, CodecSelection};
use crate::Resolved;
use ngx_compress_core::{AcceptEncoding, ContentCoding, StreamingCodec};

/// Default tie-break order for equal client quality (no server standard exists).
const PRIORITY: [ContentCoding; 4] = [
    ContentCoding::Zstd,
    ContentCoding::Brotli,
    ContentCoding::Gzip,
    ContentCoding::Deflate,
];

/// Selects a coding and provides its codec (reused from the worker pool when
/// possible) with the key needed to return it on cleanup, or `None` for
/// identity.
impl CodecSelection for CodecKey {
    fn choose(
        resolved: &Resolved<'_>,
        accept: &AcceptEncoding,
    ) -> Option<(Box<dyn StreamingCodec>, Self)> {
        let coding = PRIORITY
            .iter()
            .copied()
            .filter(|&coding| available(resolved, coding))
            .fold(None, |best: Option<(ContentCoding, u16)>, coding| {
                let quality = accept.quality(coding);
                match best {
                    Some((_, best_quality)) if quality <= best_quality => best,
                    _ if quality > 0 => Some((coding, quality)),
                    _ => best,
                }
            })
            .map(|(coding, _)| coding)?;
        build(resolved, coding)
    }
}

/// Whether a coding is enabled and its backend is compiled into this build.
fn available(resolved: &Resolved<'_>, coding: ContentCoding) -> bool {
    match coding {
        ContentCoding::Gzip => cfg!(feature = "gzip") && resolved.gzip.is_some(),
        ContentCoding::Deflate => cfg!(feature = "deflate") && resolved.deflate.is_some(),
        ContentCoding::Brotli => cfg!(feature = "brotli") && resolved.brotli.is_some(),
        ContentCoding::Zstd => cfg!(feature = "zstd") && resolved.zstd.is_some(),
        _ => false,
    }
}

fn boxed<C: StreamingCodec + 'static>(codec: C) -> Box<dyn StreamingCodec> {
    Box::new(codec)
}

/// Provides a codec for `coding`: a reset instance from the worker pool if one
/// is idle, otherwise a freshly built one. Returns it with its pool key.
fn build(
    resolved: &Resolved<'_>,
    coding: ContentCoding,
) -> Option<(Box<dyn StreamingCodec>, CodecKey)> {
    let level = level_i32(resolved, coding)?;
    let window = match coding {
        ContentCoding::Brotli => resolved.brotli.map_or(0, |(_, window)| window),
        _ => 0,
    };
    let key = CodecKey::new(coding, level, window);
    let codec = match key.acquire() {
        Ok(Some(codec)) => codec,
        // A missing pooled codec or one that failed to reset is replaced before
        // this request starts; the failed instance was already dropped.
        Ok(None) | Err(_) => construct(resolved, coding)?,
    };
    Some((codec, key))
}

/// The discriminating compression level for a coding's pool key.
fn level_i32(resolved: &Resolved<'_>, coding: ContentCoding) -> Option<i32> {
    let level = match coding {
        ContentCoding::Gzip => i32::try_from(resolved.gzip?).ok()?,
        ContentCoding::Deflate => i32::try_from(resolved.deflate?).ok()?,
        ContentCoding::Brotli => i32::try_from(resolved.brotli?.0).ok()?,
        ContentCoding::Zstd => resolved.zstd?,
        _ => return None,
    };
    Some(level)
}

/// Builds a fresh codec (pool miss). Kept separate so `build` can prefer reuse.
fn construct(resolved: &Resolved<'_>, coding: ContentCoding) -> Option<Box<dyn StreamingCodec>> {
    match coding {
        #[cfg(feature = "gzip")]
        ContentCoding::Gzip => resolved
            .gzip
            .map(|level| boxed(ngx_compress_codecs::Gzip::new(level))),
        #[cfg(feature = "deflate")]
        ContentCoding::Deflate => resolved
            .deflate
            .map(|level| boxed(ngx_compress_codecs::Deflate::new(level))),
        #[cfg(feature = "brotli")]
        ContentCoding::Brotli => resolved
            .brotli
            .map(|(quality, window)| boxed(ngx_compress_codecs::Brotli::new(quality, window))),
        #[cfg(feature = "zstd")]
        ContentCoding::Zstd => resolved
            .zstd
            .and_then(|level| ngx_compress_codecs::Zstd::new(level).ok())
            .map(boxed),
        _ => None,
    }
}
