<div align="center">

<img alt="drova logo" src="https://github.com/TempoWorks/.github/raw/main/imgs/Drova.png" width='256'>

[![crates.io](https://img.shields.io/crates/v/drova_sdk.svg)](https://crates.io/crates/drova_sdk)

# DROVA SDK

Absolute SDK for DROVA.

</div>

# Usage

## Install

```bash
cargo add drova_sdk
```

## Example

```rust
use dalet::types::{Page};
use drova_sdk::requester::{Error, InputHandler};
use async_trait::async_trait;
use drova_sdk::requester::{Error, ProtocolHandler, Response};

pub struct HttpProtocol;

#[async_trait]
impl ProtocolHandler for HttpProtocol {
    async fn fetch(&self, url: &url::Url) -> Result<Response, Error> {
        todo!()
    }
}

pub struct TextInput;

impl InputHandler for TextInput {
    fn process_text(&self, data: String, _: Option<&url::Url>) -> Result<Page, Error> {
        todo!()
    }

    fn process_bytes(&self, _: Vec<u8>, _: Option<&url::Url>) -> Result<Page, Error> {
        todo!()
    }
}

fn main() {
  let requester = RequesterBuilder::default()
        .protocol("http", &HttpProtocol)
        .protocol("https", &HttpProtocol)
        .input("text/plain", &TextInput)
        .input("text/*", &TextInput)
        .build();

  println!("{:#?}", requester.process("http://example.com"))
}
```
