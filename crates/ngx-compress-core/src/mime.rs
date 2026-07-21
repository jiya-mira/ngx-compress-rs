/// Rust-owned MIME allow-list resolved during configuration loading.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MimeTypes {
    entries: Box<[Box<str>]>,
}

impl MimeTypes {
    /// Builds an immutable MIME allow-list from owned directive values.
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = String>) -> Self {
        Self {
            entries: values.into_iter().map(String::into_boxed_str).collect(),
        }
    }

    fn contains(&self, kind: &str) -> bool {
        self.entries
            .iter()
            .any(|mime| mime.as_ref() == "*" || mime.eq_ignore_ascii_case(kind))
    }
}

/// Returns whether a response content type is eligible for compression.
///
/// Parameters following the MIME type are ignored. An explicit allow-list
/// replaces the built-in defaults and matches ASCII case-insensitively.
#[must_use]
pub fn compressible(content_type: &str, types: Option<&MimeTypes>) -> bool {
    let kind = content_type
        .split_once(';')
        .map_or(content_type, |(kind, _)| kind)
        .trim();
    types.map_or_else(|| builtin_compressible(kind), |types| types.contains(kind))
}

fn builtin_compressible(kind: &str) -> bool {
    kind.starts_with("text/")
        || matches!(
            kind,
            "application/json"
                | "application/javascript"
                | "application/xml"
                | "application/rss+xml"
                | "application/atom+xml"
                | "application/wasm"
                | "image/svg+xml"
        )
}

#[cfg(test)]
mod tests {
    use super::{MimeTypes, compressible};

    #[test]
    fn builtins_ignore_parameters() {
        assert!(compressible("text/html; charset=utf-8", None));
        assert!(compressible("application/json", None));
        assert!(!compressible("image/png", None));
    }

    #[test]
    fn explicit_types_are_owned_and_case_insensitive() {
        let types = MimeTypes::new(["application/custom".to_owned()]);

        assert!(compressible("Application/Custom", Some(&types)));
        assert!(!compressible("text/plain", Some(&types)));
    }

    #[test]
    fn wildcard_accepts_every_type() {
        let types = MimeTypes::new(["*".to_owned()]);

        assert!(compressible("application/octet-stream", Some(&types)));
    }
}
