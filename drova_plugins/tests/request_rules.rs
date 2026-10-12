mod support;

use drova_plugins::{
    protocols::{HttpMethod, HttpProtocol, HttpRequest, SearxFallback, SearxProtocol},
    requester_plugins, searx_plugin,
    sites::SearxngSite,
};
use drova_sdk::{
    requester::{Error, RequesterBuilder},
    sites::UrlMatcher,
};
use support::{Reply, Server};

const RESULTS: &str = r#"{"query":"Hello","results":[]}"#;
const HTML: &str = r#"<html><head><meta name="generator" content="searxng/2026"></head><body><article><p>This page contains enough ordinary article content for the existing generic HTML parser to extract when the JSON API is unavailable.</p></article></body></html>"#;

fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request
        .split("\r\n\r\n")
        .next()
        .unwrap()
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

#[tokio::test]
async fn http_rules_change_method_headers_cookie_and_body_independently() {
    let server = Server::start([Reply::json(RESULTS), Reply::json(RESULTS)]).await;
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .request_rule::<HttpRequest>(
            UrlMatcher::new().origin(&server.base).paths(["/api"]),
            |ctx, request| {
                assert!(ctx.via::<HttpProtocol>());
                request
                    .method(HttpMethod::POST)
                    .header("User-Agent", "custom-client")
                    .header("Cookie", "session=abc")
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body("q=Hello")
            },
        )
        .build();
    requester
        .process_request(server.base.join("api").unwrap().as_str())
        .await
        .unwrap();
    requester
        .process_request(server.base.join("other").unwrap().as_str())
        .await
        .unwrap();
    let requests = server.raw_requests();
    assert!(requests[0].starts_with("POST /api HTTP/1.1"));
    assert_eq!(header(&requests[0], "cookie"), Some("session=abc"));
    assert_eq!(header(&requests[0], "user-agent"), Some("custom-client"));
    assert!(requests[0].ends_with("\r\n\r\nq=Hello"));
    assert!(requests[1].starts_with("GET /other HTTP/1.1"));
    assert_eq!(header(&requests[1], "cookie"), None);
}

#[tokio::test]
async fn plugin_can_register_rules_scoped_to_searx_protocol_and_site() {
    fn plugin(builder: RequesterBuilder<'_>) -> RequesterBuilder<'_> {
        builder.request_rule::<HttpRequest>(UrlMatcher::new(), |ctx, request| {
            if ctx.via::<SearxProtocol>() || ctx.via::<SearxngSite>() {
                request.header("Cookie", "searx_session=abc")
            } else {
                request
            }
        })
    }
    let server = Server::start([
        Reply::json(RESULTS),
        Reply::json(RESULTS),
        Reply::json(RESULTS),
    ])
    .await;
    let site = SearxngSite::new();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .plugin(searx_plugin)
        .plugin(plugin)
        .site(
            UrlMatcher::new()
                .origin(&server.base)
                .paths(["/search"])
                .require_query("q"),
            &site,
        )
        .build();
    requester
        .process(&server.searx_url("?q=Hello"))
        .await
        .unwrap();
    requester
        .process(server.base.join("search?q=Hello").unwrap().as_str())
        .await
        .unwrap();
    requester
        .process_request(server.base.join("ordinary").unwrap().as_str())
        .await
        .unwrap();
    let requests = server.raw_requests();
    assert_eq!(header(&requests[0], "cookie"), Some("searx_session=abc"));
    assert_eq!(header(&requests[1], "cookie"), Some("searx_session=abc"));
    assert_eq!(header(&requests[2], "cookie"), None);
    assert_eq!(
        server.requests(),
        [
            "/search?q=Hello&format=json",
            "/search?q=Hello&format=json",
            "/ordinary"
        ]
    );
}

#[tokio::test]
async fn detection_probe_and_generic_fallback_do_not_receive_site_rules() {
    let server = Server::start([Reply::html(HTML), Reply::error(403)]).await;
    let site = SearxngSite::detecting();
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .site(UrlMatcher::new(), &site)
        .request_rule::<HttpRequest>(UrlMatcher::new(), |ctx, request| {
            if ctx.via::<SearxngSite>() {
                request.header("Cookie", "site_cookie=abc")
            } else {
                request
            }
        })
        .build();
    requester
        .process(server.base.join("search?q=Hello").unwrap().as_str())
        .await
        .unwrap();
    let requests = server.raw_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(header(&requests[0], "cookie"), None);
    assert_eq!(header(&requests[1], "cookie"), Some("site_cookie=abc"));
}

#[tokio::test]
async fn explicit_html_fallback_preserves_searx_context() {
    let server = Server::start([Reply::error(406), Reply::html(HTML)]).await;
    let protocol = SearxProtocol::new().fallback(SearxFallback::Html);
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .plugin(searx_plugin)
        .protocol("searx+http", &protocol)
        .request_rule::<HttpRequest>(UrlMatcher::new().schemes(["http"]), |ctx, request| {
            assert!(ctx.via::<SearxProtocol>());
            request.header("Cookie", "session=abc")
        })
        .build();
    requester
        .process(&server.searx_url("?q=Hello"))
        .await
        .unwrap();
    assert_eq!(
        server
            .raw_requests()
            .iter()
            .filter(|request| header(request, "cookie") == Some("session=abc"))
            .count(),
        2
    );
}

#[tokio::test]
async fn invalid_rule_header_fails_before_network_io() {
    let server = Server::start([]).await;
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .request_rule::<HttpRequest>(UrlMatcher::new(), |_, request| {
            request.header("Cookie", "bad\r\nheader")
        })
        .build();
    assert!(matches!(
        requester.process_request(server.base.as_str()).await,
        Err(Error::BadRequest)
    ));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn http_connection_is_reused_between_requests() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        for _ in 0..2 {
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                headers.push(socket.read_u8().await.unwrap());
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
                )
                .await
                .unwrap();
        }
    });
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .build();
    for _ in 0..2 {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            requester.process_request(&target),
        )
        .await
        .unwrap()
        .unwrap();
    }
    server.await.unwrap();
}

#[tokio::test]
async fn gopher_uses_url_rules_and_skips_http_rules() {
    use drova_plugins::protocols::GopherProtocol;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use url::Url;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("gopher://{}/0/original", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n") {
            request.push(socket.read_u8().await.unwrap());
        }
        assert_eq!(request, b"modified\r\n");
        socket.write_all(b"gopher response\r\n.\r\n").await.unwrap();
    });
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .request_rule::<Url>(UrlMatcher::new().schemes(["gopher"]), |ctx, mut url| {
            assert!(ctx.via::<GopherProtocol>());
            url.set_path("/0/modified");
            url
        })
        .request_rule::<HttpRequest>(UrlMatcher::new(), |_, _| panic!("HTTP rule ran on Gopher"))
        .build();
    let response = requester.process_request(&target).await.unwrap();
    assert_eq!(response.ty, "text/plain");
    server.await.unwrap();
}

#[tokio::test]
async fn later_http_rules_replace_headers() {
    let server = Server::start([Reply::json(RESULTS)]).await;
    let requester = RequesterBuilder::default()
        .plugin(requester_plugins)
        .request_rule::<HttpRequest>(UrlMatcher::new(), |_, request| {
            request.header("X-Rule", "first")
        })
        .request_rule::<HttpRequest>(UrlMatcher::new(), |_, request| {
            request.header("X-Rule", "second")
        })
        .build();
    requester
        .process_request(server.base.as_str())
        .await
        .unwrap();
    let request = &server.raw_requests()[0];
    assert_eq!(header(request, "x-rule"), Some("second"));
    assert_eq!(request.to_ascii_lowercase().matches("x-rule:").count(), 1);
}

#[tokio::test]
async fn standalone_searx_plugin_applies_http_rules_without_protocol_registration() {
    let server = Server::start([Reply::json(RESULTS)]).await;
    let requester = RequesterBuilder::default()
        .plugin(searx_plugin)
        .request_rule::<HttpRequest>(UrlMatcher::new(), |ctx, request| {
            assert!(ctx.via::<SearxProtocol>());
            assert!(ctx.via::<HttpProtocol>());
            request.header("Cookie", "session=standalone")
        })
        .build();
    requester
        .process(&server.searx_url("?q=Hello"))
        .await
        .unwrap();
    assert_eq!(
        header(&server.raw_requests()[0], "cookie"),
        Some("session=standalone")
    );
}

#[tokio::test]
async fn searx_honors_registered_transport_errors_without_falling_back() {
    use async_trait::async_trait;
    use drova_sdk::requester::{ProtocolHandler, Response};
    struct CustomHttp;
    #[async_trait]
    impl ProtocolHandler for CustomHttp {
        async fn fetch(&self, _: &url::Url) -> Result<Response, Error> {
            Err(Error::UnsupportedProtocol)
        }
    }
    let server = Server::start([]).await;
    let requester = RequesterBuilder::default()
        .plugin(searx_plugin)
        .protocol("http", &CustomHttp)
        .build();
    assert!(matches!(
        requester.process(&server.searx_url("?q=Hello")).await,
        Err(Error::UnsupportedProtocol)
    ));
    assert!(server.requests().is_empty());
}
