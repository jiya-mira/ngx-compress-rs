use super::{
    DriveError, DriveFailure, DriveOutcome, DriveState, OutputAction, OutputBoundary,
    OutputProvider, OutputUse, WorkBudget, drive_input,
};
use crate::{CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec};

struct CopyCodec;

impl StreamingCodec for CopyCodec {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Identity
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let copied = input.len().min(output.len());
        output[..copied].copy_from_slice(&input[..copied]);
        let state = if copied < input.len() {
            StepState::NeedsOutput
        } else if operation == Operation::Continue {
            StepState::NeedsInput
        } else {
            StepState::Complete
        };
        Ok(StepResult {
            consumed: copied,
            produced: copied,
            state,
        })
    }

    fn reset(&mut self) -> Result<(), CodecError> {
        Ok(())
    }
}

struct RecordingOutput {
    capacity: usize,
    bytes: Vec<u8>,
    actions: Vec<OutputAction>,
    fail: bool,
}

impl OutputProvider for RecordingOutput {
    type Error = ();

    fn with_output<T>(
        &mut self,
        use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
    ) -> Result<T, Self::Error> {
        if self.fail {
            return Err(());
        }
        let mut capacity = vec![0_u8; self.capacity];
        let used = use_output(&mut capacity);
        if let OutputAction::Emit { produced, .. } = used.action {
            self.bytes.extend_from_slice(&capacity[..produced]);
        }
        self.actions.push(used.action);
        Ok(used.value)
    }
}

fn output(capacity: usize) -> RecordingOutput {
    RecordingOutput {
        capacity,
        bytes: Vec::new(),
        actions: Vec::new(),
        fail: false,
    }
}

#[test]
fn drives_multiple_output_buffers_without_server_pointers() {
    let mut codec = CopyCodec;
    let mut output = output(3);

    let result = drive_input(
        &mut codec,
        Operation::Continue,
        b"abcdef",
        &mut output,
        &mut WorkBudget::per_callback(),
    );

    assert_eq!(
        result,
        Ok(DriveOutcome {
            consumed: 6,
            state: DriveState::NeedsInput,
        })
    );
    assert_eq!(output.bytes, b"abcdef");
    assert_eq!(output.actions.len(), 2);
}

#[test]
fn emits_empty_finish_boundary() {
    let mut codec = CopyCodec;
    let mut output = output(8);

    let result = drive_input(
        &mut codec,
        Operation::Finish,
        b"",
        &mut output,
        &mut WorkBudget::per_callback(),
    );

    assert_eq!(
        result,
        Ok(DriveOutcome {
            consumed: 0,
            state: DriveState::Finished,
        })
    );
    assert_eq!(
        output.actions,
        [OutputAction::Emit {
            produced: 0,
            boundary: OutputBoundary::Finish,
        }]
    );
}

#[test]
fn reports_output_failure_without_ffi_state() {
    let mut codec = CopyCodec;
    let mut output = output(8);
    output.fail = true;

    let result = drive_input(
        &mut codec,
        Operation::Continue,
        b"input",
        &mut output,
        &mut WorkBudget::per_callback(),
    );

    assert_eq!(
        result,
        Err(DriveFailure {
            consumed: 0,
            error: DriveError::Output(()),
        })
    );
}

#[test]
fn resumes_unconsumed_suffix_after_input_budget_exhaustion() {
    let mut codec = CopyCodec;
    let mut output = output(8);
    let input = b"abcdefghijkl";
    let mut first_budget = WorkBudget::new(5, 32);

    let first = drive_input(
        &mut codec,
        Operation::Finish,
        input,
        &mut output,
        &mut first_budget,
    );
    assert_eq!(
        first,
        Ok(DriveOutcome {
            consumed: 5,
            state: DriveState::BudgetExhausted,
        })
    );

    let mut second_budget = WorkBudget::per_callback();
    let second = drive_input(
        &mut codec,
        Operation::Finish,
        &input[5..],
        &mut output,
        &mut second_budget,
    );
    assert_eq!(
        second,
        Ok(DriveOutcome {
            consumed: 7,
            state: DriveState::Finished,
        })
    );
    assert_eq!(output.bytes, input);
}

#[test]
fn step_budget_stops_before_reusing_consumed_input() {
    let mut codec = CopyCodec;
    let mut output = output(2);
    let mut budget = WorkBudget::new(usize::MAX, 2);

    let result = drive_input(
        &mut codec,
        Operation::Continue,
        b"abcdef",
        &mut output,
        &mut budget,
    );

    assert_eq!(
        result,
        Ok(DriveOutcome {
            consumed: 4,
            state: DriveState::BudgetExhausted,
        })
    );
    assert_eq!(output.bytes, b"abcd");
}

#[test]
fn failure_reports_input_consumed_before_output_exhaustion() {
    struct LimitedOutput {
        calls: usize,
        bytes: Vec<u8>,
    }

    impl OutputProvider for LimitedOutput {
        type Error = ();

        fn with_output<T>(
            &mut self,
            use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
        ) -> Result<T, Self::Error> {
            if self.calls == 1 {
                return Err(());
            }
            self.calls += 1;
            let mut capacity = [0_u8; 2];
            let used = use_output(&mut capacity);
            if let OutputAction::Emit { produced, .. } = used.action {
                self.bytes.extend_from_slice(&capacity[..produced]);
            }
            Ok(used.value)
        }
    }

    let mut codec = CopyCodec;
    let mut output = LimitedOutput {
        calls: 0,
        bytes: Vec::new(),
    };
    let result = drive_input(
        &mut codec,
        Operation::Continue,
        b"abcdef",
        &mut output,
        &mut WorkBudget::per_callback(),
    );

    assert_eq!(
        result,
        Err(DriveFailure {
            consumed: 2,
            error: DriveError::Output(()),
        })
    );
    assert_eq!(output.bytes, b"ab");
}
