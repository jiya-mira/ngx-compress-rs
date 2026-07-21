#![cfg(any(
    feature = "gzip",
    feature = "deflate",
    feature = "brotli",
    feature = "zstd"
))]

//! Round-trip tests: drive each codec through the streaming step contract with
//! varying output buffer sizes (to exercise backpressure), then decode the
//! result with a reference decoder and assert it equals the input.

use ngx_compress_core::{Operation, StepState, StreamingCodec, validate_progress};

fn sample() -> Vec<u8> {
    let unit = b"the quick brown fox jumps over the lazy dog 0123456789\n";
    unit.iter().copied().cycle().take(16_384).collect()
}

/// Runs a codec to completion, feeding `Continue` until the input drains and
/// `Finish` afterwards, writing into a fixed `chunk`-sized output buffer.
fn compress(codec: &mut dyn StreamingCodec, input: &[u8], chunk: usize) -> Vec<u8> {
    let mut output = vec![0_u8; chunk];
    let mut result = Vec::new();
    let mut consumed_total = 0;
    // Bounded to guarantee termination even if a codec violated its contract.
    for _ in 0..1_000_000 {
        // style:allow-for-in
        let remaining = &input[consumed_total..];
        let operation = if remaining.is_empty() {
            Operation::Finish
        } else {
            Operation::Continue
        };
        let Ok(step) = codec.step(operation, remaining, &mut output) else {
            unreachable!("codec step returned an error");
        };
        assert!(
            validate_progress(operation, remaining.len(), output.len(), step).is_ok(),
            "codec violated the progress contract"
        );
        result.extend_from_slice(&output[..step.produced]);
        consumed_total += step.consumed;
        if operation == Operation::Finish && step.state == StepState::Complete {
            return result;
        }
    }
    unreachable!("codec did not reach Complete");
}

#[cfg(feature = "gzip")]
#[test]
fn gzip_roundtrips_across_buffer_sizes() {
    use std::io::Read;
    let input = sample();
    for chunk in [4_usize, 91, 8_192] {
        // style:allow-for-in
        let mut codec = ngx_compress_codecs::Gzip::new(6);
        let compressed = compress(&mut codec, &input, chunk);
        let mut decoded = Vec::new();
        assert!(
            flate2::read::GzDecoder::new(&compressed[..])
                .read_to_end(&mut decoded)
                .is_ok()
        );
        assert_eq!(decoded, input, "gzip mismatch at chunk {chunk}");
    }
}

#[cfg(feature = "deflate")]
#[test]
fn deflate_roundtrips_across_buffer_sizes() {
    use std::io::Read;
    let input = sample();
    for chunk in [4_usize, 91, 8_192] {
        // style:allow-for-in
        let mut codec = ngx_compress_codecs::Deflate::new(6);
        let compressed = compress(&mut codec, &input, chunk);
        let mut decoded = Vec::new();
        assert!(
            flate2::read::ZlibDecoder::new(&compressed[..])
                .read_to_end(&mut decoded)
                .is_ok()
        );
        assert_eq!(decoded, input, "deflate mismatch at chunk {chunk}");
    }
}

#[cfg(feature = "brotli")]
#[test]
fn brotli_roundtrips_across_buffer_sizes() {
    use std::io::Read;
    let input = sample();
    for chunk in [4_usize, 91, 8_192] {
        // style:allow-for-in
        let mut codec = ngx_compress_codecs::Brotli::new(5, 22);
        let compressed = compress(&mut codec, &input, chunk);
        let mut decoded = Vec::new();
        assert!(
            brotli::Decompressor::new(&compressed[..], 4_096)
                .read_to_end(&mut decoded)
                .is_ok()
        );
        assert_eq!(decoded, input, "brotli mismatch at chunk {chunk}");
    }
}

#[cfg(feature = "zstd")]
#[test]
fn zstd_roundtrips_across_buffer_sizes() {
    let input = sample();
    for chunk in [4_usize, 91, 8_192] {
        // style:allow-for-in
        let Ok(mut codec) = ngx_compress_codecs::Zstd::new(9) else {
            unreachable!("zstd encoder creation failed");
        };
        let compressed = compress(&mut codec, &input, chunk);
        let Ok(decoded) = zstd::decode_all(&compressed[..]) else {
            unreachable!("zstd decode failed");
        };
        assert_eq!(decoded, input, "zstd mismatch at chunk {chunk}");
    }
}

#[cfg(feature = "gzip")]
#[test]
fn gzip_handles_empty_input() {
    use std::io::Read;
    let mut codec = ngx_compress_codecs::Gzip::new(6);
    let compressed = compress(&mut codec, b"", 8_192);
    let mut decoded = Vec::new();
    assert!(
        flate2::read::GzDecoder::new(&compressed[..])
            .read_to_end(&mut decoded)
            .is_ok()
    );
    assert!(decoded.is_empty());
}
