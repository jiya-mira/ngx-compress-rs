//! Codec compression matrix — the calibration harness behind the `compress`
//! profile tiers (M3 "benchmark-driven profiles").
//!
//! For each codec and level it reports the compression ratio and encode
//! throughput over a corpus, so the `fast`/`balanced`/`max` tier levels (and the
//! per-response-class priority order) can be chosen from data instead of guesses.
//!
//! Usage:
//!   cargo run --release -- <corpus-file>   # a real representative file
//!   cargo run --release                     # embedded synthetic sample
//!
//! Throughput is wall-clock over repeated single-threaded encodes; treat the
//! numbers as relative, and always calibrate against a real corpus.

use std::time::Instant;

use ngx_compress_codecs::{Brotli, Gzip, Zstd};
use ngx_compress_core::{Operation, StepState, StreamingCodec};

/// Synthetic fallback corpus (mixed markup/JSON/prose) used only when no file is
/// given; real calibration must pass a representative corpus.
const SAMPLE: &str = concat!(
    "<!doctype html><html><head><title>Example</title></head><body>",
    "<h1>Compression matrix</h1><p>The quick brown fox jumps over the lazy dog. ",
    "Pack my box with five dozen liquor jugs. </p>",
    r#"{"id":12345,"name":"widget","tags":["alpha","beta","gamma"],"active":true,"score":98.6}"#,
    "function render(items){return items.map(function(i){return i.name;}).join(', ');}",
    "</body></html>\n",
);

fn main() {
    let input = load();
    if input.is_empty() {
        eprintln!("empty input"); // style:allow-stdio-log
        return;
    }

    let mut report = String::new();
    report.push_str(&format!(
        "# codec compression matrix\n\ninput: {} bytes\n\n",
        input.len()
    ));
    report.push_str("| codec | level | window | ratio % | out bytes | MB/s |\n");
    report.push_str("|---|---|---|---|---|---|\n");

    for level in 1..=9u32 {
        let mut codec = Gzip::new(level);
        report.push_str(&row("gzip", i64::from(level), 0, &input, &mut codec));
    }
    for level in 0..=11u32 {
        let mut codec = Brotli::new(level, 22);
        report.push_str(&row("brotli", i64::from(level), 22, &input, &mut codec));
    }
    for level in [1i32, 3, 6, 9, 12, 15, 19] {
        match Zstd::new(level) {
            Ok(mut codec) => report.push_str(&row("zstd", i64::from(level), 0, &input, &mut codec)),
            Err(_) => report.push_str(&format!("| zstd | {level} | - | (init error) | - | - |\n")),
        }
    }

    report.push_str(
        "\nCalibrated tiers — fast: gzip 4 / br 4 w18 / zstd 3; \
         balanced: gzip 6 / br 5 w22 / zstd 6; max: gzip 9 / br 11 w24 / zstd 19.\n",
    );
    println!("{report}"); // style:allow-stdio-log
}

/// Reads the corpus from `argv[1]`, or falls back to the embedded sample.
fn load() -> Vec<u8> {
    match std::env::args().nth(1) {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("cannot read {path}: {err}; using embedded sample"); // style:allow-stdio-log
                SAMPLE.repeat(256).into_bytes()
            }
        },
        None => {
            eprintln!("no corpus given; using embedded synthetic sample"); // style:allow-stdio-log
            SAMPLE.repeat(256).into_bytes()
        }
    }
}

/// Benchmarks one codec configuration and formats a Markdown table row.
fn row(
    label: &str,
    level: i64,
    window: u32,
    input: &[u8],
    codec: &mut dyn StreamingCodec,
) -> String {
    let Some((out, mbps)) = bench(codec, input) else {
        return format!("| {label} | {level} | - | (encode error) | - | - |\n");
    };
    // style:allow-as-cast (reporting accepts approximate integer-to-f64 conversion)
    let ratio = 100.0 * out as f64 / input.len() as f64;
    let win = if window == 0 {
        "-".to_string()
    } else {
        window.to_string()
    };
    format!("| {label} | {level} | {win} | {ratio:.1} | {out} | {mbps:.0} |\n")
}

/// Encodes `input` repeatedly for ~0.3s; returns (compressed size, MB/s).
fn bench(codec: &mut dyn StreamingCodec, input: &[u8]) -> Option<(usize, f64)> {
    let out_len = encode(codec, input)?;
    let start = Instant::now();
    let mut iterations = 0u64;
    while start.elapsed().as_secs_f64() < 0.3 {
        codec.reset().ok()?;
        encode(codec, input)?;
        iterations += 1;
    }
    let secs = start.elapsed().as_secs_f64();
    // style:allow-as-cast (throughput reporting accepts approximate integer-to-f64 conversion)
    let bytes = input.len() as f64 * iterations as f64;
    Some((out_len, bytes / secs / 1.0e6))
}

/// Drives the streaming codec over the whole input and returns the output size.
fn encode(codec: &mut dyn StreamingCodec, input: &[u8]) -> Option<usize> {
    let mut produced = 0usize;
    let mut scratch = [0u8; 1 << 16];
    let mut offset = 0usize;
    loop {
        let remaining = &input[offset..];
        let operation = if remaining.is_empty() {
            Operation::Finish
        } else {
            Operation::Continue
        };
        let step = codec.step(operation, remaining, &mut scratch).ok()?;
        offset += step.consumed;
        produced += step.produced;
        if operation == Operation::Finish && step.state == StepState::Complete {
            return Some(produced);
        }
        if step.consumed == 0 && step.produced == 0 {
            return None; // no forward progress; avoid an infinite loop
        }
    }
}
