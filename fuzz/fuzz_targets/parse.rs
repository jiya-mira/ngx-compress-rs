#![no_main]

use libfuzzer_sys::fuzz_target;
use ngx_compress_core::{AcceptEncoding, ContentCoding};

// The Accept-Encoding parser must never panic on arbitrary bytes, and selection
// must stay within bounds. Run with: cargo +nightly fuzz run parse
fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let accepted = AcceptEncoding::parse(text);
        accepted.quality(ContentCoding::Gzip);
        accepted.select(&[
            ContentCoding::Zstd,
            ContentCoding::Brotli,
            ContentCoding::Gzip,
            ContentCoding::Identity,
        ]);
    }
});
