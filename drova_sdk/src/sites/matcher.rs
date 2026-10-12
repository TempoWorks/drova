use glob_match::glob_match;
use url::{Origin, Url};

/// Fields are ANDed; alternatives within schemes, hosts, and paths are ORed.
/// Unspecified fields impose no restriction. Hosts and paths are exact unless
/// a path is explicitly registered with `path_glob`.
#[derive(Debug, Default, Clone)]
pub struct UrlMatcher {
    schemes: Vec<String>,
    hosts: Vec<String>,
    paths: Vec<String>,
    path_globs: Vec<String>,
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
        self.path_globs.push(pattern.into());
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
        (self.schemes.is_empty() || self.schemes.iter().any(|s| s == url.scheme()))
            && (self.hosts.is_empty()
                || url
                    .host_str()
                    .is_some_and(|h| self.hosts.iter().any(|s| s.eq_ignore_ascii_case(h))))
            && self
                .origin
                .as_ref()
                .is_none_or(|origin| origin.is_tuple() && *origin == url.origin())
            && ((self.paths.is_empty() && self.path_globs.is_empty())
                || self.paths.iter().any(|p| p == url.path())
                || self.path_globs.iter().any(|p| glob_match(p, url.path())))
            && self
                .query_keys
                .iter()
                .all(|key| url.query_pairs().any(|(k, _)| k == key.as_str()))
    }
}
