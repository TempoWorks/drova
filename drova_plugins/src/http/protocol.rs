use async_trait::async_trait;
use drova_sdk::{Error, Protocol, Response, ResponseType};
use mime::Mime;
use reqwest::header::CONTENT_TYPE;

use crate::utils::mime_to_str;

pub struct HttpProtocol;

#[async_trait]
impl Protocol for HttpProtocol {
    async fn fetch(&self, url: &url::Url) -> Result<Response, Error> {
        let res = reqwest::get(url.to_string())
            .await
            .map_err(match_reqwest_error)?;

        let mime = mime_to_str(
            {
                match res.headers().get(CONTENT_TYPE) {
                    Some(header) => match header.to_str() {
                        Ok(mime) => mime.to_owned(),
                        Err(_) => "text/plain".to_owned(),
                    },
                    None => "text/plain".to_owned(),
                }
            }
            .parse::<Mime>()
            .map_err(|_| Error::InvalidMimeType)?,
        );

        match mime.starts_with("text") {
            true => Ok(Response {
                ty: ResponseType::TextOutput(res.text().await.map_err(match_reqwest_error)?),
                mime,
            }),
            false => Ok(Response {
                ty: ResponseType::BitsOutput(
                    res.bytes().await.map_err(match_reqwest_error)?.to_vec(),
                ),
                mime,
            }),
        }
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
            _ => Error::InvalidStatus,
        },
        None => Error::InvalidStatus,
    }
}
