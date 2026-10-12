//! URL-selected adapters that run before generic MIME parsers.

mod context;
mod index;
mod matcher;
pub(crate) use index::{UrlFacts, UrlIndex};

pub use context::RequestContext;
pub use matcher::UrlMatcher;

use async_trait::async_trait;
use dalet::types::Page;

use crate::requester::Error;

/// Declining a request is explicit; handler errors are never silently discarded.
pub enum SiteOutcome {
    Parsed(Page),
    Fallback,
}

#[async_trait]
pub trait SiteHandler: Send + Sync {
    async fn process(&self, ctx: &mut RequestContext<'_, '_>) -> Result<SiteOutcome, Error>;
    fn source_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub(crate) struct SiteRegistration<'a> {
    pub matcher: UrlMatcher,
    pub handler: &'a dyn SiteHandler,
}
