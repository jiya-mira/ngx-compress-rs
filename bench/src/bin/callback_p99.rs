//! Callback-budget calibration for the `fast` and `balanced` profiles.

use std::time::{Duration, Instant};

use ngx_compress_codecs::{Brotli, Gzip, Zstd};
use ngx_compress_core::{
    DriveState, Operation, OutputAction, OutputProvider, OutputUse, StreamingCodec, WorkBudget,
    drive_input,
};

const SAMPLE_LINE: &str =
    "The quick brown fox jumps over the lazy dog 0123456789 application/json text/html\n";
const DEFAULT_REQUESTS: usize = 200;

struct Scratch {
    bytes: [u8; 64 * 1024],
}

impl OutputProvider for Scratch {
    type Error = core::convert::Infallible;

    fn with_output<T>(
        &mut self,
        use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
    ) -> Result<T, Self::Error> {
        let used = use_output(&mut self.bytes);
        if let OutputAction::Emit { produced, .. } = used.action {
            assert!(produced <= self.bytes.len());
        }
        Ok(used.value)
    }
}

fn main() {
    let input = load_input();
    println!("profile\tcoding\tcallbacks\tp99_ms\tmax_ms\tstatus"); // style:allow-stdio-log
    run("fast", "gzip", Gzip::new(4), &input);
    run("fast", "br", Brotli::new(4, 18), &input);
    run_zstd("fast", 3, &input);
    run("balanced", "gzip", Gzip::new(6), &input);
    run("balanced", "br", Brotli::new(5, 22), &input);
    run_zstd("balanced", 6, &input);
}

fn load_input() -> Vec<u8> {
    std::env::args().nth(1).map_or_else(
        || SAMPLE_LINE.repeat(16_384).into_bytes(),
        |file| std::fs::read(file).unwrap_or_else(|error| panic!("cannot read corpus: {error}")),
    )
}

fn run_zstd(profile: &str, level: i32, input: &[u8]) {
    let codec = Zstd::new(level).unwrap_or_else(|error| panic!("zstd init failed: {error:?}"));
    run(profile, "zstd", codec, input);
}

fn run<C: StreamingCodec>(profile: &str, coding: &str, mut codec: C, input: &[u8]) {
    let mut samples = Vec::new();
    for request in 0..=DEFAULT_REQUESTS {
        // style:allow-for-in
        if request > 0 {
            codec
                .reset()
                .unwrap_or_else(|error| panic!("codec reset failed: {error:?}"));
        }
        encode_request(&mut codec, input, (request > 0).then_some(&mut samples));
    }
    samples.sort_unstable();
    let p99 = percentile(&samples, 99);
    let maximum = samples.last().copied().unwrap_or(Duration::ZERO);
    let status = if p99 <= Duration::from_millis(1) {
        "pass"
    } else {
        "fail"
    };
    println!(
        // style:allow-stdio-log
        "{profile}\t{coding}\t{}\t{:.3}\t{:.3}\t{status}",
        samples.len(),
        p99.as_secs_f64() * 1_000.0,
        maximum.as_secs_f64() * 1_000.0,
    );
}

fn encode_request(
    codec: &mut dyn StreamingCodec,
    input: &[u8],
    mut samples: Option<&mut Vec<Duration>>,
) {
    let mut offset = 0;
    loop {
        let mut output = Scratch {
            bytes: [0; 64 * 1024],
        };
        let mut budget = WorkBudget::per_callback();
        let started = Instant::now();
        let outcome = drive_input(
            codec,
            Operation::Finish,
            &input[offset..],
            &mut output,
            &mut budget,
        )
        .unwrap_or_else(|failure| panic!("callback failed after {} bytes", failure.consumed));
        if let Some(values) = &mut samples {
            values.push(started.elapsed());
        }
        offset += outcome.consumed;
        if outcome.state == DriveState::Finished {
            return;
        }
    }
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let rank = samples.len().saturating_mul(percentile).div_ceil(100);
    samples
        .get(rank.saturating_sub(1))
        .copied()
        .unwrap_or(Duration::ZERO)
}
