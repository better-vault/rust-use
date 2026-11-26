# Contributing to rust-use

Thanks for taking the time to contribute. This document covers how the repository is laid out,
what a change is expected to include, and how to get a pull request merged.

By participating you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

## Before you start

- **Bug fix?** Go ahead — open a [bug report](https://github.com/better-vault/rust-use/issues/new?template=bug_report.yml)
  and send the fix.
- **New behavior, or a change to an existing API?** Open an issue or a
  [discussion](https://github.com/better-vault/rust-use/discussions) first. rust-use aims at one
  small vocabulary an agent learns once and uses on every screen, and the design conversation is
  much cheaper before the code than after it.
- **Documentation or a typo?** Straight to a pull request.

## Getting set up

Requires **Rust 1.85+** (the toolchain file pins stable). The GPUI adapter and the chess demo
build on macOS out of the box; on Linux, GPUI needs the system packages listed in
[Zed's Linux guide](https://zed.dev/docs/development/linux).

```sh
git clone https://github.com/better-vault/rust-use.git
cd rust-use
cargo build
```

| Command | What it does |
| --- | --- |
| `cargo test --workspace --all-features` | Run every test, including the macro UI tests |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lint, warnings as errors |
| `cargo fmt --all` | Format |
| `cargo doc --workspace --no-deps --all-features --open` | Build and open the API docs |
| `cargo run -p chess-demo` | Start the GPUI chess demo |
| `cargo bench -p rust-use-bench` | Run the scripted micro-benchmark |

Linux and Windows contributors without GPUI's system dependencies can leave the adapter out:
`cargo test --workspace --exclude rust-use-gpui`.

### Where things live

```
crates/rust-use         the facade — what `cargo add rust-use` installs; re-exports and features
crates/rust-use-core    state, change events, the Primitive trait and the behaviors
crates/rust-use-macros  #[derive(Primitive)] and #[actions]
crates/rust-use-agent   the agent bridge: snapshot, act, query, permissions, the compact format
crates/rust-use-gpui    the GPUI adapter
crates/rust-use-cli     the `rust-use` binary: IPC client and the MCP shim
demos/chess             GPUI chess app used for the benchmark
bench                   micro-benchmark, Claude Code runner and published results
```

## Conventions

These are not style preferences; CI enforces most of them.

**Users import one crate.** Everything public is reachable from `rust_use::…`, and the things
nearly every program needs are in `rust_use::prelude`. A new public type that is not re-exported
from the facade is a bug.

**Every public item is documented.** `missing_docs` is on. A doc comment says what the item is for
and, where it is not obvious, carries an example that compiles as a doctest.

**No panics in library code.** `unwrap` and `expect` are linted. Errors are typed with
`thiserror` and returned; a panic is only acceptable for a broken internal invariant, and then it
says which one.

**What an agent sees is part of the API.** The compact snapshot format is specified in
`crates/rust-use-agent/FORMAT.md` and guarded by size tests. A change that makes a snapshot
larger needs a reason, because every byte is paid for in tokens on every call.

**Changes made by an agent are marked.** Behaviors report `ChangeSource::Agent` for anything that
arrived through the bridge, and every such change passes through the `allow` hook first. Do not
add a path around it.

**Features stay additive.** Enabling a feature never removes or changes an API; the crate builds
and its tests pass with `--no-default-features`.

**No `unsafe`.** It is forbidden workspace-wide.

## Commits and pull requests

[Conventional Commits](https://www.conventionalcommits.org/), scoped with the crate or area
where it helps:

```
feat(core): add a 2-D grid mode to roving focus
fix(agent): reject act steps whose ref left the page
docs(readme): show the benchmark table
```

`feat`, `fix`, `docs`, `refactor`, `chore`, `test`, `perf`, `build`, `ci`.

Keep a pull request to one logical change. Before you open it:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

CI runs these on Linux, macOS and Windows for every pull request. Fill in the pull request
template — the checklist is short and it is what the review is against.

Do **not** bump the version in `Cargo.toml` in a pull request; releases are cut separately.

## Reporting a security issue

Please do not open a public issue. Use the repository's **Security** tab → *Report a
vulnerability*, which is private and visible only to the maintainers. See
[SECURITY.md](SECURITY.md).

## Licence

By contributing you agree that your contributions will be licensed under the
[MIT Licence](LICENSE).
