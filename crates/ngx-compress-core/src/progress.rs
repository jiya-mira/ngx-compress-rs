#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Continue,
    Flush,
    Finish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepState {
    NeedsInput,
    NeedsOutput,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepResult {
    pub consumed: usize,
    pub produced: usize,
    pub state: StepState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressError {
    ConsumedPastInput,
    ProducedPastOutput,
    StalledWithAvailableInputAndOutput,
    RequestedInputBeforeConsumingAvailableInput,
    RequestedOutputWithCapacityRemaining,
}

/// Verifies that one streaming encoder step respected its buffer contract.
///
/// # Errors
///
/// Returns [`ProgressError`] when the codec reports impossible byte counts,
/// requests input or output prematurely, or can spin without making progress.
pub fn validate_progress(
    operation: Operation,
    input_available: usize,
    output_capacity: usize,
    result: StepResult,
) -> Result<(), ProgressError> {
    if result.consumed > input_available {
        return Err(ProgressError::ConsumedPastInput);
    }
    if result.produced > output_capacity {
        return Err(ProgressError::ProducedPastOutput);
    }
    if result.state == StepState::NeedsInput && result.consumed < input_available {
        return Err(ProgressError::RequestedInputBeforeConsumingAvailableInput);
    }
    if result.state == StepState::NeedsOutput && result.produced < output_capacity {
        return Err(ProgressError::RequestedOutputWithCapacityRemaining);
    }

    let made_progress = result.consumed > 0 || result.produced > 0;
    let completed_boundary =
        operation != Operation::Continue && result.state == StepState::Complete;
    let legitimately_waiting = (input_available == 0 && result.state == StepState::NeedsInput)
        || (output_capacity == 0 && result.state == StepState::NeedsOutput);

    if !made_progress && !completed_boundary && !legitimately_waiting {
        return Err(ProgressError::StalledWithAvailableInputAndOutput);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Operation, ProgressError, StepResult, StepState, validate_progress};

    #[test]
    fn rejects_continue_loop_without_progress() {
        let result = validate_progress(
            Operation::Continue,
            1,
            4_096,
            StepResult {
                consumed: 0,
                produced: 0,
                state: StepState::Complete,
            },
        );

        assert_eq!(
            result,
            Err(ProgressError::StalledWithAvailableInputAndOutput)
        );
    }

    #[test]
    fn accepts_empty_completed_flush() {
        let result = validate_progress(
            Operation::Flush,
            0,
            4_096,
            StepResult {
                consumed: 0,
                produced: 0,
                state: StepState::Complete,
            },
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn accepts_waiting_for_input_when_none_is_available() {
        let result = validate_progress(
            Operation::Continue,
            0,
            4_096,
            StepResult {
                consumed: 0,
                produced: 0,
                state: StepState::NeedsInput,
            },
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn rejects_false_output_backpressure() {
        let result = validate_progress(
            Operation::Continue,
            16,
            4_096,
            StepResult {
                consumed: 0,
                produced: 0,
                state: StepState::NeedsOutput,
            },
        );

        assert_eq!(
            result,
            Err(ProgressError::RequestedOutputWithCapacityRemaining)
        );
    }
}
