//! `deflate` (zlib) and `gzip` codings, both backed by flate2. `deflate` uses
//! flate2's zlib framing directly; `gzip` streams raw deflate and adds the gzip
//! header and CRC32/ISIZE trailer itself, since flate2's low-level `Compress`
//! does not emit gzip framing.

use flate2::{Compress, Compression, FlushCompress, Status};
use ngx_compress_core::{
    CodecError, ContentCoding, Operation, StepResult, StepState, StreamingCodec,
};

const fn map_flush(operation: Operation) -> FlushCompress {
    match operation {
        Operation::Continue => FlushCompress::None,
        Operation::Flush => FlushCompress::Sync,
        Operation::Finish => FlushCompress::Finish,
    }
}

fn derive_state(
    operation: Operation,
    input_drained: bool,
    output_full: bool,
    ended: bool,
) -> StepState {
    if ended {
        return StepState::Complete;
    }
    match operation {
        Operation::Continue if input_drained => StepState::NeedsInput,
        Operation::Continue => StepState::NeedsOutput,
        _ if output_full => StepState::NeedsOutput,
        _ => StepState::Complete,
    }
}

fn delta(before: u64, after: u64) -> usize {
    usize::try_from(after.saturating_sub(before)).unwrap_or(usize::MAX)
}

struct FlateStep {
    consumed: usize,
    produced: usize,
    ended: bool,
}

/// Shared flate2 stream: the raw compress-a-slice step used by both codings.
struct FlateCore {
    compress: Compress,
}

impl FlateCore {
    fn new(level: u32, zlib_header: bool) -> Self {
        Self {
            compress: Compress::new(Compression::new(level), zlib_header),
        }
    }

    fn reset(&mut self, level: u32, zlib_header: bool) {
        self.compress = Compress::new(Compression::new(level), zlib_header);
    }

    fn run(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<FlateStep, CodecError> {
        let before_in = self.compress.total_in();
        let before_out = self.compress.total_out();
        let status = self
            .compress
            .compress(input, output, map_flush(operation))
            .map_err(|_| CodecError::Backend)?;
        Ok(FlateStep {
            consumed: delta(before_in, self.compress.total_in()),
            produced: delta(before_out, self.compress.total_out()),
            ended: status == Status::StreamEnd,
        })
    }
}

/// The `deflate` coding: a zlib stream (RFC 9110 treats `deflate` as zlib data).
#[cfg(feature = "deflate")]
pub struct Deflate {
    core: FlateCore,
    level: u32,
}

#[cfg(feature = "deflate")]
impl Deflate {
    /// Creates a deflate encoder at the given zlib level (1–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        Self {
            core: FlateCore::new(level, true),
            level,
        }
    }
}

#[cfg(feature = "deflate")]
impl StreamingCodec for Deflate {
    fn coding(&self) -> ContentCoding {
        ContentCoding::Deflate
    }

    fn step(
        &mut self,
        operation: Operation,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<StepResult, CodecError> {
        let step = self.core.run(operation, input, output)?;
        let state = derive_state(
            operation,
            step.consumed == input.len(),
            step.produced == output.len(),
            step.ended,
        );
        Ok(StepResult {
            consumed: step.consumed,
            produced: step.produced,
            state,
        })
    }

    fn reset(&mut self) {
        self.core.reset(self.level, true);
    }
}

#[cfg(feature = "gzip")]
const GZIP_HEADER: [u8; 10] = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff];

#[cfg(feature = "gzip")]
fn copy_into(src: &[u8], dst: &mut [u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

/// The `gzip` coding: raw deflate wrapped in the gzip header and CRC32/ISIZE
/// trailer. The header and trailer are emitted incrementally so a short output
/// buffer applies backpressure rather than losing framing bytes.
#[cfg(feature = "gzip")]
pub struct Gzip {
    core: FlateCore,
    crc: flate2::Crc,
    level: u32,
    header_pos: usize,
    body_done: bool,
    trailer: [u8; 8],
    trailer_pos: usize,
}

#[cfg(feature = "gzip")]
impl Gzip {
    /// Creates a gzip encoder at the given deflate level (1–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        Self {
            core: FlateCore::new(level, false),
            crc: flate2::Crc::new(),
            level,
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

#[cfg(feature = "gzip")]
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

    fn reset(&mut self) {
        self.core.reset(self.level, false);
        self.crc = flate2::Crc::new();
        self.header_pos = 0;
        self.body_done = false;
        self.trailer = [0; 8];
        self.trailer_pos = 0;
    }
}

#[cfg(feature = "gzip")]
fn trailer_state(trailer_pos: usize) -> StepState {
    if trailer_pos == 8 {
        StepState::Complete
    } else {
        StepState::NeedsOutput
    }
}
