use glob_match::glob_match;
use url::{Origin, Url};

/// Fields are ANDed; alternatives within schemes, hosts, and paths are ORed.
/// Schemes, hosts, and paths accept globs. Unspecified fields impose no restriction.
#[derive(Debug, Default, Clone)]
pub struct UrlMatcher {
    schemes: Vec<String>,
    hosts: Vec<String>,
    paths: Vec<String>,
    origin: Option<Origin>,
    query_keys: Vec<String>,
}

impl UrlMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schemes(mut self, schemes: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.schemes
            .extend(schemes.into_iter().map(|s| s.as_ref().to_ascii_lowercase()));
        self
    }

    pub fn hosts(mut self, hosts: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.hosts
            .extend(hosts.into_iter().map(|s| s.as_ref().to_ascii_lowercase()));
        self
    }

    pub fn paths(mut self, paths: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.paths
            .extend(paths.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    pub fn path_glob(mut self, pattern: impl Into<String>) -> Self {
        self.paths.push(pattern.into());
        self
    }

    /// Match scheme, host, and effective port. Opaque origins never match.
    pub fn origin(mut self, url: &Url) -> Self {
        self.origin = Some(url.origin());
        self
    }

    pub fn require_query(mut self, key: impl Into<String>) -> Self {
        self.query_keys.push(key.into());
        self
    }

    pub fn matches(&self, url: &Url) -> bool {
        matches_patterns(&self.schemes, url.scheme())
            && (self.hosts.is_empty()
                || url
                    .host_str()
                    .is_some_and(|h| matches_patterns(&self.hosts, &h.to_ascii_lowercase())))
            && self
                .origin
                .as_ref()
                .is_none_or(|origin| origin.is_tuple() && *origin == url.origin())
            && matches_patterns(&self.paths, url.path())
            && self
                .query_keys
                .iter()
                .all(|key| url.query_pairs().any(|(k, _)| k == key.as_str()))
    }
}

fn matches_patterns(patterns: &[String], value: &str) -> bool {
    patterns.is_empty()
        || patterns
            .iter()
            .any(|pattern| pattern == value || glob_match(pattern, value))
}
