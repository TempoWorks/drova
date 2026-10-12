use std::any::type_name;

use url::Url;

use crate::requester::{Error, ProtocolHandler, Requester, Response};

pub(crate) struct Source<'r> {
    pub name: &'static str,
    pub parent: Option<&'r Source<'r>>,
}

pub struct RequestInfo<'r> {
    pub(crate) url: &'r Url,
    pub(crate) source: Option<&'r Source<'r>>,
}

impl RequestInfo<'_> {
    pub fn url(&self) -> &Url {
        self.url
    }

    /// Whether this request was initiated through a particular handler type.
    pub fn via<T: ?Sized>(&self) -> bool {
        let mut source = self.source;
        while let Some(current) = source {
            if current.name == type_name::<T>() {
                return true;
            }
            source = current.parent;
        }
        false
    }
}

/// Transport dispatch and typed rules available to a protocol implementation.
pub struct FetchContext<'r, 'a> {
    pub(crate) requester: &'r Requester<'a>,
    pub(crate) source: Source<'r>,
}

impl FetchContext<'_, '_> {
    pub fn has_rules<R: 'static>(&self) -> bool {
        self.requester.rules.has::<R>()
    }
    pub fn apply_rules<R: 'static>(&self, url: &Url, request: R) -> R {
        self.requester.rules.apply(url, Some(&self.source), request)
    }

    /// Delegate to another registered protocol while preserving initiator context.
    pub async fn fetch(&self, url: &Url) -> Result<Response, Error> {
        self.requester.fetch_from(url, Some(&self.source)).await
    }

    /// Use a fallback transport only when no registered protocol matches.
    pub async fn fetch_or(
        &self,
        url: &Url,
        fallback: &dyn ProtocolHandler,
    ) -> Result<Response, Error> {
        let protocol: &dyn ProtocolHandler = match self.requester.protocol_for(url) {
            Some(protocol) => protocol,
            None => fallback,
        };
        let ctx = FetchContext {
            requester: self.requester,
            source: Source {
                name: protocol.source_name(),
                parent: Some(&self.source),
            },
        };
        protocol.fetch_with_context(url, &ctx).await
    }
}
