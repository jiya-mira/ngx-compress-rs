use crate::{Operation, StepError, StepState, StreamingCodec, checked_step};

/// Boundary flag attached to an emitted output buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputBoundary {
    /// Ordinary compressed bytes.
    None,
    /// A completed upstream flush boundary.
    Flush,
    /// The completed end of the response stream.
    Finish,
}

/// Instruction returned to an output provider while its capacity is borrowed.
#[derive(Debug)]
pub struct OutputUse<T> {
    /// Value returned to the safe streaming driver.
    pub value: T,
    /// How the provider should submit or recycle the current output buffer.
    pub action: OutputAction,
}

/// Safe-core instruction for the current output buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputAction {
    /// Return an unused buffer to the provider.
    Recycle,
    /// Commit bytes and attach an optional stream boundary.
    Emit {
        /// Number of initialized bytes in the provided capacity.
        produced: usize,
        /// Boundary carried by this buffer.
        boundary: OutputBoundary,
    },
}

/// Supplies bounded writable regions and applies the resulting typed action.
///
/// Implementations own all server-specific allocation and submission details;
/// the streaming driver never sees their pointers or chain representation.
pub trait OutputProvider {
    /// Provider-specific allocation or submission error.
    type Error;

    /// Borrows the next output capacity, then atomically applies the action
    /// returned by `use_output` before releasing that borrow.
    ///
    /// # Errors
    ///
    /// Returns the provider error when output cannot be allocated or the typed
    /// action cannot be submitted.
    fn with_output<T>(
        &mut self,
        use_output: impl FnOnce(&mut [u8]) -> OutputUse<T>,
    ) -> Result<T, Self::Error>;
}

/// Fixed work allowance shared by all input buffers in one server callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkBudget {
    input_bytes: usize,
    codec_steps: usize,
}

impl WorkBudget {
    /// Creates a callback-local allowance.
    #[must_use]
    pub const fn new(input_bytes: usize, codec_steps: usize) -> Self {
        Self {
            input_bytes,
            codec_steps,
        }
    }

    /// Production callback allowance: at most 64 KiB and 32 codec calls.
    #[must_use]
    pub const fn per_callback() -> Self {
        Self::new(64 * 1024, 32)
    }

    fn can_step(self, has_input: bool) -> bool {
        self.codec_steps > 0 && (!has_input || self.input_bytes > 0)
    }

    fn input_limit(self, available: usize) -> usize {
        available.min(self.input_bytes)
    }

    fn record_step(&mut self, consumed: usize) {
        self.codec_steps -= 1;
        self.input_bytes -= consumed;
    }
}

/// Why the driver returned control to its caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveState {
    /// The codec accepted the complete supplied input and needs another buffer.
    NeedsInput,
    /// A flush operation completed.
    Flushed,
    /// A finish operation completed the response stream.
    Finished,
    /// The callback allowance was consumed; retry only the unconsumed suffix.
    BudgetExhausted,
}

/// Result of driving part or all of one upstream input buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DriveOutcome {
    /// Total input bytes consumed by the codec.
    pub consumed: usize,
    /// Reason control returned to the caller.
    pub state: DriveState,
}

/// Error from the codec/progress contract or the output provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DriveFailure<E> {
    /// Input accepted before the failure. Callers must never retry this prefix.
    pub consumed: usize,
    /// Underlying failure.
    pub error: DriveError<E>,
}

/// Kind of streaming driver failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveError<E> {
    /// Codec backend failure or invalid progress report.
    Step(StepError),
    /// Output allocation or submission failure.
    Output(E),
}

/// Drives a codec over one upstream buffer using a server-agnostic output sink.
///
/// All step/progress/boundary decisions happen here. The provider receives only
/// a complete [`OutputAction`] to apply to its current buffer.
///
/// # Errors
///
/// Returns [`DriveError::Step`] for codec/progress failures and
/// [`DriveError::Output`] when the provider cannot allocate or submit output.
pub fn drive_input<C, P>(
    codec: &mut C,
    operation: Operation,
    input: &[u8],
    output: &mut P,
    budget: &mut WorkBudget,
) -> Result<DriveOutcome, DriveFailure<P::Error>>
where
    C: StreamingCodec + ?Sized,
    P: OutputProvider,
{
    let mut offset = 0;
    loop {
        if !budget.can_step(offset < input.len()) {
            return Ok(DriveOutcome {
                consumed: offset,
                state: DriveState::BudgetExhausted,
            });
        }
        let limit = budget.input_limit(input.len() - offset);
        let end = offset + limit;
        // A flush/finish boundary may only be presented with the final input
        // suffix. A budget-truncated prefix is always an ordinary continuation.
        let step_operation = if end == input.len() {
            operation
        } else {
            Operation::Continue
        };
        let stepped = output
            .with_output(|capacity| {
                match checked_step(codec, step_operation, &input[offset..end], capacity) {
                    Ok(step) => {
                        let complete = step.state == StepState::Complete;
                        let boundary = if step_operation == Operation::Finish && complete {
                            OutputBoundary::Finish
                        } else if step_operation == Operation::Flush && complete {
                            OutputBoundary::Flush
                        } else {
                            OutputBoundary::None
                        };
                        let action = if step.produced > 0 || boundary != OutputBoundary::None {
                            OutputAction::Emit {
                                produced: step.produced,
                                boundary,
                            }
                        } else {
                            OutputAction::Recycle
                        };
                        OutputUse {
                            value: Ok((step, boundary)),
                            action,
                        }
                    }
                    Err(error) => OutputUse {
                        value: Err(error),
                        action: OutputAction::Recycle,
                    },
                }
            })
            .map_err(|error| DriveFailure {
                consumed: offset,
                error: DriveError::Output(error),
            })?;
        let (step, boundary) = stepped.map_err(|error| DriveFailure {
            consumed: offset,
            error: DriveError::Step(error),
        })?;
        offset += step.consumed;
        budget.record_step(step.consumed);

        if boundary == OutputBoundary::Finish {
            return Ok(DriveOutcome {
                consumed: offset,
                state: DriveState::Finished,
            });
        }
        if boundary == OutputBoundary::Flush {
            return Ok(DriveOutcome {
                consumed: offset,
                state: DriveState::Flushed,
            });
        }
        if step.state == StepState::NeedsInput && offset == input.len() {
            return Ok(DriveOutcome {
                consumed: offset,
                state: DriveState::NeedsInput,
            });
        }
    }
}

#[cfg(test)]
mod tests;
