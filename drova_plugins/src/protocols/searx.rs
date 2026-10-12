use async_trait::async_trait;
use drova_sdk::requester::{Error, ProtocolHandler, Response};
use drova_sdk::requests::{FetchContext, FetchFuture};
use url::Url;

use super::HttpProtocol;
use crate::inputs::{prepare_response, SEARXNG_HTML};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SearxFallback {
    /// Fail when an explicit scheme cannot obtain supported JSON.
    #[default]
    Error,
    /// Fetch HTML if JSON is unavailable. Other errors propagate.
    Html,
}

pub(crate) fn api_unavailable(error: &Error) -> bool {
    matches!(error, Error::Forbidden | Error::NotAcceptable)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SearxProtocol {
    fallback: SearxFallback,
}

impl SearxProtocol {
    pub const fn new() -> Self {
        Self {
            fallback: SearxFallback::Error,
        }
    }

    pub const fn fallback(mut self, fallback: SearxFallback) -> Self {
        self.fallback = fallback;
        self
    }

    async fn fetch_html(
        &self,
        url: &Url,
        ctx: Option<&FetchContext<'_, '_>>,
    ) -> Result<Response, Error> {
        let mut response = fetch_http(&html_url(url)?, ctx).await?;
        if response.ty == "text/html" {
            response.ty = SEARXNG_HTML.into();
        }
        Ok(response)
    }

    async fn fetch_inner(
        &self,
        url: &Url,
        ctx: Option<&FetchContext<'_, '_>>,
    ) -> Result<Response, Error> {
        match fetch_http(&json_url(url)?, ctx).await {
            Ok(response) => match prepare_response(response)? {
                Some(response) => Ok(response),
                None if self.fallback == SearxFallback::Html => self.fetch_html(url, ctx).await,
                None => Err(Error::UnsupportedInput),
            },
            Err(error) if api_unavailable(&error) && self.fallback == SearxFallback::Html => {
                self.fetch_html(url, ctx).await
            }
            Err(error) => Err(error),
        }
    }
}

#[async_trait]
impl ProtocolHandler for SearxProtocol {
    async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.fetch_inner(url, None).await
    }
    fn fetch_with_context<'s>(
        &'s self,
        url: &'s Url,
        ctx: &'s FetchContext<'_, '_>,
    ) -> FetchFuture<'s> {
        Box::pin(self.fetch_inner(url, Some(ctx)))
    }
}

async fn fetch_http(url: &Url, ctx: Option<&FetchContext<'_, '_>>) -> Result<Response, Error> {
    match ctx {
        Some(ctx) => ctx.fetch_or(url, &HttpProtocol).await,
        None => HttpProtocol.fetch(url).await,
    }
}

/// Resolve explicit adapter schemes into their HTTP transport. Ordinary HTTP
/// URLs retain their path. Custom root/directory paths default to `search`.
pub fn search_url(url: &Url) -> Result<Url, Error> {
    let transport = match url.scheme() {
        "searx" | "searx+https" => "https",
        "searx+http" => "http",
        "http" | "https" => return validate(url.clone()),
        _ => return Err(Error::UnsupportedProtocol),
    };

    // Url::set_scheme cannot switch a non-special scheme to HTTP(S).
    let (_, rest) = url.as_str().split_once(':').ok_or(Error::InvalidUrl)?;
    if !rest.starts_with("//") || url.host_str().is_none() {
        return Err(Error::InvalidUrl);
    }
    let mut target = Url::parse(&format!("{transport}:{rest}"))?;
    if target.path().ends_with('/') {
        target.set_path(&format!("{}search", target.path()));
    }
    validate(target)
}

fn validate(mut url: Url) -> Result<Url, Error> {
    if url.host_str().is_none() {
        return Err(Error::InvalidUrl);
    }
    if !url.query_pairs().any(|(key, _)| key == "q") {
        return Err(Error::BadRequest);
    }
    url.set_fragment(None);
    Ok(url)
}

fn format_url(url: &Url, format: &str) -> Result<Url, Error> {
    let mut target = search_url(url)?;
    let pairs: Vec<_> = target
        .query_pairs()
        .filter(|(key, _)| key != "format")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    target
        .query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair("format", format);
    Ok(target)
}

pub fn json_url(url: &Url) -> Result<Url, Error> {
    format_url(url, "json")
}

pub fn html_url(url: &Url) -> Result<Url, Error> {
    format_url(url, "html")
}
