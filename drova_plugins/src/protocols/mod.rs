mod gemini;
mod gopher;
mod http;
mod searx;

pub use gemini::GeminiProtocol;
pub use gopher::{GopherProtocol, ItemType as GopherItemType};
pub use http::HttpProtocol;
pub(crate) use searx::api_unavailable;
pub use searx::{html_url, json_url, search_url, SearxFallback, SearxProtocol};
