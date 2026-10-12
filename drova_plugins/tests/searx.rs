mod support;

use drova_plugins::{
    inputs::{SearxInput, SEARXNG_JSON},
    protocols::{json_url, search_url, SearxFallback, SearxProtocol},
    requester_plugins, searx_plugin,
    sites::SearxngSite,
};
use drova_sdk::{
    requester::{Error, InputHandler, RequesterBuilder},
    sites::UrlMatcher,
};
use support::{Reply, Server};
use url::Url;

const RESULTS: &str = r#"{"query":"Hello","results":[{"url":"https://rust-lang.org/","title":"Rust &amp; <b>safe</b>","content":"A <b>language</b> for everyone."}]}"#;
const HTML: &str = r#"<html><head><title>HTML fallback</title></head><body><article><h1>HTML fallback</h1><p>Search results available through this ordinary page with enough content for the existing generic HTML extraction algorithm.</p><p>Read <a href="/about">more about this instance</a> and its useful search results.</p></article></body></html>"#;
const DETECTED_HTML: &str = r#"<html><head><meta name="generator" content="searxng/2026.10"><title>Search</title></head><body><article><p>This is an ordinary HTML page containing enough content for fallback parsing when JSON results are unavailable.</p></article></body></html>"#;

#[test]
fn schemes_select_transport_and_preserve_search_paths() {
    for (raw, expected) in [
        (
            "searx://example.org?q=Hello",
            "https://example.org/search?q=Hello",
        ),
        (
            "searx+https://example.org?q=Hello",
            "https://example.org/search?q=Hello",
        ),
        (
            "searx+http://example.org:8080/base/?q=Hello",
            "http://example.org:8080/base/search?q=Hello",
        ),
        (
            "searx+http://[::1]:8080/search?q=Hello",
            "http://[::1]:8080/search?q=Hello",
        ),
        (
            "https://example.org/?q=Hello",
            "https://example.org/?q=Hello",
        ),
    ] {
        assert_eq!(
            search_url(&Url::parse(raw).unwrap()).unwrap().as_str(),
            expected
        );
    }
}

#[test]
fn json_url_replaces_all_formats_and_preserves_other_pairs() {
    let original = Url::parse("searx://example.org/base/search?q=Hello+%26+world&language=en&pageno=2&format=html&format=rss&engines=a&engines=b#results").unwrap();
    let rewritten = json_url(&original).unwrap();
    let pairs: Vec<_> = rewritten.query_pairs().collect();
    assert_eq!(pairs.iter().filter(|(key, _)| key == "format").count(), 1);
    assert!(pairs
        .iter()
        .any(|(key, value)| key == "format" && value == "json"));
    assert!(pairs
        .iter()
        .any(|(key, value)| key == "q" && value == "Hello & world"));
    assert!(pairs
        .iter()
        .any(|(key, value)| key == "pageno" && value == "2"));
    assert_eq!(pairs.iter().filter(|(key, _)| key == "engines").count(), 2);
    assert_eq!(rewritten.fragment(), None);
    assert_eq!(original.fragment(), Some("results"));
}

#[test]
fn adapter_urls_require_authority_query_and_supported_scheme() {
    assert!(matches!(
        search_url(&Url::parse("searx:example.org?q=x").unwrap()),
        Err(Error::InvalidUrl)
    ));
    assert!(matches!(
        search_url(&Url::parse("searx://example.org").unwrap()),
        Err(Error::BadRequest)
    ));
    assert!(matches!(
        search_url(&Url::parse("gopher://example.org/?q=x").unwrap()),
        Err(Error::UnsupportedProtocol)
    ));
}

#[test]
fn input_converts_text_bytes_and_empty_results() {
    let page = SearxInput.process_text(RESULTS.into(), None).unwrap();
    let bytes_page = SearxInput
        .process_bytes(RESULTS.as_bytes().to_vec(), None)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&page).unwrap(),
        serde_json::to_value(&bytes_page).unwrap()
    );
    let serialized = serde_json::to_string(&page).unwrap();
    assert!(serialized.contains("Rust & safe"));
    assert!(serialized.contains("A language for everyone."));
    assert!(serialized.contains("https://rust-lang.org/"));
    assert_eq!(page.title.as_deref(), Some("SearXNG: Hello"));
    let empty = SearxInput
        .process_text(r#"{"query":"empty","results":[]}"#.into(), None)
        .unwrap();
    assert!(empty.body.is_empty());
    assert!(SearxInput
        .process_text(r#"{"unrelated":true}"#.into(), None)
        .is_err());
    assert!(SearxInput.process_text("broken JSON".into(), None).is_err());
}

#[tokio::test]
async fn explicit_scheme_fetches_only_api_and_uses_private_mime() {
    let server = Server::start([Reply::json(RESULTS), Reply::json(RESULTS)]).await;
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .plugin(searx_plugin)
        .build();
    let url = server.searx_url("?q=Hello");
    let response = requester.process_request(&url).await.unwrap();
    assert_eq!(response.ty, SEARXNG_JSON);
    assert_eq!(
        requester.process(&url).await.unwrap().title.as_deref(),
        Some("SearXNG: Hello")
    );
    assert_eq!(
        server.requests(),
        ["/search?q=Hello&format=json", "/search?q=Hello&format=json"]
    );
}

#[tokio::test]
async fn configured_site_skips_original_fetch() {
    let server = Server::start([Reply::json(RESULTS)]).await;
    let site = SearxngSite::new();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .site(
            UrlMatcher::new()
                .origin(&server.base)
                .paths(["/search"])
                .require_query("q"),
            &site,
        )
        .build();
    let page = requester
        .process(
            server
                .base
                .join("search?q=Hello&language=en")
                .unwrap()
                .as_str(),
        )
        .await
        .unwrap();
    assert_eq!(page.title.as_deref(), Some("SearXNG: Hello"));
    assert_eq!(
        server.requests(),
        ["/search?q=Hello&language=en&format=json"]
    );
}

#[tokio::test]
async fn unavailable_api_falls_back_to_original_html() {
    let server = Server::start([Reply::error(403), Reply::html(HTML)]).await;
    let site = SearxngSite::new();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .site(UrlMatcher::new(), &site)
        .build();
    let page = requester
        .process(server.base.join("search?q=Hello").unwrap().as_str())
        .await
        .unwrap();
    assert_eq!(page.title.as_deref(), Some("HTML fallback"));
    assert_eq!(
        server.requests(),
        ["/search?q=Hello&format=json", "/search?q=Hello"]
    );
}

#[tokio::test]
async fn unsupported_mime_or_json_shape_declines_to_generic_parser() {
    for reply in [Reply::html(HTML), Reply::json(r#"{"other_api":true}"#)] {
        let server = Server::start([reply, Reply::html(HTML)]).await;
        let site = SearxngSite::new();
        let requester = RequesterBuilder::default()
            .plugin(requester_plugins)
            .site(UrlMatcher::new(), &site)
            .build();
        assert_eq!(
            requester
                .process(server.base.join("search?q=Hello").unwrap().as_str())
                .await
                .unwrap()
                .title
                .as_deref(),
            Some("HTML fallback")
        );
        assert_eq!(server.requests().len(), 2);
    }
}

#[tokio::test]
async fn rate_limits_and_malformed_json_do_not_trigger_fallback() {
    for (reply, malformed) in [
        (Reply::error(429), false),
        (Reply::json("broken JSON"), true),
        (Reply::json(r#"{"query":false,"results":[}"#), true),
    ] {
        let server = Server::start([reply]).await;
        let site = SearxngSite::new();
        let requester = RequesterBuilder::default()
            .plugin(requester_plugins)
            .site(UrlMatcher::new(), &site)
            .build();
        let error = requester
            .process(server.base.join("search?q=Hello").unwrap().as_str())
            .await
            .unwrap_err();
        assert!(if malformed {
            matches!(error, Error::ParserError(_))
        } else {
            matches!(error, Error::TooManyRequests)
        });
        assert_eq!(server.requests().len(), 1);
    }
}

#[tokio::test]
async fn explicit_schemes_are_strict_unless_html_fallback_enabled() {
    let server = Server::start([Reply::error(403)]).await;
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .plugin(searx_plugin)
        .build();
    assert!(matches!(
        requester.process(&server.searx_url("?q=Hello")).await,
        Err(Error::Forbidden)
    ));
    assert_eq!(server.requests().len(), 1);

    let server = Server::start([Reply::error(403), Reply::html(HTML)]).await;
    let protocol = SearxProtocol::new().fallback(SearxFallback::Html);
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .plugin(searx_plugin)
        .protocol("searx+http", &protocol)
        .build();
    let page = requester
        .process(&server.searx_url("?q=Hello"))
        .await
        .unwrap();
    assert_eq!(page.title.as_deref(), Some("HTML fallback"));
    // Relative links use real transport origin, not the adapter scheme.
    assert!(serde_json::to_string(&page)
        .unwrap()
        .contains(server.base.join("about").unwrap().as_str()));
    assert_eq!(
        server.requests(),
        ["/search?q=Hello&format=json", "/search?q=Hello&format=html"]
    );
}

#[tokio::test]
async fn optional_detection_is_cached_per_instance_base_path() {
    let server = Server::start([
        Reply::html(DETECTED_HTML),
        Reply::json(RESULTS),
        Reply::json(RESULTS),
        Reply::html(HTML),
    ])
    .await;
    let site = SearxngSite::detecting();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .site(
            UrlMatcher::new()
                .schemes(["http", "https"])
                .require_query("q"),
            &site,
        )
        .build();
    let first = server.base.join("searx/search?q=Hello").unwrap();
    assert_eq!(
        requester
            .process(first.as_str())
            .await
            .unwrap()
            .title
            .as_deref(),
        Some("SearXNG: Hello")
    );
    let next = server.base.join("searx/?q=Next").unwrap();
    requester.process(next.as_str()).await.unwrap();
    // Different application under the same host must be detected separately.
    requester
        .process(server.base.join("other/search?q=Hello").unwrap().as_str())
        .await
        .unwrap();
    assert_eq!(
        server.requests(),
        [
            "/searx/search?q=Hello",
            "/searx/search?q=Hello&format=json",
            "/searx/?q=Next&format=json",
            "/other/search?q=Hello",
        ]
    );
}

#[tokio::test]
async fn detection_fallback_reuses_already_fetched_html() {
    let server = Server::start([Reply::html(DETECTED_HTML), Reply::error(406)]).await;
    let site = SearxngSite::detecting();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .site(UrlMatcher::new(), &site)
        .build();
    requester
        .process(server.base.join("search?q=Hello").unwrap().as_str())
        .await
        .unwrap();
    assert_eq!(
        server.requests(),
        ["/search?q=Hello", "/search?q=Hello&format=json"]
    );
}
