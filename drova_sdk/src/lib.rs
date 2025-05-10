use async_trait::async_trait;
use dalet::typed::Page;
use glob_match::glob_match;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseData {
    TextOutput(String),
    BitsOutput(Vec<u8>),
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub data: ResponseData,
    pub mime: String,
}

#[async_trait]
pub trait Protocol: Send + Sync {
    async fn fetch(&self, url: &Url) -> Result<Response, Error>;
}

pub trait Input: Send + Sync {
    fn process_text(&self, s: String, url: Option<&Url>) -> Result<Page, Error>;
    fn process_bytes(&self, b: Vec<u8>, url: Option<&Url>) -> Result<Page, Error>;
}

pub struct Requester<'a> {
    protocols: IndexMap<String, &'a dyn Protocol>,
    inputs: IndexMap<String, &'a dyn Input>,
}

impl Default for Requester<'_> {
    fn default() -> Self {
        Self {
            protocols: IndexMap::new(),
            inputs: IndexMap::new(),
        }
    }
}

impl<'a> Requester<'a> {
    pub async fn process(&self, s: &str) -> Result<Page, Error> {
        use ResponseData::*;

        let url = Url::parse(s)?;

        let scheme = url.scheme();

        let protocol = self
            .protocols
            .get(scheme)
            .or_else(|| {
                self.protocols
                    .get(self.protocols.keys().find(|p| glob_match(&p, scheme))?)
            })
            .ok_or(Error::UnsupportedProtocol)?;

        let resp = protocol.fetch(&url).await?;

        let input = self
            .inputs
            .get(&resp.mime)
            .or_else(|| {
                self.inputs
                    .get(self.inputs.keys().find(|p| glob_match(&p, &resp.mime))?)
            })
            .ok_or(Error::UnsupportedInput)?;

        match resp.data {
            TextOutput(s) => input.process_text(s, Some(&url)),
            BitsOutput(b) => input.process_bytes(b, Some(&url)),
        }
    }

    pub fn process_text(&self, ty: &str, s: String) -> Result<Page, Error> {
        self.inputs
            .get(ty)
            .ok_or(Error::UnsupportedInput)?
            .process_text(s, None)
    }

    pub fn process_bytes(&self, ty: &str, s: Vec<u8>) -> Result<Page, Error> {
        self.inputs
            .get(ty)
            .ok_or(Error::UnsupportedInput)?
            .process_bytes(s, None)
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
        R: Fn(RequesterBuilder) -> RequesterBuilder,
    {
        registrar(self)
    }

    pub fn protocol(mut self, schema: &'a str, protocol: &'a dyn Protocol) -> Self {
        self.core.protocols.insert(schema.into(), protocol);
        self
    }

    pub fn input(mut self, ty: &'a str, input: &'a dyn Input) -> Self {
        self.core.inputs.insert(ty.into(), input);
        self
    }

    pub fn build(self) -> Requester<'a> {
        self.core
    }
}
