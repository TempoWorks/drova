use dalet::types::{Page, Tag};
use drova_sdk::requester::{Error, InputHandler, Response, ResponseData};
use scraper::Html;
use serde::Deserialize;
use url::Url;

use super::HtmlInput;
use crate::protocols::search_url;

/// Adapter MIME types; ordinary site JSON is not claimed by this parser.
pub const SEARXNG_JSON: &str = "application/x-searxng+json";
pub const SEARXNG_HTML: &str = "text/x-searxng-html";

pub struct SearxInput;

impl InputHandler for SearxInput {
    fn process_text(&self, text: String, _: Option<&Url>) -> Result<Page, Error> {
        parse_response(text.as_bytes())
    }

    fn process_bytes(&self, bytes: Vec<u8>, _: Option<&Url>) -> Result<Page, Error> {
        parse_response(&bytes)
    }
}

/// Reuse HTML conversion with the transport URL as the base for relative links.
pub struct SearxHtmlInput;

impl InputHandler for SearxHtmlInput {
    fn process_text(&self, text: String, url: Option<&Url>) -> Result<Page, Error> {
        let base = url.map(search_url).transpose()?;
        HtmlInput.process_text(text, base.as_ref())
    }

    fn process_bytes(&self, bytes: Vec<u8>, url: Option<&Url>) -> Result<Page, Error> {
        let base = url.map(search_url).transpose()?;
        HtmlInput.process_bytes(bytes, base.as_ref())
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    query: String,
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    url: String,
    title: Option<String>,
    content: Option<String>,
}

fn decode(bytes: &[u8]) -> Result<Option<SearchResponse>, Error> {
    match serde_json::from_slice(bytes) {
        Ok(search) => Ok(Some(search)),
        Err(error) if error.is_data() => {
            serde_json::from_slice::<serde::de::IgnoredAny>(bytes)
                .map_err(|error| Error::ParserError(error.to_string()))?;
            Ok(None)
        }
        Err(error) => Err(Error::ParserError(error.to_string())),
    }
}

fn bytes(response: &Response) -> &[u8] {
    match &response.data {
        ResponseData::TextOutput(text) => text.as_bytes(),
        ResponseData::BitsOutput(bytes) => bytes,
    }
}

pub(crate) fn prepare_response(mut response: Response) -> Result<Option<Response>, Error> {
    if response.ty != "application/json" && response.ty != SEARXNG_JSON {
        return Ok(None);
    }
    if decode(bytes(&response))?.is_none() {
        return Ok(None);
    }
    response.ty = SEARXNG_JSON.into();
    Ok(Some(response))
}

pub(crate) fn parse_response(bytes: &[u8]) -> Result<Page, Error> {
    let search =
        decode(bytes)?.ok_or_else(|| Error::ParserError("Unsupported SearXNG response".into()))?;
    Ok(convert_search(search))
}

pub(crate) fn parse_site_response(response: &Response) -> Result<Option<Page>, Error> {
    if response.ty != "application/json" && response.ty != SEARXNG_JSON {
        return Ok(None);
    }
    Ok(decode(bytes(response))?.map(convert_search))
}

fn convert_search(search: SearchResponse) -> Page {
    let mut body = Vec::with_capacity(search.results.len() * 2);
    for result in search.results {
        let title = result
            .title
            .map(plain_text)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| result.url.clone());
        body.push(Tag::Paragraph {
            body: vec![Tag::NavLink {
                body: Some(title.into()),
                dref: result.url,
            }]
            .into(),
        });
        if let Some(content) = result.content {
            let content = plain_text(content);
            if !content.is_empty() {
                body.push(Tag::Paragraph {
                    body: content.into(),
                });
            }
        }
    }
    Page {
        title: Some(format!("SearXNG: {}", search.query)),
        description: None,
        body,
        variables: None,
    }
}

fn plain_text(html: String) -> String {
    if !html.contains(['<', '&']) {
        return html;
    }
    Html::parse_fragment(&html)
        .root_element()
        .text()
        .collect::<String>()
}
