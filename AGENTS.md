# rust-use — notes for coding agents

rust-use is a Rust library of UI behaviors that agents use to build and drive interfaces, GPUI
first. The promise it has to keep: an agent driving a screen through rust-use spends fewer tool
calls, fewer tokens and less time than one driving the same screen through generic MCP
automation. Every design decision is weighed against that.

## Layout

| Path | What it is |
| --- | --- |
| `crates/rust-use` | Facade. The only crate users add. Re-exports, features, `prelude`. |
| `crates/rust-use-core` | Store, change events, `Primitive`, `Ui` registry, behaviors. |
| `crates/rust-use-macros` | `#[derive(Primitive)]`, `#[actions]`. |
| `crates/rust-use-agent` | Bridge: `snapshot`/`diff`/`act`/`query`, `allow` hook, compact format (`FORMAT.md`). |
| `crates/rust-use-gpui` | GPUI adapter. Builds on macOS; Linux needs system packages. |
| `crates/rust-use-cli` | `rust-use` binary: IPC client, MCP shim. |
| `demos/chess` | GPUI chess app; three driving variants for the benchmark. |
| `bench/` | Micro-bench, `run-claude.sh`, `results/`. |
| `.codex/tasks/` | Task briefs given to implementers, one per task, kept as a record. |

## The gate — run all of it before calling anything done

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --all-features
```

## Code rules

- Users import `rust_use::prelude::*` and nothing else for the common path. Re-export every
  public type from the facade.
- `missing_docs`, clippy `pedantic`, `unwrap_used`/`expect_used` are on. No `unsafe`.
- Errors are `thiserror` enums, returned, never panicked.
- Agent-originated changes carry `ChangeSource::Agent` and always pass the `allow` hook.
- Snapshot bytes are tokens: changes to the compact format update `FORMAT.md` and the size tests.
- Features are additive; `--no-default-features` must build and test.

## Git & release

- **Every task gets its own commit**, Conventional Commits (`feat(core): …`). Never batch
  unrelated work; never leave finished work uncommitted.
- **Push at the end of each milestone**, once the whole gate is green.
- **Never cut a release without asking the user first** — every time, with the proposed version
  and a "no release yet" option. Pre-1.0: minor for new behaviors or API, patch for fixes.
- Release steps once approved: bump `workspace.package.version` → `CHANGELOG.md` entry (Keep a
  Changelog), committed with the bump → push → tag `vX.Y.Z` → GitHub release from that entry.
  `.github/workflows/publish.yml` publishes to crates.io from the release; nobody runs
  `cargo publish` by hand after 0.1.0.
- Do not bump versions in feature work.
