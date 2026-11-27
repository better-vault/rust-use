# Changelog

Notable changes to [`rust-use`](https://crates.io/crates/rust-use) and its crates.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [semantic versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor release
(0.2.0, 0.3.0) may change the API, and a patch (0.1.1) is a fix that leaves it alone.

## [0.1.0] — 2026-11-27

The first release, in beta. rust-use is a library of behaviors for agents to build and drive user
interfaces. You describe a component as a Rust struct and its methods, mount it, and an agent can
see it and operate it by name. Against generic UI automation over MCP, Claude Code finished a
full chess game in the demo with 23× fewer tool calls, 7.3× fewer input tokens, in a quarter of the
time and at a sixth of the cost.

Install with `cargo add rust-use`. Six crates ship together at this version: `rust-use` (the facade
you depend on), `rust-use-core`, `rust-use-macros`, `rust-use-agent`, `rust-use-gpui` and
`rust-use-cli`.

### Added

- **The core.** A thread-safe `Ui` registry of mounted components with stable ids (`@1`, `@2`),
  labels and per-component revisions. Every change goes through an action that carries a
  `ChangeSource` (user, agent or program), can be vetoed by an interceptor beforehand, and is
  announced to subscribers afterwards. Typed `Handle`s give app code direct access.
- **`#[derive(Primitive)]` and `#[actions]`.** A struct's fields become what an agent sees (a
  value, attributes and flags, with renames and skips), and its `&mut self` methods become actions
  whose parameter types and doc comments are what the agent reads. Misuse fails to compile with a
  message that says how to fix it.
- **Nine behaviors** to embed with `#[behavior]`: `Press`, `Toggle`, `Field`, `Selection`, `List`
  (searchable through `query`), `RovingFocus` (lists and 2-D grids), `Typeahead`, `Popup` and
  `DragDrop`. Agents get direct actions, such as focusing an index or dropping from one item onto
  another, rather than key or pointer sequences.
- **The agent bridge.** `snapshot`, `diff`, `act` and `query` over a compact, line-oriented text
  format ([`FORMAT.md`](crates/rust-use-agent/FORMAT.md)): one line per component and only the
  actions available right now. An `act` reply carries what changed, so an agent never needs a
  follow-up snapshot. Failed steps name the step and list what is available. `allow` vetoes agent
  actions.
- **IPC** (feature `ipc`). A local socket, or a named pipe on Windows, that serves a running app to
  out-of-process agents in about 85 µs per fresh connection and snapshot, with app-specific commands
  and a request observer.
- **The `rust-use` CLI.** `snapshot`, `diff`, `act`, `query`, `raw` and `list` against a running
  app, keeping `diff` incremental across invocations, so any agent that can run a shell command
  can drive a rust-use app.
- **An MCP shim** (feature `mcp`, and `rust-use mcp`). The same three tools for any MCP client,
  with descriptions kept short because they are paid for on every request.
- **The GPUI adapter** (feature `gpui`). `ui.bind(cx)` re-renders a view whenever the `Ui` changes
  from any thread, and `handle.on_click(..)` sends GPUI input through the same actions an agent
  uses. Shaders compile at runtime by default, so building on macOS does not need Xcode's separate
  Metal Toolchain.
- **The chess demo and benchmark.** A GPUI chess app that Claude Code plays through generic UI
  automation, through the rust-use CLI and through rust-use over MCP, with live traffic counters,
  a wire replay, a Claude Code runner and a side-by-side race video
  ([`bench/`](bench/README.md)).

[0.1.0]: https://github.com/better-vault/rust-use/releases/tag/v0.1.0
