# rust-use-agent

The agent bridge for rust-use: compact snapshots, batched actions, queries and permissions.

> **Most projects should depend on [`rust-use`](https://crates.io/crates/rust-use)** rather than this
> crate directly: `cargo add rust-use`. It re-exports everything here behind features.

`snapshot`, `diff`, `act` and `query` over a compact, line-oriented text format: one line per component, listing only the actions available right now. An `act` reply carries exactly what changed, so an agent never needs a follow-up snapshot. `allow` vetoes agent actions. The format is specified in [`FORMAT.md`](https://github.com/better-vault/rust-use/blob/main/crates/rust-use-agent/FORMAT.md).

```text
rev 2
@1 board "Chess board" ="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" turn=w !move(uci) !reset
```

With the `ipc` feature it serves a running app over a local socket (a named pipe on Windows) to out-of-process agents. With `mcp` it serves the same three tools to any MCP client.

rust-use is in beta: APIs may change between minor versions until 1.0.

- [Documentation](https://docs.rs/rust-use-agent)
- [Repository and the full README](https://github.com/better-vault/rust-use)
- [Changelog](https://github.com/better-vault/rust-use/blob/main/CHANGELOG.md)

## License

[MIT](https://github.com/better-vault/rust-use/blob/main/LICENSE) © Khalid Abdi
