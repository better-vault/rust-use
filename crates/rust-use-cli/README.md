# rust-use-cli

Command-line client for driving a rust-use app from an agent.

> This is the `rust-use` binary. To build an app agents can drive, depend on
> [`rust-use`](https://crates.io/crates/rust-use) with the `ipc` feature.

```sh
cargo install rust-use-cli

rust-use list                          # running apps
rust-use --app chess snapshot          # what is on screen
rust-use --app chess act "board move e2e4"
rust-use --app chess mcp               # the same tools over MCP stdio
```

It prints the app's replies, exits 1 on an `err` reply, and keeps `diff` incremental across separate invocations. After an app restart, `diff` prints a notice and shows changes from revision zero. Any coding agent that can run a shell command, including Claude Code, Codex and Cursor, can drive a rust-use app with it.

rust-use is in beta: APIs may change between minor versions until 1.0.

- [Documentation](https://docs.rs/rust-use-cli)
- [Repository and the full README](https://github.com/better-vault/rust-use)
- [Changelog](https://github.com/better-vault/rust-use/blob/main/CHANGELOG.md)

## License

[MIT](https://github.com/better-vault/rust-use/blob/main/LICENSE) © Khalid Abdi
