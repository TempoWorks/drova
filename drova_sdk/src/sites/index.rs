use std::{
    borrow::Cow,
    cell::OnceCell,
    collections::{HashMap, HashSet},
};

use smallvec::SmallVec;
use url::{Origin, Url};

use super::UrlMatcher;
use crate::patterns::has_glob;

pub(crate) struct UrlFacts<'u> {
    pub url: &'u Url,
    pub host: Option<Cow<'u, str>>,
    origin: OnceCell<Origin>,
    query_keys: OnceCell<HashSet<Cow<'u, str>>>,
}

impl<'u> UrlFacts<'u> {
    pub fn new(url: &'u Url) -> Self {
        let host = url.host_str().map(|host| {
            if host.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Owned(host.to_ascii_lowercase())
            } else {
                Cow::Borrowed(host)
            }
        });
        Self {
            url,
            host,
            origin: OnceCell::new(),
            query_keys: OnceCell::new(),
        }
    }

    pub fn origin(&self) -> &Origin {
        self.origin.get_or_init(|| self.url.origin())
    }

    pub fn has_query(&self, key: &str) -> bool {
        self.query_keys
            .get_or_init(|| self.url.query_pairs().map(|(k, _)| k).collect())
            .contains(key)
    }
}

/// Candidate lists retain registration order and are merged without duplicates.
#[derive(Default)]
pub(crate) struct UrlIndex {
    exact: HashMap<String, Vec<usize>>,
    suffix: HashMap<String, Vec<usize>>,
    general: Vec<usize>,
}

impl UrlIndex {
    pub fn build<'a>(matchers: impl IntoIterator<Item = &'a UrlMatcher>) -> Self {
        let mut index = Self::default();
        for (id, matcher) in matchers.into_iter().enumerate() {
            if let Some(origin) = &matcher.origin {
                if let Origin::Tuple(_, host, _) = origin {
                    index
                        .exact
                        .entry(host.to_string().to_ascii_lowercase())
                        .or_default()
                        .push(id);
                }
                continue;
            }
            if matcher.hosts.is_empty() {
                index.general.push(id);
                continue;
            }
            for host in &matcher.hosts.literals {
                index.exact.entry(host.clone()).or_default().push(id);
            }
            let mut general = false;
            for pattern in &matcher.hosts.globs {
                if let Some(suffix) = pattern
                    .strip_prefix('*')
                    .filter(|s| s.starts_with('.') && !has_glob(s))
                {
                    index.suffix.entry(suffix.to_owned()).or_default().push(id);
                } else {
                    general = true;
                }
            }
            if general {
                index.general.push(id);
            }
        }
        index
    }

    pub fn candidates<'a>(&'a self, facts: &UrlFacts<'_>) -> Candidates<'a> {
        let mut candidates = Candidates::default();
        candidates.push(&self.general);
        if let Some(host) = facts.host.as_deref() {
            if let Some(ids) = self.exact.get(host) {
                candidates.push(ids);
            }
            if !self.suffix.is_empty() {
                for (offset, _) in host.match_indices('.') {
                    if let Some(ids) = self.suffix.get(&host[offset..]) {
                        candidates.push(ids);
                    }
                }
            }
        }
        candidates
    }
}

#[derive(Default)]
pub(crate) struct Candidates<'a> {
    lists: SmallVec<[&'a [usize]; 4]>,
}

impl<'a> Candidates<'a> {
    fn push(&mut self, ids: &'a [usize]) {
        if !ids.is_empty() {
            self.lists.push(ids);
        }
    }
}

impl Iterator for Candidates<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        let id = self
            .lists
            .iter()
            .filter_map(|ids| ids.first().copied())
            .min()?;
        for ids in &mut self.lists {
            while ids.first() == Some(&id) {
                *ids = &ids[1..];
            }
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_matches_equal_linear_matches_in_registration_order() {
        let mut matchers = vec![
            UrlMatcher::new(),
            UrlMatcher::new().hosts(["*.example.org", "example.org"]),
            UrlMatcher::new().hosts(["a.*", "a.example.org"]),
            UrlMatcher::new().hosts(["!blocked.example.org"]),
            UrlMatcher::new().hosts(["[::1]"]),
            UrlMatcher::new()
                .hosts(["{a,b}.example.org"])
                .require_query("q"),
            UrlMatcher::new().hosts(["*.example.org", "*.org"]),
            UrlMatcher::new().origin(&Url::parse("https://example.org:443/").unwrap()),
            UrlMatcher::new().origin(&Url::parse("https://[::1]/").unwrap()),
            UrlMatcher::new().origin(&Url::parse("searx://example.org").unwrap()),
        ];
        for i in 0..1000 {
            matchers.push(UrlMatcher::new().hosts([format!("host{i}.example.net")]));
            matchers.push(UrlMatcher::new().hosts([format!("*.host{i}.example.net")]));
        }
        let index = UrlIndex::build(&matchers);
        for raw in [
            "https://example.org/",
            "http://a.example.org/search?q=x",
            "https://a.b.example.org/",
            "https://blocked.example.org/",
            "https://unrelated.net/",
            "http://[::1]/",
            "https://[::1]/",
            "file:///search",
            "searx://A.EXAMPLE.ORG/search?q=x",
            "https://host999.example.net/",
            "https://sub.host999.example.net/",
            "https://b.example.org/?q=hello",
        ] {
            let url = Url::parse(raw).unwrap();
            let facts = UrlFacts::new(&url);
            let linear: Vec<_> = matchers
                .iter()
                .enumerate()
                .filter(|(_, m)| m.matches(&url))
                .map(|(id, _)| id)
                .collect();
            let indexed: Vec<_> = index
                .candidates(&facts)
                .filter(|&id| matchers[id].matches_facts(&facts))
                .collect();
            assert_eq!(indexed, linear, "{raw}");
        }
        let url = Url::parse("https://sub.host999.example.net/").unwrap();
        let facts = UrlFacts::new(&url);
        assert!(index.candidates(&facts).count() < 15);
    }

    #[test]
    #[ignore = "release timing benchmark"]
    fn matching_benchmark() {
        use std::{hint::black_box, time::Instant};
        for count in [1, 100, 10_000] {
            let mut matchers: Vec<_> = (0..count)
                .map(|id| UrlMatcher::new().hosts([format!("host{id}.example.org")]))
                .collect();
            matchers.push(UrlMatcher::new().hosts(["*.target.example.org"]));
            let index = UrlIndex::build(&matchers);
            let url = Url::parse("https://sub.target.example.org/search?q=x").unwrap();
            let iterations = 10_000;
            let start = Instant::now();
            for _ in 0..iterations {
                let found = matchers.iter().position(|m| m.matches(black_box(&url)));
                black_box(found);
            }
            let linear = start.elapsed();
            let start = Instant::now();
            for _ in 0..iterations {
                let facts = UrlFacts::new(black_box(&url));
                let found = index
                    .candidates(&facts)
                    .find(|&id| matchers[id].matches_facts(&facts));
                black_box(found);
            }
            let indexed = start.elapsed();
            println!("{count} sites, {iterations} lookups: linear {linear:?}, indexed {indexed:?}");
        }
    }
}
