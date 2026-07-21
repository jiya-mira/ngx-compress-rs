use crate::{MimeTypes, compressible};

/// Rust-owned response facts prefetched at the server boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseFacts {
    /// Whether this is the main request rather than a subrequest.
    pub main_response: bool,
    /// Whether the response status is eligible for representation coding.
    pub successful: bool,
    /// Whether an upstream handler already selected a content encoding.
    pub already_encoded: bool,
    /// Known response length, or `None` for streaming/unknown-length responses.
    pub content_length: Option<usize>,
    /// Owned Content-Type value, including optional parameters.
    pub content_type: String,
}

/// Safe-core configuration needed to decide response eligibility.
#[derive(Clone, Copy, Debug)]
pub struct CompressionPolicy<'a> {
    /// Master enable switch.
    pub enabled: bool,
    /// Smallest known response length eligible for compression.
    pub min_length: usize,
    /// Optional configured MIME allow-list.
    pub types: Option<&'a MimeTypes>,
}

/// Applies compression eligibility policy to an owned response snapshot.
#[must_use]
pub fn eligible(policy: CompressionPolicy<'_>, facts: &ResponseFacts) -> bool {
    policy.enabled
        && facts.main_response
        && facts.successful
        && !facts.already_encoded
        && facts
            .content_length
            .is_none_or(|length| length >= policy.min_length)
        && compressible(&facts.content_type, policy.types)
}

#[cfg(test)]
mod tests {
    use super::{CompressionPolicy, ResponseFacts, eligible};

    fn policy() -> CompressionPolicy<'static> {
        CompressionPolicy {
            enabled: true,
            min_length: 20,
            types: None,
        }
    }

    fn facts() -> ResponseFacts {
        ResponseFacts {
            main_response: true,
            successful: true,
            already_encoded: false,
            content_length: Some(128),
            content_type: "text/plain".to_owned(),
        }
    }

    #[test]
    fn accepts_eligible_main_response() {
        assert!(eligible(policy(), &facts()));
    }

    #[test]
    fn accepts_unknown_streaming_length() {
        let mut facts = facts();
        facts.content_length = None;

        assert!(eligible(policy(), &facts));
    }

    #[test]
    fn rejects_each_ineligible_fact() {
        let mut response = facts();
        response.main_response = false;
        assert!(!eligible(policy(), &response));

        let mut response = facts();
        response.successful = false;
        assert!(!eligible(policy(), &response));

        let mut response = facts();
        response.already_encoded = true;
        assert!(!eligible(policy(), &response));

        let mut response = facts();
        response.content_length = Some(19);
        assert!(!eligible(policy(), &response));

        let mut response = facts();
        response.content_type = "image/png".to_owned();
        assert!(!eligible(policy(), &response));
    }
}
