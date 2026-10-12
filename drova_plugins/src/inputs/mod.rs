mod daletpack;
mod gemtext;
mod gophermap;
pub mod html;
mod markdown;
mod searx;
mod text;

pub use daletpack::DaletpackInput;
pub use gemtext::GemtextInput;
pub use gophermap::GophermapInput;
pub use html::HtmlInput;
pub use markdown::MarkdownInput;
pub(crate) use searx::{parse_site_response, prepare_response};
pub use searx::{SearxHtmlInput, SearxInput, SEARXNG_HTML, SEARXNG_JSON};
pub use text::TextInput;
