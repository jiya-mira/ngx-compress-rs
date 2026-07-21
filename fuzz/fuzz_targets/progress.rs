#![no_main]

use libfuzzer_sys::fuzz_target;
use ngx_compress_core::{Operation, StepResult, StepState, validate_progress};

// validate_progress must never panic for any byte-count/state combination.
// Run with: cargo +nightly fuzz run progress
fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }
    let operation = match data[4] % 3 {
        0 => Operation::Continue,
        1 => Operation::Flush,
        _ => Operation::Finish,
    };
    let state = match data[5] % 3 {
        0 => StepState::NeedsInput,
        1 => StepState::NeedsOutput,
        _ => StepState::Complete,
    };
    let result = StepResult {
        consumed: usize::from(data[2]),
        produced: usize::from(data[3]),
        state,
    };
    validate_progress(operation, usize::from(data[0]), usize::from(data[1]), result);
});
