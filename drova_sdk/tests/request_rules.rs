use async_trait::async_trait;
use drova_sdk::{
    requester::{Error, ProtocolHandler, RequesterBuilder, Response, ResponseData},
    requests::{FetchContext, FetchFuture},
    sites::{RequestContext, SiteHandler, SiteOutcome, UrlMatcher},
};
use url::Url;

struct Payload(String);
struct Echo;

#[async_trait]
impl ProtocolHandler for Echo {
    async fn fetch(&self, _: &Url) -> Result<Response, Error> {
        Ok(Response {
            ty: "text/plain".into(),
            data: ResponseData::TextOutput("legacy".into()),
        })
    }
    fn fetch_with_context<'s>(
        &'s self,
        url: &'s Url,
        ctx: &'s FetchContext<'_, '_>,
    ) -> FetchFuture<'s> {
        Box::pin(async move {
            let payload = ctx.apply_rules(url, Payload(String::new()));
            Ok(Response {
                ty: "text/plain".into(),
                data: ResponseData::TextOutput(payload.0),
            })
        })
    }
}

struct Delegating;

#[async_trait]
impl ProtocolHandler for Delegating {
    async fn fetch(&self, _: &Url) -> Result<Response, Error> {
        Err(Error::Failure)
    }
    fn fetch_with_context<'s>(
        &'s self,
        _: &'s Url,
        ctx: &'s FetchContext<'_, '_>,
    ) -> FetchFuture<'s> {
        Box::pin(async move {
            ctx.fetch(&Url::parse("test://api.example.org/data").unwrap())
                .await
        })
    }
}

struct Site;

#[async_trait]
impl SiteHandler for Site {
    async fn process(&self, ctx: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error> {
        // Original/detection fetch deliberately has no site provenance.
        let ResponseData::TextOutput(original) = &ctx.original_response().await?.data else {
            panic!()
        };
        assert_eq!(original, "P");
        let ResponseData::TextOutput(api) = ctx
            .fetch(&Url::parse("test://api.example.org/data").unwrap())
            .await?
            .data
        else {
            panic!()
        };
        assert_eq!(api, "PS");
        Ok(SiteOutcome::Fallback)
    }
}

fn text(response: Response) -> String {
    let ResponseData::TextOutput(text) = response.data else {
        panic!()
    };
    text
}

#[tokio::test]
async fn rules_preserve_order_filter_urls_and_skip_other_request_types() {
    let captured = "captured".to_owned();
    let requester = RequesterBuilder::default()
        .protocol("test", &Echo)
        .plugin(|builder| {
            builder.request_rule::<Payload>(
                UrlMatcher::new().hosts(["*.example.org"]),
                |_, mut r| {
                    r.0.push_str(&captured);
                    r
                },
            )
        })
        .request_rule::<Payload>(UrlMatcher::new().hosts(["api.example.org"]), |_, mut r| {
            r.0.push('2');
            r
        })
        .request_rule::<Payload>(UrlMatcher::new().path_glob("/data*"), |_, mut r| {
            r.0.push('3');
            r
        })
        .request_rule::<Vec<u8>>(UrlMatcher::new(), |_, _| panic!("wrong request type"))
        .build();
    assert_eq!(
        text(
            requester
                .process_request("test://api.example.org/data")
                .await
                .unwrap()
        ),
        "captured23"
    );
    assert_eq!(
        text(
            requester
                .process_request("test://other.net/data")
                .await
                .unwrap()
        ),
        "3"
    );
    assert_eq!(
        text(
            requester
                .process_request("test://other.net/elsewhere")
                .await
                .unwrap()
        ),
        ""
    );
}

#[tokio::test]
async fn nested_protocols_preserve_initiator_and_match_outgoing_url() {
    let requester = RequesterBuilder::default()
        .protocol("adapter", &Delegating)
        .protocol("test", &Echo)
        .request_rule::<Payload>(
            UrlMatcher::new()
                .schemes(["test"])
                .hosts(["api.example.org"]),
            |ctx, mut r| {
                assert!(ctx.via::<Echo>());
                assert!(ctx.via::<Delegating>());
                assert_eq!(ctx.url().path(), "/data");
                r.0.push('D');
                r
            },
        )
        .build();
    assert_eq!(
        text(
            requester
                .process_request("adapter://different.net/start")
                .await
                .unwrap()
        ),
        "D"
    );
}

#[tokio::test]
async fn site_provenance_applies_to_api_but_not_original_or_fallback() {
    let requester = RequesterBuilder::default()
        .protocol("test", &Echo)
        .site(UrlMatcher::new().hosts(["site.example.org"]), &Site)
        .request_rule::<Payload>(UrlMatcher::new(), |ctx, mut r| {
            assert!(ctx.via::<Echo>());
            r.0.push('P');
            if ctx.via::<Site>() {
                r.0.push('S');
            }
            r
        })
        .build();
    assert!(matches!(
        requester.process("test://site.example.org/page").await,
        Err(Error::UnsupportedInput)
    ));
}
