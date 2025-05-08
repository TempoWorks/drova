use std::str;

use async_trait::async_trait;
use drova_sdk::{Error, Protocol, Response, ResponseData};
use tokio_gemini::{certs::SelfsignedCertVerifier, LibError, StatusCode};

use crate::utils::mime_to_str;

pub struct GeminiProtocol;

#[async_trait]
impl Protocol for GeminiProtocol {
    async fn fetch(&self, url: &url::Url) -> Result<Response, Error> {
        let client = tokio_gemini::Client::builder()
            .with_selfsigned_cert_verifier(CertVerifier)
            .build();

        let mut resp = client
            .request(url.to_string())
            .await
            .map_err(match_lib_err)?;

        match resp.status().status_code() {
            StatusCode::Success => {
                let mime = mime_to_str(resp.mime().map_err(match_lib_err)?);

                match mime.starts_with("text") {
                    true => Ok(Response {
                        data: ResponseData::TextOutput(resp.text().await.map_err(match_lib_err)?),
                        mime,
                    }),
                    false => Ok(Response {
                        data: ResponseData::BitsOutput(
                            resp.bytes().await.map_err(match_lib_err)?.to_vec(),
                        ),
                        mime,
                    }),
                }
            }
            StatusCode::Input => Err(Error::GetInput),
            StatusCode::InputSensitive => Err(Error::GetSecureInput),
            StatusCode::NotFound => Err(Error::NotFound),
            StatusCode::TempRedirect => Err(Error::ExceededMaxRedirects),
            StatusCode::PermRedirect => Err(Error::ExceededMaxRedirects),
            StatusCode::TempFail => Err(Error::TemporalFailure),
            StatusCode::ServerUnavailable => Err(Error::ServerUnavailable),
            StatusCode::CgiError => Err(Error::Failure),
            StatusCode::ProxyError => Err(Error::UnsupportedProtocol),
            StatusCode::SlowDown => Err(Error::TooManyRequests),
            StatusCode::PermFail => Err(Error::Failure),
            StatusCode::Gone => Err(Error::Gone),
            StatusCode::ProxyRequestRefused => todo!(),
            StatusCode::BadRequest => Err(Error::BadRequest),
            StatusCode::ClientCerts => Err(Error::ClientCertRequired),
            StatusCode::CertNotAuthorized => Err(Error::NotAuthorized),
            StatusCode::CertNotValid => Err(Error::InvalidCert),
            StatusCode::Unknown(s) => Err(Error::UnknownStatus(s.into())),
        }
    }
}

// TODO: remove when there will be file cert verifier in tokio_gemini
struct CertVerifier;
#[async_trait]
impl SelfsignedCertVerifier for CertVerifier {
    async fn verify(
        &self,
        _: &tokio_gemini::certs::CertificateDer<'_>,
        _: &str,
        _: u16,
    ) -> Result<bool, tokio_gemini::LibError> {
        Ok(true)
    }
}

fn match_lib_err(e: LibError) -> Error {
    match e {
        LibError::IoError(e) => Error::IoError(e.to_string()),
        LibError::InvalidUrlError(_) => Error::InvalidUrl,
        LibError::HostLookupError => Error::DnsFailed,
        LibError::RustlsError(_) => Error::TlsFailed,
        LibError::StatusOutOfRange(_) => Error::ExceededStatusSize,
        LibError::DataNotUtf8(_) => Error::InvalidEncoding,
        LibError::InvalidMime(e) => Error::InvalidMimeType(e.to_string()),
    }
}
