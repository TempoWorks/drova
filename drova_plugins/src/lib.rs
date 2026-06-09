use drova_sdk::requester::RequesterBuilder;
use inputs::{DaletpackInput, GemtextInput, GophermapInput, HtmlInput, MarkdownInput, TextInput};
use outputs::{DaletpackOutput, HtmlOutput};
use protocols::{GeminiProtocol, GopherProtocol, HttpProtocol};

pub mod inputs;
pub mod outputs;
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
        .output("application/daletpack", &DaletpackOutput)
        .output("text/html", &HtmlOutput)
}
