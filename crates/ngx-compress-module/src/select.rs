//! Server-side codec selection: honor the client's `Accept-Encoding` quality
//! values, break ties by the default priority order, and build the codec.

use crate::conf::Resolved;
use ngx_compress_core::{AcceptEncoding, ContentCoding, StreamingCodec};

/// Default tie-break order for equal client quality (no server standard exists).
const PRIORITY: [ContentCoding; 4] = [
    ContentCoding::Zstd,
    ContentCoding::Brotli,
    ContentCoding::Gzip,
    ContentCoding::Deflate,
];

/// Selects a coding and builds its codec, or `None` to send identity.
/// style:allow-pub-crate
pub(crate) fn choose(
    resolved: &Resolved,
    accept: &AcceptEncoding,
) -> Option<Box<dyn StreamingCodec>> {
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

/// Whether a coding is enabled and its backend is compiled into this build.
fn available(resolved: &Resolved, coding: ContentCoding) -> bool {
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

fn build(resolved: &Resolved, coding: ContentCoding) -> Option<Box<dyn StreamingCodec>> {
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
