use super::UrlFacts;
use crate::patterns::Patterns;
use url::{Host, Origin, Url};

/// Fields are ANDed; alternatives within schemes, hosts, and paths are ORed.
/// Schemes, hosts, and paths accept globs. Unspecified fields impose no restriction.
#[derive(Debug, Default, Clone)]
pub struct UrlMatcher {
    schemes: Patterns,
    pub(crate) hosts: Patterns,
    paths: Patterns,
    pub(crate) origin: Option<Origin>,
    query_keys: Vec<String>,
}

impl UrlMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schemes(mut self, schemes: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for s in schemes {
            self.schemes.insert(s.as_ref().to_ascii_lowercase());
        }
        self
    }

    pub fn hosts(mut self, hosts: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for s in hosts {
            self.hosts.insert(s.as_ref().to_ascii_lowercase());
        }
        self
    }

    pub fn paths(mut self, paths: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for s in paths {
            self.paths.insert(s.as_ref().to_owned());
        }
        self
    }

    pub fn path_glob(mut self, pattern: impl Into<String>) -> Self {
        self.paths.insert(pattern.into());
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
        self.matches_facts(&UrlFacts::new(url))
    }

    pub(crate) fn matches_facts(&self, facts: &UrlFacts<'_>) -> bool {
        self.schemes.matches(facts.url.scheme())
            && (self.hosts.is_empty()
                || facts.host.as_deref().is_some_and(|h| self.hosts.matches(h)))
            && self.origin.as_ref().is_none_or(|origin| match origin {
                Origin::Tuple(scheme, host, port) if facts.url.scheme() != "blob" => {
                    scheme == facts.url.scheme()
                        && Some(*port) == facts.url.port_or_known_default()
                        && match (host, facts.url.host()) {
                            (Host::Domain(a), Some(Host::Domain(b))) => a == b,
                            (Host::Ipv4(a), Some(Host::Ipv4(b))) => *a == b,
                            (Host::Ipv6(a), Some(Host::Ipv6(b))) => *a == b,
                            _ => false,
                        }
                }
                Origin::Tuple(..) => origin == facts.origin(),
                _ => false,
            })
            && self.paths.matches(facts.url.path())
            && self.query_keys.iter().all(|key| facts.has_query(key))
    }
}
