<div align="center">

<img alt="drova logo" src="https://github.com/TempoWorks/.github/raw/main/imgs/Drova.png" width='256'>

# DROVA Plugins

Absolute collection of plugins for DROVA.

</div>

# Usage

```rust
use drova_sdk::RequesterBuilder;
use drova_plugins::requester_plugins;

let requester = RequesterBuilder::default().plugin(requester_plugins).build();

println!("{:#?}", requester.process("gemini://example.com"))
```

# Supported protocols

- [x] Http/s
- [x] Gemini
- [ ] Gopher

# Supported inputs

- [ ] application/daletpack
- [x] text/plain, fallbacks to text/\*
- [x] text/gemini
- [x] text/markdown
- [ ] text/html

# Supported outputs

- [ ] daletpack
- [ ] text
- [ ] gemtext
- [ ] markdown
- [ ] html
