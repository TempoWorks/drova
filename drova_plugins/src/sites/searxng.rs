use async_trait::async_trait;
use drova_sdk::{
    requester::{Error, InputHandler, Response, ResponseData},
    sites::{RequestContext, SiteHandler, SiteOutcome},
};

use crate::{
    inputs::{prepare_response, SearxInput},
    protocols::{api_unavailable, json_url},
};

/// Known-instance mode goes directly to JSON. Detection mode fetches HTML only
/// for an instance not yet identified by its generator metadata.
pub struct SearxngSite {
    detect: bool,
    cache: DetectionCache,
}

impl Default for SearxngSite {
    fn default() -> Self {
        Self::new()
    }
}

impl SearxngSite {
    pub const fn new() -> Self {
        Self {
            detect: false,
            cache: DetectionCache::new(),
        }
    }

    pub const fn detecting() -> Self {
        Self {
            detect: true,
            cache: DetectionCache::new(),
        }
    }
}

#[async_trait]
impl SiteHandler for SearxngSite {
    async fn process(&self, ctx: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error> {
        let url = ctx.url().clone();
        if !matches!(url.scheme(), "http" | "https") || !url.query_pairs().any(|(k, _)| k == "q") {
            return Ok(SiteOutcome::Fallback);
        }
        if self.detect && !self.cache.contains(&url) {
            if !(url.path().ends_with('/') || url.path().ends_with("/search")) {
                return Ok(SiteOutcome::Fallback);
            }
            if !self.cache.detect(&url, ctx.original_response().await?) {
                return Ok(SiteOutcome::Fallback);
            }
        }
        let response = match ctx.fetch(&json_url(&url)?).await {
            Ok(response) => response,
            Err(error) if api_unavailable(&error) => return Ok(SiteOutcome::Fallback),
            Err(error) => return Err(error),
        };
        let Some(response) = prepare_response(response)? else {
            return Ok(SiteOutcome::Fallback);
        };
        let page = match response.data {
            ResponseData::TextOutput(text) => SearxInput.process_text(text, Some(&url))?,
            ResponseData::BitsOutput(bytes) => SearxInput.process_bytes(bytes, Some(&url))?,
        };
        Ok(SiteOutcome::Parsed(page))
    }
}

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use scraper::{Html, Selector};
use url::Url;

#[derive(Default)]
pub(crate) struct DetectionCache(OnceLock<Mutex<HashSet<String>>>);

impl DetectionCache {
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    fn key(url: &Url) -> String {
        let base = url.path().strip_suffix("search").unwrap_or(url.path());
        format!("{}{}", url.origin().ascii_serialization(), base)
    }

    pub fn contains(&self, url: &Url) -> bool {
        self.0.get().is_some_and(|cache| {
            cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(&Self::key(url))
        })
    }

    pub fn detect(&self, url: &Url, response: &Response) -> bool {
        if response.ty != "text/html" {
            return false;
        }
        let html = match &response.data {
            ResponseData::TextOutput(text) => text.clone(),
            ResponseData::BitsOutput(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        };
        let document = Html::parse_document(&html);
        let selector = Selector::parse("meta[name]").expect("static selector");
        let detected = document.select(&selector).any(|meta| {
            meta.value()
                .attr("name")
                .is_some_and(|name| name.eq_ignore_ascii_case("generator"))
                && meta.value().attr("content").is_some_and(|content| {
                    let content = content.trim().to_ascii_lowercase();
                    content == "searxng" || content.starts_with("searxng/")
                })
        });
        if detected {
            let mut cache = self
                .0
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            // Bound memory for long-lived requesters processing arbitrary origins.
            if cache.len() >= 256 {
                cache.clear();
            }
            cache.insert(Self::key(url));
        }
        detected
    }
}
