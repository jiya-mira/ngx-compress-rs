//! Test-only, once-per-worker failure injection.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Point {
    CodecInitialization,
    CodecReset,
    HeaderAllocation,
    OutputAllocation,
    Downstream,
}

#[cfg(feature = "fault-injection")]
pub fn take(point: Point) -> bool {
    use std::sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    };

    static SELECTED: LazyLock<Option<Point>> = LazyLock::new(|| {
        std::env::var("NGX_COMPRESS_FAULT")
            .ok()
            .and_then(|value| match value.as_str() {
                "codec_initialization" => Some(Point::CodecInitialization),
                "codec_reset" => Some(Point::CodecReset),
                "header_allocation" => Some(Point::HeaderAllocation),
                "output_allocation" => Some(Point::OutputAllocation),
                "downstream" => Some(Point::Downstream),
                _ => None,
            })
    });
    static TAKEN: AtomicBool = AtomicBool::new(false);

    *SELECTED == Some(point) && !TAKEN.swap(true, Ordering::Relaxed)
}

#[cfg(not(feature = "fault-injection"))]
pub const fn take(_point: Point) -> bool {
    false
}
