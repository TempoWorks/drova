use async_trait::async_trait;
use drova_sdk::requester::{Error, ProtocolHandler, Response, ResponseData};
use mime::Mime;
use reqwest::header::CONTENT_TYPE;

use crate::utils::mime_to_str;

pub struct HttpProtocol;

#[async_trait]
impl ProtocolHandler for HttpProtocol {
    async fn fetch(&self, url: &url::Url) -> Result<Response, Error> {
        let res = reqwest::get(url.to_string())
            .await
            .map_err(match_reqwest_error)?;

        let ty = mime_to_str(
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
            .map_err(|e| Error::InvalidMimeType(e.to_string()))?,
        );

        match ty.starts_with("text") {
            true => Ok(Response {
                data: ResponseData::TextOutput(res.text().await.map_err(match_reqwest_error)?),
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
