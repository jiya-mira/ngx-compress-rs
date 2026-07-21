use crate::{AcceptEncoding, ContentCoding};

/// Precompressed-sidecar serving mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticMode {
    /// Never serve sidecars.
    Off,
    /// Serve a sidecar only when the client accepts that coding.
    On,
    /// Serve the first available sidecar regardless of Accept-Encoding.
    Always,
}

/// Owned request facts needed by static-sidecar policy.
pub struct StaticRequestFacts {
    /// Whether the request method is GET or HEAD.
    pub method_supported: bool,
    /// Owned raw URI bytes; no UTF-8 assumption is required.
    pub uri: Vec<u8>,
    /// Parsed client representation-coding preferences.
    pub accept_encoding: AcceptEncoding,
}

/// One sidecar representation to probe in server-priority order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticCandidate {
    /// Content-Encoding represented by the sidecar.
    pub coding: ContentCoding,
    /// Conventional filename extension for that coding.
    pub extension: &'static str,
}

const CANDIDATES: [StaticCandidate; 3] = [
    StaticCandidate {
        coding: ContentCoding::Zstd,
        extension: ".zst",
    },
    StaticCandidate {
        coding: ContentCoding::Brotli,
        extension: ".br",
    },
    StaticCandidate {
        coding: ContentCoding::Gzip,
        extension: ".gz",
    },
];

/// Returns the complete ordered list of sidecars that the FFI layer may probe.
#[must_use]
pub fn static_candidates(mode: StaticMode, facts: &StaticRequestFacts) -> Vec<StaticCandidate> {
    if mode == StaticMode::Off
        || !facts.method_supported
        || facts.uri.is_empty()
        || facts.uri.last() == Some(&b'/')
    {
        return Vec::new();
    }

    CANDIDATES
        .iter()
        .copied()
        .filter(|candidate| {
            mode == StaticMode::Always || facts.accept_encoding.quality(candidate.coding) > 0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{StaticMode, StaticRequestFacts, static_candidates};
    use crate::{AcceptEncoding, ContentCoding};

    fn facts(accept_encoding: AcceptEncoding) -> StaticRequestFacts {
        StaticRequestFacts {
            method_supported: true,
            uri: b"/asset.js".to_vec(),
            accept_encoding,
        }
    }

    #[test]
    fn on_filters_unacceptable_candidates() {
        let facts = facts(AcceptEncoding::parse("br, gzip;q=0"));
        let selected = static_candidates(StaticMode::On, &facts);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].coding, ContentCoding::Brotli);
    }

    #[test]
    fn always_keeps_server_priority_without_accept_header() {
        let selected = static_candidates(StaticMode::Always, &facts(AcceptEncoding::absent()));

        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].coding, ContentCoding::Zstd);
    }

    #[test]
    fn rejects_directory_and_unsupported_method() {
        let mut request = facts(AcceptEncoding::parse("gzip"));
        request.uri.push(b'/');
        assert!(static_candidates(StaticMode::On, &request).is_empty());

        request.uri.pop();
        request.method_supported = false;
        assert!(static_candidates(StaticMode::On, &request).is_empty());
    }
}
