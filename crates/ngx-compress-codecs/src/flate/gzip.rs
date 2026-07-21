//! Incremental gzip framing around a raw flate2 stream.

use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};

use super::{FlateCore, derive_state};

const GZIP_HEADER: [u8; 10] = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff];

fn copy_into(src: &[u8], dst: &mut [u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

/// Raw deflate wrapped in the gzip header and CRC32/ISIZE trailer.
pub struct Gzip {
    core: FlateCore,
    crc: flate2::Crc,
    header_pos: usize,
    body_done: bool,
    trailer: [u8; 8],
    trailer_pos: usize,
}

impl Gzip {
    /// Creates a gzip encoder at the given deflate level (1–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        Self {
            core: FlateCore::new(level, false),
            crc: flate2::Crc::new(),
            header_pos: 0,
            body_done: false,
            trailer: [0; 8],
            trailer_pos: 0,
        }
    }

    fn compute_trailer(&self) -> [u8; 8] {
        let sum = self.crc.sum().to_le_bytes();
        let amount = self.crc.amount().to_le_bytes();
        [
            sum[0], sum[1], sum[2], sum[3], amount[0], amount[1], amount[2], amount[3],
        ]
    }
}

impl StreamingCodec for Gzip {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Gzip
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        // Accumulates bytes written across the header/body/trailer phases.
        let mut produced = 0; // style:allow-mut-stateful

        if self.header_pos < GZIP_HEADER.len() {
            let written = copy_into(&GZIP_HEADER[self.header_pos..], &mut output[produced..]);
            self.header_pos += written;
            produced += written;
            if self.header_pos < GZIP_HEADER.len() {
                return Ok(StepResult {
                    consumed: 0,
                    produced,
                    state: StepState::NeedsOutput,
                });
            }
        }

        if self.body_done {
            let written = copy_into(&self.trailer[self.trailer_pos..], &mut output[produced..]);
            self.trailer_pos += written;
            produced += written;
            let state = trailer_state(self.trailer_pos);
            return Ok(StepResult {
                consumed: 0,
                produced,
                state,
            });
        }

        let step = self.core.run(operation, input, &mut output[produced..])?;
        self.crc.update(&input[..step.consumed]);
        produced += step.produced;

        if operation == Operation::Finish && step.ended {
            self.body_done = true;
            self.trailer = self.compute_trailer();
            let written = copy_into(&self.trailer[self.trailer_pos..], &mut output[produced..]);
            self.trailer_pos += written;
            produced += written;
            return Ok(StepResult {
                consumed: step.consumed,
                produced,
                state: trailer_state(self.trailer_pos),
            });
        }

        let state = derive_state(
            operation,
            step.consumed == input.len(),
            produced == output.len(),
            false,
        );
        Ok(StepResult {
            consumed: step.consumed,
            produced,
            state,
        })
    }

    fn reset(&mut self) -> Result<(), CodecError> {
        self.core.reset();
        self.crc = flate2::Crc::new();
        self.header_pos = 0;
        self.body_done = false;
        self.trailer = [0; 8];
        self.trailer_pos = 0;
        Ok(())
    }
}

fn trailer_state(trailer_pos: usize) -> StepState {
    if trailer_pos == 8 {
        StepState::Complete
    } else {
        StepState::NeedsOutput
    }
}
