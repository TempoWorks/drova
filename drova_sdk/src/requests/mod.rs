//! Typed request modifiers, independent of protocol and parser registration.

mod context;
mod rules;

pub(crate) use context::Source;
pub use context::{FetchContext, RequestInfo};
pub(crate) use rules::RequestRules;

pub type FetchFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<crate::requester::Response, crate::requester::Error>,
            > + Send
            + 'a,
    >,
>;
