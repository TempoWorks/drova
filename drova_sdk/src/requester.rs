use async_trait::async_trait;
use dalet::types::Page;
use glob_match::glob_match;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use url::Url;

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
}

pub trait InputHandler: Send + Sync {
    fn process_text(&self, s: String, url: Option<&Url>) -> Result<Page, Error>;
    fn process_bytes(&self, b: Vec<u8>, url: Option<&Url>) -> Result<Page, Error>;
}

/// Requester is system for extracting dalet from anything,
/// through protocol and input handlers
pub struct Requester<'a> {
    protocols: IndexMap<String, &'a dyn ProtocolHandler>,
    inputs: IndexMap<String, &'a dyn InputHandler>,
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
    /// Process url and get dalet page
    pub async fn process(&self, url: &str) -> Result<Page, Error> {
        use ResponseData::*;

        let url = Url::parse(url)?;

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
            .get(&resp.ty)
            .or_else(|| {
                self.inputs
                    .get(self.inputs.keys().find(|p| glob_match(&p, &resp.ty))?)
            })
            .ok_or(Error::UnsupportedInput)?;

        match resp.data {
            TextOutput(s) => input.process_text(s, Some(&url)),
            BitsOutput(b) => input.process_bytes(b, Some(&url)),
        }
    }

    /// Process url and get response from protocol handler
    pub async fn process_request(&self, url: &str) -> Result<Response, Error> {
        let url = Url::parse(url)?;

        let scheme = url.scheme();

        let protocol = self
            .protocols
            .get(scheme)
            .or_else(|| {
                self.protocols
                    .get(self.protocols.keys().find(|p| glob_match(&p, scheme))?)
            })
            .ok_or(Error::UnsupportedProtocol)?;

        protocol.fetch(&url).await
    }

    /// Process text with input type and get dalet page
    pub fn process_text(&self, input_type: &str, text: String) -> Result<Page, Error> {
        self.inputs
            .get(input_type)
            .ok_or(Error::UnsupportedInput)?
            .process_text(text, None)
    }

    /// Process bytes with input type and get dalet page
    pub fn process_bytes(&self, input_type: &str, bytes: Vec<u8>) -> Result<Page, Error> {
        self.inputs
            .get(input_type)
            .ok_or(Error::UnsupportedInput)?
            .process_bytes(bytes, None)
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

    pub fn protocol(mut self, schema: &'a str, protocol: &'a dyn ProtocolHandler) -> Self {
        self.core.protocols.insert(schema.into(), protocol);
        self
    }

    pub fn input(mut self, ty: &'a str, input: &'a dyn InputHandler) -> Self {
        self.core.inputs.insert(ty.into(), input);
        self
    }

    pub fn build(self) -> Requester<'a> {
        self.core
    }
}
