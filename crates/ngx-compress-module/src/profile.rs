#![forbid(unsafe_code)]

//! Named presets for the `compress` directive.
//!
//! `compress fast|balanced|max` selects a [`Profile`]; its [`Preset`] supplies a
//! full turnkey configuration (which codecs to enable, their levels/window, and
//! `min_length`) so a single directive configures the module. Explicit
//! `compress_*` directives still override any preset field — the precedence
//! (explicit > profile > built-in default) is resolved in `conf`.
//!
//! `compress on` maps to [`Profile::Custom`], which contributes nothing: only
//! explicit directives and built-in defaults apply, preserving the original
//! behavior for configs written before profiles existed.

/// A preset bundle applied when a named profile is selected. Enabling codecs is
/// still gated by the compiled-in Cargo features at selection time. style:allow-pub-crate
#[derive(Clone, Copy, Debug)]
pub(crate) struct Preset {
    pub gzip: bool,
    pub brotli: bool,
    pub zstd: bool,
    pub gzip_level: u32,
    pub deflate_level: u32,
    pub brotli_level: u32,
    pub brotli_window: u32,
    pub zstd_level: i32,
    pub min_length: usize,
}

/// Which named preset the `compress` directive selected. `Custom` is
/// `compress on` — no preset, honor explicit directives and built-in defaults
/// only. style:allow-pub-crate
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile {
    Custom,
    Fast,
    Balanced,
    Max,
}

impl Profile {
    /// Parses a `compress` argument that names a profile (not `on`/`off`).
    // style:allow-pub-crate
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("fast") {
            Some(Self::Fast)
        } else if value.eq_ignore_ascii_case("balanced") {
            Some(Self::Balanced)
        } else if value.eq_ignore_ascii_case("max") {
            Some(Self::Max)
        } else {
            None
        }
    }

    /// The tuning bundle for a named tier; `Custom` contributes nothing.
    ///
    /// Levels were calibrated with the `bench/` harness on a real web corpus
    /// (HTML, CSS, minified/unminified JS, JSON): `fast` sits just below brotli's
    /// q4→q5 speed cliff and at gzip's converged ratio; `balanced` pays the q5
    /// jump and a higher zstd level for ~1–2% more ratio while staying fast;
    /// `max` uses each codec's ceiling for offline/precompressible responses. The
    /// tier *ordering* (fast < balanced < max on CPU and ratio) is the stable
    /// contract, so re-tuning a tier's values is not a breaking change. `deflate`
    /// is intentionally left opt-in (never enabled by a preset) since clients
    /// almost never request raw `deflate`; its level is still supplied for
    /// configs that turn it on explicitly.
    // style:allow-pub-crate
    pub(crate) fn preset(self) -> Option<Preset> {
        let preset = match self {
            Self::Custom => return None,
            Self::Fast => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 4,
                deflate_level: 4,
                brotli_level: 4,
                brotli_window: 18,
                zstd_level: 3,
                min_length: 256,
            },
            Self::Balanced => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 6,
                deflate_level: 6,
                brotli_level: 5,
                brotli_window: 22,
                zstd_level: 6,
                min_length: 256,
            },
            Self::Max => Preset {
                gzip: true,
                brotli: true,
                zstd: true,
                gzip_level: 9,
                deflate_level: 9,
                brotli_level: 11,
                brotli_window: 24,
                zstd_level: 19,
                min_length: 128,
            },
        };
        Some(preset)
    }
}
