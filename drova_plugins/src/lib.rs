use drova_sdk::CoreBuilder;
use gemini::{gemtext::GemtextInput, protocol::GeminiProtocol};
use http::protocol::HttpProtocol;
use text::TextInput;

mod utils;

pub mod gemini;
pub mod http;
pub mod text;

pub fn plugins<'a>(app: &'a mut CoreBuilder<'a>) {
    app.protocol("http", &HttpProtocol)
        .protocol("https", &HttpProtocol)
        .protocol("gemini", &GeminiProtocol)
        .input("text/gemini", &GemtextInput)
        .input("text/plain", &TextInput)
        .input("text/*", &TextInput);
}
