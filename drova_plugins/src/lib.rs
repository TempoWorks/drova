use drova_sdk::CoreBuilder;
use gemini::{gemtext::GemtextInput, protocol::GeminiProtocol};
use http::protocol::HttpProtocol;
use markdown::MarkdownInput;
use text::TextInput;

mod utils;

pub mod gemini;
pub mod http;
pub mod markdown;
pub mod text;

pub fn plugins(app: CoreBuilder) -> CoreBuilder {
    app.protocol("http", &HttpProtocol)
        .protocol("https", &HttpProtocol)
        .protocol("gemini", &GeminiProtocol)
        .input("text/gemini", &GemtextInput)
        .input("text/markdown", &MarkdownInput)
        .input("text/x-markdown", &MarkdownInput)
        .input("text/plain", &TextInput)
        .input("text/*", &TextInput)
}
