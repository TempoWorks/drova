use async_trait::async_trait;
use dalet::types::Page;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::registry::Registry;
use crate::requests::{FetchContext, FetchFuture, RequestInfo, RequestRules, Source};
use crate::sites::{
    RequestContext, SiteHandler, SiteOutcome, SiteRegistration, UrlFacts, UrlIndex, UrlMatcher,
};

#[derive(Debug, Serialize, Deserialize)]
pub enum Error {
    GetInput,
    GetSecureInput,
    ClientCertRequired,

    NotAuthorized,
    NotFound,
    MethodNotAllowed,
    NotAcceptable,

    InvalidUrl,
    UnsupportedProtocol,
    UnsupportedInput,

    IoError(String),
    DnsFailed,
    TlsFailed,
    ExceededStatusSize,
    InvalidEncoding,
    InvalidMimeType(String),

    InvalidCert,

    ExceededMaxRedirects,

    TemporalFailure,
    Failure,

    ServerUnavailable,

    TooManyRequests,

    Gone,

    BadRequest,
    Forbidden,

    UnknownStatus(usize),
    InvalidStatus,

    InvalidSyntax,
    ParserError(String),
}

/// Response from protocol handler
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub data: ResponseData,

    /// Type of data for input handler. e.g. text/html
    pub ty: String,
}

/// Response data of protocol handler
#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseData {
    TextOutput(String),
    BitsOutput(Vec<u8>),
}

#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    async fn fetch(&self, url: &Url) -> Result<Response, Error>;

    fn fetch_with_context<'s>(
        &'s self,
        url: &'s Url,
        _: &'s FetchContext<'_, '_>,
    ) -> FetchFuture<'s> {
        self.fetch(url)
    }

    fn source_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait InputHandler: Send + Sync {
    fn process_text(&self, s: String, url: Option<&Url>) -> Result<Page, Error>;
    fn process_bytes(&self, b: Vec<u8>, url: Option<&Url>) -> Result<Page, Error>;
}

pub enum OutputData {
    Text(String),
    Bytes(Vec<u8>),
}

pub trait OutputHandler: Send + Sync {
    fn process_page(&self, page: Page) -> Result<OutputData, Error>;
}

/// Requester is system for extracting dalet from anything,
/// through protocol and input handlers
pub struct Requester<'a> {
    protocols: Registry<'a, dyn ProtocolHandler + 'a>,
    inputs: Registry<'a, dyn InputHandler + 'a>,
    outputs: Registry<'a, dyn OutputHandler + 'a>,
    sites: Vec<SiteRegistration<'a>>,
    site_index: UrlIndex,
    pub(crate) rules: RequestRules<'a>,
}

impl Default for Requester<'_> {
    fn default() -> Self {
        Self {
            protocols: Registry::default(),
            inputs: Registry::default(),
            outputs: Registry::default(),
            sites: Vec::new(),
            site_index: UrlIndex::default(),
            rules: RequestRules::default(),
        }
    }
}

impl<'a> Requester<'a> {
    pub fn protocols(&self) -> impl Iterator<Item = &str> {
        self.protocols.keys()
    }

    pub fn inputs(&self) -> impl Iterator<Item = &str> {
        self.inputs.keys()
    }

    pub fn outputs(&self) -> impl Iterator<Item = &str> {
        self.outputs.keys()
    }

    /// Process url and get dalet page
    pub async fn process(&self, url: &str) -> Result<Page, Error> {
        let url = Url::parse(url)?;
        let mut ctx = RequestContext::new(self, &url);

        let facts = UrlFacts::new(&url);
        for id in self.site_index.candidates(&facts) {
            let site = &self.sites[id];
            if site.matcher.matches_facts(&facts) {
                ctx.set_source(site.handler.source_name());
                match site.handler.process(&mut ctx).await? {
                    SiteOutcome::Parsed(page) => return Ok(page),
                    SiteOutcome::Fallback => continue,
                }
            }
        }

        self.parse_response(ctx.into_original_response().await?, Some(&url))
    }

    /// Fetch through protocol handlers without invoking site adapters or parsers.
    pub async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.fetch_from(url, None).await
    }

    pub(crate) async fn fetch_from(
        &self,
        url: &Url,
        parent: Option<&Source<'_>>,
    ) -> Result<Response, Error> {
        let scheme = url.scheme();

        let protocol = self
            .protocols
            .matching(scheme)
            .ok_or(Error::UnsupportedProtocol)?;

        let ctx = FetchContext {
            requester: self,
            source: Source {
                name: protocol.source_name(),
                parent,
            },
        };
        protocol.fetch_with_context(url, &ctx).await
    }

    /// Parse a fetched response using exact MIME matching, then registered globs.
    pub fn parse_response(&self, resp: Response, url: Option<&Url>) -> Result<Page, Error> {
        use ResponseData::*;

        let input = self
            .inputs
            .matching(&resp.ty)
            .ok_or(Error::UnsupportedInput)?;

        match resp.data {
            TextOutput(s) => input.process_text(s, url),
            BitsOutput(b) => input.process_bytes(b, url),
        }
    }

    /// Process url and get response from protocol handler
    pub async fn process_request(&self, url: &str) -> Result<Response, Error> {
        let url = Url::parse(url)?;
        self.fetch(&url).await
    }

    /// Process text with input type and get dalet page
    pub fn process_text(&self, input_type: &str, text: String) -> Result<Page, Error> {
        self.inputs
            .exact(input_type)
            .ok_or(Error::UnsupportedInput)?
            .process_text(text, None)
    }

    /// Process bytes with input type and get dalet page
    pub fn process_bytes(&self, input_type: &str, bytes: Vec<u8>) -> Result<Page, Error> {
        self.inputs
            .exact(input_type)
            .ok_or(Error::UnsupportedInput)?
            .process_bytes(bytes, None)
    }

    /// Process dalet page with output type and get rendered output
    pub fn process_output(&self, output_type: &str, page: Page) -> Result<OutputData, Error> {
        self.outputs
            .matching(output_type)
            .ok_or(Error::UnsupportedInput)?
            .process_page(page)
    }
}

impl From<url::ParseError> for Error {
    fn from(_: url::ParseError) -> Self {
        Error::InvalidUrl
    }
}

pub struct RequesterBuilder<'a> {
    core: Requester<'a>,
}

impl Default for RequesterBuilder<'_> {
    fn default() -> Self {
        Self {
            core: Requester::default(),
        }
    }
}

impl<'a> RequesterBuilder<'a> {
    pub fn plugin<R>(self, registrar: R) -> Self
    where
        R: FnOnce(RequesterBuilder<'a>) -> RequesterBuilder<'a>,
    {
        registrar(self)
    }

    pub fn protocol(mut self, schema: &'a str, protocol: &'a dyn ProtocolHandler) -> Self {
        self.core.protocols.insert(schema.into(), protocol);
        self
    }

    /// Try matching site adapters in registration order before MIME parsing.
    pub fn site(mut self, matcher: UrlMatcher, handler: &'a dyn SiteHandler) -> Self {
        self.core.sites.push(SiteRegistration { matcher, handler });
        self
    }

    pub fn request_rule<R: 'static>(
        mut self,
        matcher: UrlMatcher,
        modify: impl Fn(&RequestInfo<'_>, R) -> R + Send + Sync + 'a,
    ) -> Self {
        self.core.rules.insert(matcher, modify);
        self
    }

    pub fn input(mut self, ty: &'a str, input: &'a dyn InputHandler) -> Self {
        self.core.inputs.insert(ty.into(), input);
        self
    }

    pub fn output(mut self, ty: &'a str, output: &'a dyn OutputHandler) -> Self {
        self.core.outputs.insert(ty.into(), output);
        self
    }

    pub fn build(mut self) -> Requester<'a> {
        self.core.site_index = UrlIndex::build(self.core.sites.iter().map(|site| &site.matcher));
        self.core.rules.build();
        self.core
    }
}
