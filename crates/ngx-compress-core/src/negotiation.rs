const QUALITY_MAX: u16 = 1_000;
const CODING_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ContentCoding {
    Gzip,
    Deflate,
    Brotli,
    Zstd,
    DictionaryBrotli,
    DictionaryZstd,
    Identity,
}

impl ContentCoding {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
            Self::Brotli => "br",
            Self::Zstd => "zstd",
            Self::DictionaryBrotli => "dcb",
            Self::DictionaryZstd => "dcz",
            Self::Identity => "identity",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("gzip") || value.eq_ignore_ascii_case("x-gzip") {
            Some(Self::Gzip)
        } else if value.eq_ignore_ascii_case("deflate") {
            Some(Self::Deflate)
        } else if value.eq_ignore_ascii_case("br") {
            Some(Self::Brotli)
        } else if value.eq_ignore_ascii_case("zstd") {
            Some(Self::Zstd)
        } else if value.eq_ignore_ascii_case("dcb") {
            Some(Self::DictionaryBrotli)
        } else if value.eq_ignore_ascii_case("dcz") {
            Some(Self::DictionaryZstd)
        } else if value.eq_ignore_ascii_case("identity") {
            Some(Self::Identity)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptEncoding {
    quality: [Option<u16>; CODING_COUNT],
    wildcard: Option<u16>,
    field_present: bool,
}

impl AcceptEncoding {
    #[must_use]
    pub const fn absent() -> Self {
        Self {
            quality: [None; CODING_COUNT],
            wildcard: None,
            field_present: false,
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Self {
        let mut parsed = Self {
            quality: [None; CODING_COUNT],
            wildcard: None,
            field_present: true,
        };

        // Parsing the external Accept-Encoding header (RFC 9110 list syntax). style:allow-delimited-split
        for item in value.split(',') {
            let mut parts = item.trim().split(';'); // style:allow-delimited-split
            let token = parts.next().unwrap_or_default().trim();
            if token.is_empty() {
                continue;
            }

            let mut quality = QUALITY_MAX;
            let mut valid = true;
            for parameter in parts {
                let Some((name, parameter_value)) = parameter.trim().split_once('=') else {
                    valid = false;
                    break;
                };
                if name.trim().eq_ignore_ascii_case("q") {
                    let Some(parsed_quality) = parse_quality(parameter_value.trim()) else {
                        valid = false;
                        break;
                    };
                    quality = parsed_quality;
                }
            }

            if !valid {
                continue;
            }

            if token == "*" {
                parsed.wildcard = Some(parsed.wildcard.map_or(quality, |old| old.max(quality)));
            } else if let Some(coding) = ContentCoding::parse(token) {
                let slot = &mut parsed.quality[coding.index()];
                *slot = Some(slot.map_or(quality, |old| old.max(quality)));
            }
        }

        parsed
    }

    #[must_use]
    pub fn quality(self, coding: ContentCoding) -> u16 {
        if let Some(quality) = self.quality[coding.index()] {
            return quality;
        }

        if coding == ContentCoding::Identity {
            if self.wildcard == Some(0) {
                return 0;
            }
            return QUALITY_MAX;
        }

        if !self.field_present {
            return 0;
        }

        self.wildcard.unwrap_or(0)
    }

    #[must_use]
    pub fn select(self, server_preference: &[ContentCoding]) -> Option<ContentCoding> {
        server_preference
            .iter()
            .copied()
            .fold(None, |selected: Option<(ContentCoding, u16)>, coding| {
                let quality = self.quality(coding);
                match selected {
                    Some((_, selected_quality)) if quality <= selected_quality => selected,
                    _ if quality > 0 => Some((coding, quality)),
                    _ => selected,
                }
            })
            .map(|(coding, _)| coding)
    }
}

fn parse_quality(value: &str) -> Option<u16> {
    let (whole, fractional) = value.split_once('.').unwrap_or((value, ""));
    if fractional.len() > 3 || !fractional.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    match whole {
        "0" => {
            let mut quality = 0_u16;
            let mut place = 100_u16;
            for digit in fractional.bytes() {
                quality += u16::from(digit - b'0') * place;
                place /= 10;
            }
            Some(quality)
        }
        "1" if fractional.bytes().all(|byte| byte == b'0') => Some(QUALITY_MAX),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptEncoding, ContentCoding};

    #[test]
    fn negotiates_common_browser_header_without_allocation() {
        let accepted = AcceptEncoding::parse("gzip, deflate, br, zstd");
        let selected = accepted.select(&[
            ContentCoding::Zstd,
            ContentCoding::Brotli,
            ContentCoding::Gzip,
            ContentCoding::Identity,
        ]);

        assert_eq!(selected, Some(ContentCoding::Zstd));
    }

    #[test]
    fn client_quality_overrides_server_order() {
        let accepted = AcceptEncoding::parse("gzip;q=0.5, br;q=1, zstd;q=0.8");
        let selected = accepted.select(&[
            ContentCoding::Zstd,
            ContentCoding::Brotli,
            ContentCoding::Gzip,
        ]);

        assert_eq!(selected, Some(ContentCoding::Brotli));
    }

    #[test]
    fn explicit_exclusion_overrides_wildcard() {
        let accepted = AcceptEncoding::parse("*;q=0.8, zstd;q=0");

        assert_eq!(accepted.quality(ContentCoding::Zstd), 0);
        assert_eq!(accepted.quality(ContentCoding::Brotli), 800);
    }

    #[test]
    fn absent_header_selects_identity_only() {
        let selected = AcceptEncoding::absent().select(&[
            ContentCoding::Zstd,
            ContentCoding::Gzip,
            ContentCoding::Identity,
        ]);

        assert_eq!(selected, Some(ContentCoding::Identity));
    }

    #[test]
    fn dictionary_coding_requires_server_eligibility() {
        let accepted = AcceptEncoding::parse("dcz, zstd, br");
        let without_dictionary = accepted.select(&[
            ContentCoding::Zstd,
            ContentCoding::Brotli,
            ContentCoding::Identity,
        ]);
        let with_dictionary = accepted.select(&[
            ContentCoding::DictionaryZstd,
            ContentCoding::Zstd,
            ContentCoding::Brotli,
        ]);

        assert_eq!(without_dictionary, Some(ContentCoding::Zstd));
        assert_eq!(with_dictionary, Some(ContentCoding::DictionaryZstd));
    }

    #[test]
    fn duplicate_coding_keeps_highest_quality() {
        let accepted = AcceptEncoding::parse("br;q=0.2, br;q=0.9");

        assert_eq!(accepted.quality(ContentCoding::Brotli), 900);
    }
}
