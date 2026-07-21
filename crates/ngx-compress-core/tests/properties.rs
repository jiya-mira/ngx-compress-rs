//! Property tests (L0): the `Accept-Encoding` parser must never panic and must
//! keep qualities in range; the progress validator must reject impossible byte
//! counts and accept any well-formed identity-shaped step.

use ngx_compress_core::{
    AcceptEncoding, ContentCoding, Operation, ProgressError, StepResult, StepState,
    validate_progress,
};
use proptest::prelude::*;

const CODINGS: [ContentCoding; 7] = [
    ContentCoding::Gzip,
    ContentCoding::Deflate,
    ContentCoding::Brotli,
    ContentCoding::Zstd,
    ContentCoding::DictionaryBrotli,
    ContentCoding::DictionaryZstd,
    ContentCoding::Identity,
];

fn q_str(quality: u32) -> String {
    if quality >= 1000 {
        "1.0".to_owned()
    } else {
        format!("0.{quality:03}")
    }
}

proptest! {
    /// Arbitrary input never panics, and every coding's quality stays in [0, 1000].
    #[test]
    fn parse_never_panics_and_quality_bounded(header in ".*") {
        let accepted = AcceptEncoding::parse(&header);
        for coding in CODINGS {
            // style:allow-for-in
            prop_assert!(accepted.quality(coding) <= 1000);
        }
    }

    /// A selected coding is always one that was offered and not excluded (q > 0).
    #[test]
    fn select_returns_only_acceptable(
        header in ".*",
        prefs in proptest::sample::subsequence(CODINGS.to_vec(), 0..=CODINGS.len()),
    ) {
        let accepted = AcceptEncoding::parse(&header);
        if let Some(chosen) = accepted.select(&prefs) {
            prop_assert!(accepted.quality(chosen) > 0);
            prop_assert!(prefs.contains(&chosen));
        }
    }

    /// Duplicate coding entries keep the highest quality.
    #[test]
    fn duplicate_keeps_highest(a in 0u32..=1000, b in 0u32..=1000) {
        let header = format!("br;q={}, br;q={}", q_str(a), q_str(b));
        let accepted = AcceptEncoding::parse(&header);
        prop_assert_eq!(u32::from(accepted.quality(ContentCoding::Brotli)), a.max(b));
    }

    /// Consuming more than the available input is always rejected.
    #[test]
    fn over_consume_rejected(input in 0usize..1000, output in 0usize..1000, extra in 1usize..100) {
        let result = StepResult {
            consumed: input + extra,
            produced: 0,
            state: StepState::Complete,
        };
        prop_assert_eq!(
            validate_progress(Operation::Continue, input, output, result),
            Err(ProgressError::ConsumedPastInput)
        );
    }

    /// Producing more than the output capacity is always rejected.
    #[test]
    fn over_produce_rejected(input in 0usize..1000, output in 0usize..1000, extra in 1usize..100) {
        let result = StepResult {
            consumed: 0,
            produced: output + extra,
            state: StepState::Complete,
        };
        prop_assert_eq!(
            validate_progress(Operation::Continue, input, output, result),
            Err(ProgressError::ProducedPastOutput)
        );
    }

    /// Any well-formed identity-shaped step satisfies the progress contract for
    /// every buffer size and operation.
    #[test]
    fn identity_shaped_step_is_valid(
        input in 0usize..1000,
        output in 0usize..1000,
        operation in prop_oneof![
            Just(Operation::Continue),
            Just(Operation::Flush),
            Just(Operation::Finish),
        ],
    ) {
        let moved = input.min(output);
        let drained = moved == input;
        let state = match operation {
            _ if !drained => StepState::NeedsOutput,
            Operation::Continue => StepState::NeedsInput,
            Operation::Flush | Operation::Finish => StepState::Complete,
        };
        let result = StepResult { consumed: moved, produced: moved, state };
        prop_assert!(validate_progress(operation, input, output, result).is_ok());
    }
}
