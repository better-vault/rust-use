# rust-use-gpui

GPUI adapter for rust-use.

> **Most projects should depend on [`rust-use`](https://crates.io/crates/rust-use)** rather than this
> crate directly: `cargo add rust-use`. It re-exports everything here behind features.

`ui.bind(cx)` re-renders a [GPUI](https://gpui.rs) view whenever the `Ui` changes, from any thread, coalescing bursts into one notify. `handle.on_click(..)` and `handle.on_mouse_down(..)` send GPUI input through the same actions an agent uses, so people and agents share one path and the same `allow` checks.

Shaders compile at runtime by default (the `runtime-shaders` feature), so building on macOS does not need Xcode's separate Metal Toolchain.

rust-use is in beta: APIs may change between minor versions until 1.0.

- [Documentation](https://docs.rs/rust-use-gpui)
- [Repository and the full README](https://github.com/better-vault/rust-use)
- [Changelog](https://github.com/better-vault/rust-use/blob/main/CHANGELOG.md)

## License

[MIT](https://github.com/better-vault/rust-use/blob/main/LICENSE) © Khalid Abdi
