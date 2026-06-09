use drova_sdk::requester::RequesterBuilder;
use inputs::{
    DaletpackInput, GemtextInput, GophermapInput, HtmlInput, MarkdownInput, TextInput,
};
use protocols::{GeminiProtocol, GopherProtocol, HttpProtocol};

pub mod inputs;
pub mod protocols;
mod utils;

pub fn requester_plugins(app: RequesterBuilder) -> RequesterBuilder {
    app.protocol("http", &HttpProtocol)
        .protocol("https", &HttpProtocol)
        .protocol("gemini", &GeminiProtocol)
        .protocol("gopher", &GopherProtocol)
        .input("application/daletpack", &DaletpackInput)
        .input("text/gemini", &GemtextInput)
        .input("text/x-gophermap", &GophermapInput)
        .input("text/markdown", &MarkdownInput)
        .input("text/x-markdown", &MarkdownInput)
        .input("text/html", &HtmlInput)
        .input("text/plain", &TextInput)
        .input("text/*", &TextInput)
}
