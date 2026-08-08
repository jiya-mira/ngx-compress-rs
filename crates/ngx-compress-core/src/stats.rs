//! Allocation-free accounting on the compression path and late formatting for
//! NGINX variables and the optional `Server-Timing` trailer.

use std::fmt::Write;
use std::time::Duration;

use crate::ContentCoding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsField {
    Coding,
    Level,
    InputBytes,
    OutputBytes,
    Ratio,
    TimeMs,
}

#[derive(Clone, Copy, Debug)]
pub struct CompressionStats {
    coding: ContentCoding,
    level: i32,
    input_bytes: u64,
    output_bytes: u64,
    codec_time: Duration,
}

impl CompressionStats {
    #[must_use]
    pub const fn new(coding: ContentCoding, level: i32) -> Self {
        Self {
            coding,
            level,
            input_bytes: 0,
            output_bytes: 0,
            codec_time: Duration::ZERO,
        }
    }

    pub fn record(&mut self, consumed: usize, produced: usize, elapsed: Duration) {
        self.input_bytes = self
            .input_bytes
            .saturating_add(u64::try_from(consumed).unwrap_or(u64::MAX));
        self.output_bytes = self
            .output_bytes
            .saturating_add(u64::try_from(produced).unwrap_or(u64::MAX));
        self.codec_time = self.codec_time.saturating_add(elapsed);
    }

    #[must_use]
    pub fn value(self, field: StatsField) -> Option<String> {
        match field {
            StatsField::Coding => Some(self.coding.as_str().to_owned()),
            StatsField::Level => Some(self.level.to_string()),
            StatsField::InputBytes => Some(self.input_bytes.to_string()),
            StatsField::OutputBytes => Some(self.output_bytes.to_string()),
            StatsField::Ratio => self.ratio(),
            StatsField::TimeMs => Some(format!("{:.3}", self.time_ms())),
        }
    }

    #[must_use]
    pub fn server_timing(self) -> String {
        let mut value = format!(
            "compress;dur={:.3};desc=\"{}\";level={};input={};output={}",
            self.time_ms(),
            self.coding.as_str(),
            self.level,
            self.input_bytes,
            self.output_bytes
        );
        if let Some(ratio) = self.ratio() {
            let _ = write!(value, ";ratio={ratio}");
        }
        value
    }

    fn ratio(self) -> Option<String> {
        if self.input_bytes == 0 {
            return None;
        }
        let input = u128::from(self.input_bytes);
        let output = u128::from(self.output_bytes);
        let scaled = (output.saturating_mul(1_000_000) + input / 2) / input;
        Some(format!("{}.{:06}", scaled / 1_000_000, scaled % 1_000_000))
    }

    fn time_ms(self) -> f64 {
        self.codec_time.as_secs_f64() * 1_000.0
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::ContentCoding;

    use super::{CompressionStats, StatsField};

    #[test]
    fn accumulates_and_formats_the_public_fields() {
        let mut stats = CompressionStats::new(ContentCoding::Zstd, 6);
        stats.record(400, 100, Duration::from_micros(125));
        stats.record(600, 150, Duration::from_micros(375));

        assert_eq!(stats.value(StatsField::Coding).as_deref(), Some("zstd"));
        assert_eq!(stats.value(StatsField::Level).as_deref(), Some("6"));
        assert_eq!(stats.value(StatsField::InputBytes).as_deref(), Some("1000"));
        assert_eq!(stats.value(StatsField::OutputBytes).as_deref(), Some("250"));
        assert_eq!(stats.value(StatsField::Ratio).as_deref(), Some("0.250000"));
        assert_eq!(stats.value(StatsField::TimeMs).as_deref(), Some("0.500"));
        assert_eq!(
            stats.server_timing(),
            "compress;dur=0.500;desc=\"zstd\";level=6;input=1000;output=250;ratio=0.250000"
        );
    }

    #[test]
    fn zero_input_has_no_ratio() {
        let stats = CompressionStats::new(ContentCoding::Gzip, 4);

        assert_eq!(stats.value(StatsField::Ratio), None);
        assert!(!stats.server_timing().contains("ratio="));
    }
}
