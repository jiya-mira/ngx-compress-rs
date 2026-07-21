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

/// Result of driving one complete upstream input buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DriveOutcome {
    /// Total input bytes consumed by the codec.
    pub consumed: usize,
    /// Whether a finish operation completed the response stream.
    pub finished: bool,
}

/// Error from the codec/progress contract or the output provider.
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
) -> Result<DriveOutcome, DriveError<P::Error>>
where
    C: StreamingCodec + ?Sized,
    P: OutputProvider,
{
    let mut offset = 0;
    loop {
        let stepped = output
            .with_output(|capacity| {
                match checked_step(codec, operation, &input[offset..], capacity) {
                    Ok(step) => {
                        let complete = step.state == StepState::Complete;
                        let boundary = if operation == Operation::Finish && complete {
                            OutputBoundary::Finish
                        } else if operation == Operation::Flush && complete {
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
            .map_err(DriveError::Output)?;
        let (step, boundary) = stepped.map_err(DriveError::Step)?;
        offset += step.consumed;

        if boundary == OutputBoundary::Finish {
            return Ok(DriveOutcome {
                consumed: offset,
                finished: true,
            });
        }
        if boundary == OutputBoundary::Flush || step.state == StepState::NeedsInput {
            return Ok(DriveOutcome {
                consumed: offset,
                finished: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DriveError, DriveOutcome, OutputAction, OutputBoundary, OutputProvider, OutputUse,
        drive_input,
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

        let result = drive_input(&mut codec, Operation::Continue, b"abcdef", &mut output);

        assert_eq!(
            result,
            Ok(DriveOutcome {
                consumed: 6,
                finished: false,
            })
        );
        assert_eq!(output.bytes, b"abcdef");
        assert_eq!(output.actions.len(), 2);
    }

    #[test]
    fn emits_empty_finish_boundary() {
        let mut codec = CopyCodec;
        let mut output = output(8);

        let result = drive_input(&mut codec, Operation::Finish, b"", &mut output);

        assert_eq!(
            result,
            Ok(DriveOutcome {
                consumed: 0,
                finished: true,
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

        let result = drive_input(&mut codec, Operation::Continue, b"input", &mut output);

        assert_eq!(result, Err(DriveError::Output(())));
    }
}
