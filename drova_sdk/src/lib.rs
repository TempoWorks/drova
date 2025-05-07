use async_trait::async_trait;
use dalet::typed::Page;
use glob_match::glob_match;
use indexmap::IndexMap;
use url::Url;

#[derive(Debug)]
pub enum ResponseData {
    TextOutput(String),
    BitsOutput(Vec<u8>),
}

#[derive(Debug)]
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

    IoError,
    DnsFailed,
    TlsFailed,
    ExceededStatusSize,
    InvalidEncoding,
    InvalidMimeType,

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
}

pub struct Response {
    pub data: ResponseData,
    pub mime: String,
}

#[async_trait]
pub trait Protocol {
    async fn fetch(&self, url: &Url) -> Result<Response, Error>;
}

pub trait Input {
    fn process_text(&self, s: String, url: Option<&Url>) -> Result<Page, Error>;
    fn process_bytes(&self, b: Vec<u8>, url: Option<&Url>) -> Result<Page, Error>;
}

pub struct Core<'a> {
    protocols: IndexMap<String, &'a dyn Protocol>,
    inputs: IndexMap<String, &'a dyn Input>,
}

impl Default for Core<'_> {
    fn default() -> Self {
        Self {
            protocols: IndexMap::new(),
            inputs: IndexMap::new(),
        }
    }
}

impl<'a> Core<'a> {
    pub async fn process(self, s: &str) -> Result<Page, Error> {
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

    pub fn process_text(self, ty: &str, s: String) -> Result<Page, Error> {
        self.inputs
            .get(ty)
            .ok_or(Error::UnsupportedInput)?
            .process_text(s, None)
    }

    pub fn process_bytes(self, ty: &str, s: Vec<u8>) -> Result<Page, Error> {
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

pub struct CoreBuilder<'a> {
    core: Core<'a>,
}

impl Default for CoreBuilder<'_> {
    fn default() -> Self {
        Self {
            core: Core::default(),
        }
    }
}

impl<'a> CoreBuilder<'a> {
    pub fn plugins<R>(&'a mut self, registrars: Vec<R>) -> &'a mut Self
    where
        R: Fn(&mut CoreBuilder),
    {
        for registrar in registrars {
            registrar(self);
        }

        self
    }

    pub fn plugin<R>(&'a mut self, registrar: R) -> &'a mut Self
    where
        R: Fn(&mut CoreBuilder),
    {
        registrar(self);

        self
    }

    pub fn protocol(&'a mut self, schema: &'a str, protocol: &'a dyn Protocol) -> &'a mut Self {
        self.core.protocols.insert(schema.into(), protocol);
        self
    }

    pub fn input(&'a mut self, ty: &'a str, input: &'a dyn Input) -> &'a mut Self {
        self.core.inputs.insert(ty.into(), input);
        self
    }

    pub fn build(self) -> Core<'a> {
        self.core
    }
}
