use async_trait::async_trait;
use drova_sdk::requester::{Error, ProtocolHandler, Response, ResponseData};
use drova_sdk::requests::{FetchContext, FetchFuture};
use mime::Mime;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use std::sync::OnceLock;
use url::Url;

pub use reqwest::Method as HttpMethod;

use crate::utils::{decode_bytes, mime_to_str};

pub struct HttpProtocol;

impl HttpProtocol {
    pub async fn send(&self, request: HttpRequest) -> Result<Response, Error> {
        let res = client()?
            .execute(request.inner?)
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(match_reqwest_error)?;

        let ty = mime_to_str(
            res.headers()
                .get(CONTENT_TYPE)
                .and_then(|header| header.to_str().ok())
                .unwrap_or("text/plain")
                .parse::<Mime>()
                .map_err(|e| Error::InvalidMimeType(e.to_string()))?,
        );

        match ty.starts_with("text") {
            true => Ok(Response {
                data: ResponseData::TextOutput(
                    decode_bytes(&res.bytes().await.map_err(match_reqwest_error)?)
                        .map_err(|e| Error::ParserError(e.to_string()))?,
                ),
                ty,
            }),
            false => Ok(Response {
                data: ResponseData::BitsOutput(
                    res.bytes().await.map_err(match_reqwest_error)?.to_vec(),
                ),
                ty,
            }),
        }
    }
}

#[async_trait]
impl ProtocolHandler for HttpProtocol {
    async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.send(HttpRequest::new(url)).await
    }

    fn fetch_with_context<'s>(
        &'s self,
        url: &'s Url,
        ctx: &'s FetchContext<'_, '_>,
    ) -> FetchFuture<'s> {
        Box::pin(async move {
            let request = ctx.apply_rules(url, HttpRequest::new(url));
            self.send(request).await
        })
    }
}

fn client() -> Result<&'static reqwest::Client, Error> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .map_err(|e| e.to_string())
    }) {
        Ok(client) => Ok(client),
        Err(error) => Err(Error::IoError(error.clone())),
    }
}

/// HTTP request state passed to independently registered request rules.
#[derive(Debug)]
pub struct HttpRequest {
    inner: Result<reqwest::Request, Error>,
}

impl HttpRequest {
    pub fn new(url: &Url) -> Self {
        Self {
            inner: client()
                .and_then(|client| client.get(url.clone()).build().map_err(match_reqwest_error)),
        }
    }

    /// Set a header; later rules replace earlier values of the same name.
    pub fn header(mut self, name: &str, value: impl AsRef<str>) -> Self {
        if let Ok(request) = &mut self.inner {
            match (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value.as_ref()),
            ) {
                (Ok(name), Ok(value)) => {
                    request.headers_mut().insert(name, value);
                }
                _ => self.inner = Err(Error::BadRequest),
            }
        }
        self
    }

    pub fn method(mut self, method: HttpMethod) -> Self {
        if let Ok(request) = &mut self.inner {
            *request.method_mut() = method;
        }
        self
    }

    pub fn body(mut self, body: impl Into<reqwest::Body>) -> Self {
        if let Ok(request) = &mut self.inner {
            *request.body_mut() = Some(body.into());
        }
        self
    }

    pub fn request_mut(&mut self) -> Option<&mut reqwest::Request> {
        self.inner.as_mut().ok()
    }
    pub fn into_request(self) -> Result<reqwest::Request, Error> {
        self.inner
    }
}

fn match_reqwest_error(e: reqwest::Error) -> Error {
    match e.status() {
        Some(s) => match s.as_u16() {
            400 => Error::BadRequest,
            401 => Error::NotAuthorized,
            403 => Error::Forbidden,
            404 => Error::NotFound,
            405 => Error::MethodNotAllowed,
            406 => Error::NotAcceptable,
            410 => Error::Gone,
            429 => Error::TooManyRequests,
            500 => Error::Failure,
            503 => Error::ServerUnavailable,
            s => Error::UnknownStatus(s.into()),
        },
        None => Error::InvalidStatus,
    }
}
