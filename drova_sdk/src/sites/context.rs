use url::Url;

use crate::requester::{Error, Requester, Response};

/// Constructing a context does no I/O. Original content is fetched only on demand.
pub struct RequestContext<'r, 'a> {
    requester: &'r Requester<'a>,
    url: &'r Url,
    original: Option<Response>,
}

impl<'r, 'a> RequestContext<'r, 'a> {
    pub(crate) fn new(requester: &'r Requester<'a>, url: &'r Url) -> Self {
        Self {
            requester,
            url,
            original: None,
        }
    }

    pub fn url(&self) -> &Url {
        self.url
    }

    /// Fetch an alternate resource through protocol handlers, bypassing site dispatch.
    pub async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.requester.fetch(url).await
    }

    /// Share the original response across handlers and the final MIME parser.
    pub async fn original_response(&mut self) -> Result<&Response, Error> {
        if self.original.is_none() {
            self.original = Some(self.fetch(self.url).await?);
        }
        Ok(self.original.as_ref().expect("original response fetched"))
    }

    pub(crate) async fn into_original_response(mut self) -> Result<Response, Error> {
        match self.original.take() {
            Some(response) => Ok(response),
            None => self.fetch(self.url).await,
        }
    }
}
