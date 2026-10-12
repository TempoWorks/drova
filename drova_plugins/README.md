<div align="center">

<img alt="drova logo" src="https://github.com/TempoWorks/.github/raw/main/imgs/Drova.png" width='256'>

[![crates.io](https://img.shields.io/crates/v/drova_plugins.svg)](https://crates.io/crates/drova_plugins)

# DROVA Plugins

Absolute collection of plugins for DROVA.

</div>

# Usage

## Install

```bash
cargo add drova_plugins
```

## Example

```rust
use drova_sdk::{requester::{Error, RequesterBuilder}, sites::UrlMatcher};
use drova_plugins::{requester_plugins, searx_plugin, sites::SearxngSite};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
  let site = SearxngSite::new();
  let requester = RequesterBuilder::default()
      .plugin(requester_plugins)
      .plugin(searx_plugin)
      .site(
          UrlMatcher::new()
              .schemes(["http", "https"])
              .hosts(["search.example.com"])
              .paths(["/", "/search"])
              .require_query("q"),
          &site,
      )
      .build();

  println!("{:#?}", requester.process("searx://search.example.com?q=Hello").await?);
  println!("{:#?}", requester.process("https://search.example.com/search?q=Hello").await?);
  Ok(())
}
```

# Supported protocols

- [x] Http/s
- [x] Gemini
- [x] Gopher
- [x] SearXNG (`searx`, `searx+https`: HTTPS; `searx+http`: HTTP)

# Supported inputs

- [x] application/daletpack
- [x] text/plain, fallbacks to text/\*
- [x] text/gemini
- [x] text/markdown
- [x] text/html
- [x] application/x-searxng+json

# Supported outputs

- [x] application/daletpack
- [ ] text
- [ ] gemtext
- [ ] markdown
- [x] text/html
