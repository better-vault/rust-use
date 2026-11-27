# rust-use-macros

Derive and attribute macros for rust-use.

> **Most projects should depend on [`rust-use`](https://crates.io/crates/rust-use)** rather than this
> crate directly: `cargo add rust-use`. It re-exports everything here behind features.

`#[derive(Primitive)]` turns a struct's fields into what an agent sees: a value, attributes and flags, with `#[agent(rename = ..)]` and `#[agent(skip)]`, and embedded `#[behavior]` fields merged in. `#[actions]` turns `&mut self` methods into actions, whose parameter types and doc comments are what the agent reads.

```rust
use rust_use::prelude::*;

/// A button that mutes audio.
#[derive(Primitive, Default)]
struct MuteButton {
    #[agent(value)]
    muted: bool,
}

#[actions]
impl MuteButton {
    /// Mute or unmute.
    fn toggle(&mut self) {
        self.muted = !self.muted;
    }
}
```

rust-use is in beta: APIs may change between minor versions until 1.0.

- [Documentation](https://docs.rs/rust-use-macros)
- [Repository and the full README](https://github.com/better-vault/rust-use)
- [Changelog](https://github.com/better-vault/rust-use/blob/main/CHANGELOG.md)

## License

[MIT](https://github.com/better-vault/rust-use/blob/main/LICENSE) © Khalid Abdi
