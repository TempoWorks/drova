use drova_sdk::requester::RequesterBuilder;
use gemini::{gemtext::GemtextInput, protocol::GeminiProtocol};
use gopher::{gophermap::GophermapInput, protocol::GopherProtocol};
use html::HtmlInput;
use http::protocol::HttpProtocol;
use markdown::MarkdownInput;
use text::TextInput;

mod utils;

pub mod gemini;
pub mod gopher;
pub mod html;
pub mod http;
pub mod markdown;
pub mod text;

pub fn requester_plugins(app: RequesterBuilder) -> RequesterBuilder {
    app.protocol("http", &HttpProtocol)
        .protocol("https", &HttpProtocol)
        .protocol("gemini", &GeminiProtocol)
        .protocol("gopher", &GopherProtocol)
        .input("text/gemini", &GemtextInput)
        .input("text/x-gophermap", &GophermapInput)
        .input("text/markdown", &MarkdownInput)
        .input("text/x-markdown", &MarkdownInput)
        .input("text/html", &HtmlInput)
        .input("text/plain", &TextInput)
        .input("text/*", &TextInput)
}
