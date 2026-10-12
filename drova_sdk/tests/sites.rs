use std::sync::Mutex;

use async_trait::async_trait;
use dalet::types::Page;
use drova_sdk::{
    requester::{Error, InputHandler, ProtocolHandler, RequesterBuilder, Response, ResponseData},
    sites::{RequestContext, SiteHandler, SiteOutcome, UrlMatcher},
};
use url::Url;

fn page(title: String) -> Page {
    Page {
        title: Some(title),
        description: None,
        body: vec![],
        variables: None,
    }
}

#[derive(Default)]
struct Protocol(Mutex<Vec<String>>);

#[async_trait]
impl ProtocolHandler for Protocol {
    async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.0.lock().unwrap().push(url.to_string());
        Ok(Response {
            ty: "text/plain".into(),
            data: ResponseData::TextOutput(url.path().into()),
        })
    }
}

struct Input;

impl InputHandler for Input {
    fn process_text(&self, text: String, url: Option<&Url>) -> Result<Page, Error> {
        let mut page = page(text);
        page.description = url.map(ToString::to_string);
        Ok(page)
    }
    fn process_bytes(&self, _: Vec<u8>, _: Option<&Url>) -> Result<Page, Error> {
        Err(Error::UnsupportedInput)
    }
}

struct ApiSite;

#[async_trait]
impl SiteHandler for ApiSite {
    async fn process(&self, ctx: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error> {
        let response = ctx.fetch(&ctx.url().join("/api")?).await?;
        let ResponseData::TextOutput(text) = response.data else {
            unreachable!()
        };
        Ok(SiteOutcome::Parsed(page(text)))
    }
}

struct Probe;

#[async_trait]
impl SiteHandler for Probe {
    async fn process(&self, ctx: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error> {
        assert_eq!(ctx.original_response().await?.ty, "text/plain");
        assert_eq!(ctx.original_response().await?.ty, "text/plain");
        Ok(SiteOutcome::Fallback)
    }
}

struct Reject;

#[async_trait]
impl SiteHandler for Reject {
    async fn process(&self, _: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error> {
        Err(Error::TooManyRequests)
    }
}

#[tokio::test]
async fn api_adapter_skips_original_and_mime_parser() {
    let protocol = Protocol::default();
    let requester = RequesterBuilder::default()
        .protocol("https", &protocol)
        .site(
            UrlMatcher::new()
                .schemes(["http*"])
                .hosts(["example.*"])
                .paths(["/search*"]),
            &ApiSite,
        )
        .build();
    let result = requester
        .process("https://example.org/search?q=rust")
        .await
        .unwrap();
    assert_eq!(result.title.as_deref(), Some("/api"));
    assert_eq!(*protocol.0.lock().unwrap(), ["https://example.org/api"]);
}

#[tokio::test]
async fn original_is_shared_between_declining_handlers_and_mime_parser() {
    let protocol = Protocol::default();
    let requester = RequesterBuilder::default()
        .protocol("https", &protocol)
        .input("text/*", &Input)
        .site(UrlMatcher::new(), &Probe)
        .site(UrlMatcher::new(), &Probe)
        .build();
    let result = requester
        .process("https://example.org/original")
        .await
        .unwrap();
    assert_eq!(result.title.as_deref(), Some("/original"));
    assert_eq!(
        result.description.as_deref(),
        Some("https://example.org/original")
    );
    assert_eq!(protocol.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn errors_stop_dispatch_without_fetching_original() {
    let protocol = Protocol::default();
    let requester = RequesterBuilder::default()
        .protocol("https", &protocol)
        .site(UrlMatcher::new(), &Reject)
        .site(UrlMatcher::new(), &ApiSite)
        .build();
    assert!(matches!(
        requester.process("https://example.org/").await,
        Err(Error::TooManyRequests)
    ));
    assert!(protocol.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unmatched_sites_and_raw_requests_preserve_normal_flow() {
    let protocol = Protocol::default();
    let requester = RequesterBuilder::default()
        .protocol("https", &protocol)
        .input("text/plain", &Input)
        .site(UrlMatcher::new().hosts(["other.org"]), &Reject)
        .build();
    assert_eq!(
        requester
            .process("https://example.org/normal")
            .await
            .unwrap()
            .title
            .as_deref(),
        Some("/normal")
    );
    requester
        .process_request("https://other.org/raw")
        .await
        .unwrap();
    assert_eq!(
        *protocol.0.lock().unwrap(),
        ["https://example.org/normal", "https://other.org/raw"]
    );
}

#[test]
fn matcher_combines_fields_and_rejects_domain_lookalikes() {
    let matcher = UrlMatcher::new()
        .schemes(["HTTPS"])
        .hosts(["EXAMPLE.ORG", "search.example.org"])
        .paths(["/search", "/"])
        .require_query("q");
    for raw in [
        "https://example.org/search?q=a",
        "https://search.example.org/?q=",
    ] {
        assert!(matcher.matches(&Url::parse(raw).unwrap()));
    }
    for raw in [
        "http://example.org/search?q=a",
        "https://example.org.evil/search?q=a",
        "https://example.org/search",
        "https://example.org/other?q=a",
    ] {
        assert!(!matcher.matches(&Url::parse(raw).unwrap()));
    }
}

#[test]
fn matcher_handles_path_globs_and_effective_origin_ports() {
    let origin = Url::parse("https://example.org:443/base/").unwrap();
    let matcher = UrlMatcher::new().origin(&origin).path_glob("/*/*/issues/*");
    assert!(matcher.matches(&Url::parse("https://example.org/org/repo/issues/42").unwrap()));
    assert!(!matcher.matches(&Url::parse("https://example.org:444/org/repo/issues/42").unwrap()));
    assert!(!matcher.matches(&Url::parse("http://example.org/org/repo/issues/42").unwrap()));
    let opaque = Url::parse("searx://example.org").unwrap();
    assert!(!UrlMatcher::new().origin(&opaque).matches(&opaque));
}

#[test]
fn matcher_accepts_globs_in_schemes_hosts_and_paths() {
    let matcher = UrlMatcher::new()
        .schemes(["HTTP*", "searx+*"])
        .hosts(["*.EXAMPLE.ORG", "example.net"])
        .paths(["/articles/*", "/search"])
        .require_query("q");
    for raw in [
        "http://search.example.org/search?q=a",
        "https://search.example.org/articles/42?q=a",
        "https://example.net/search?q=a",
        "searx+http://SEARCH.EXAMPLE.ORG/search?q=a",
    ] {
        assert!(matcher.matches(&Url::parse(raw).unwrap()), "{raw}");
    }
    for raw in [
        "ftp://search.example.org/search?q=a",
        "https://example.org/search?q=a",
        "https://search.example.org.evil/search?q=a",
        "https://search.example.org/Articles/42?q=a",
        "https://search.example.org/search",
        "file:///articles/42?q=a",
    ] {
        assert!(!matcher.matches(&Url::parse(raw).unwrap()), "{raw}");
    }
}

#[test]
fn path_glob_is_an_alternative_to_paths_and_cannot_override_origin() {
    let origin = Url::parse("https://example.org/").unwrap();
    let matcher = UrlMatcher::new()
        .origin(&origin)
        .schemes(["*"])
        .hosts(["*"])
        .paths(["/articles/*"])
        .path_glob("/search*");
    for raw in [
        "https://example.org/articles/42",
        "https://example.org/search",
    ] {
        assert!(matcher.matches(&Url::parse(raw).unwrap()));
    }
    assert!(!matcher.matches(&Url::parse("https://other.org/search").unwrap()));
    assert!(!matcher.matches(&Url::parse("http://example.org/search").unwrap()));
    assert!(UrlMatcher::new().matches(&Url::parse("https://other.org/anything").unwrap()));
}

#[test]
fn exact_ipv6_hosts_and_paths_with_glob_syntax_still_match() {
    let matcher = UrlMatcher::new().hosts(["[::1]"]).paths(["/article[1]"]);
    assert!(matcher.matches(&Url::parse("http://[::1]/article[1]").unwrap()));
    assert!(!matcher.matches(&Url::parse("http://[::2]/article[1]").unwrap()));
}

#[tokio::test]
async fn registrar_can_capture_borrowed_handlers() {
    let protocol = Protocol::default();
    let requester = RequesterBuilder::default()
        .plugin(|builder| builder.protocol("https", &protocol))
        .build();
    requester
        .process_request("https://example.org/")
        .await
        .unwrap();
    assert_eq!(protocol.0.lock().unwrap().len(), 1);
}
