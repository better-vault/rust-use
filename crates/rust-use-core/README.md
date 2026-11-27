# rust-use-core

Core state, primitives and behaviors for rust-use.

> **Most projects should depend on [`rust-use`](https://crates.io/crates/rust-use)** rather than this
> crate directly: `cargo add rust-use`. It re-exports everything here behind features.

The `Ui` registry of mounted components, typed `Handle`s, actions that carry a `ChangeSource` and pass through interceptors, change events, the `Behavior` and `Primitive` traits, and the nine 0.1 behaviors: `Press`, `Toggle`, `Field`, `Selection`, `List`, `RovingFocus`, `Typeahead`, `Popup` and `DragDrop`.

It has no UI dependency. Adapters such as `rust-use-gpui` sit on top of it.

rust-use is in beta: APIs may change between minor versions until 1.0.

- [Documentation](https://docs.rs/rust-use-core)
- [Repository and the full README](https://github.com/better-vault/rust-use)
- [Changelog](https://github.com/better-vault/rust-use/blob/main/CHANGELOG.md)

## License

[MIT](https://github.com/better-vault/rust-use/blob/main/LICENSE) © Khalid Abdi
