# Benchmark

Does an agent drive a UI with fewer calls, fewer tokens and less time through rust-use than
through generic UI automation over MCP? These measurements answer that on one real task: playing
a full chess game in a running GPUI app.

There are two layers. The **wire replay** is deterministic and has no model in it: it measures
exactly what each interface puts on the wire for the same game. The **Claude Code runs** put a real
agent on each interface and measure what the session actually cost.

## The task

The [chess demo](../demos/chess) is open. The agent must play the
[Opera Game](https://en.wikipedia.org/wiki/Opera_Game) (Morphy vs Duke Karl / Count Isouard,
Paris 1858) for both sides, all 33 plies, through the app, ending in 17.Rd8#. A run counts only
if the app's board ends in checkmate on the expected final position
([`opera-final.fen`](opera-final.fen)).

## The interfaces

All three drive **the same running app and the same board**.

| Variant | Tools the agent gets | A move looks like |
|---|---|---|
| **Generic UI automation** | MCP server [`chess-mcp-baseline`](../demos/chess-mcp-baseline): `get_ui_tree`, `click(element_id)`, `type_text` | read the tree, click the from-square, click the to-square |
| **rust-use (CLI)** | Bash, limited to the `rust-use` command, over the local socket | `rust-use act "board move e2e4"` |
| **rust-use (MCP)** | The rust-use MCP shim (`rust-use mcp`): `snapshot`, `act`, `query` | `act {"script": "board move e2e4"}` |

The baseline is modelled on how accessibility- and browser-automation MCP servers expose an app.
It sees what is on screen (64 grid cells with their names, contents and bounds, the toolbar, the
status line and the move list) and clicks elements by id.

### Fairness

- One app, one board, one model, one prompt. The prompt differs only in the line that names the
  interface ([`claude/prompt.md`](claude/prompt.md)).
- The baseline's tree is compact single-line JSON, not pretty-printed, with no padding fields.
  Its click tool reports success or failure like real click tools, without returning the tree.
- Tool descriptions are short on both sides. The baseline's are shorter than ours.
- Sessions start in an empty directory with no settings, no CLAUDE.md and no other MCP servers
  (`--setting-sources "" --strict-mcp-config`). The only tools available are the ones listed above.
- rust-use (MCP) isolates the interface from the transport: same MCP framing as the baseline,
  different vocabulary.

## Wire replay

```sh
cargo run --release -p rust-use-bench --bin wire
```

This replays the game over the app's IPC socket the way each interface must. Generic automation
reads the tree each move to find element ids and confirm the result, then makes two clicks.
rust-use sends one `act` per move, or one batched `act` for the whole game. Results go to
`results/wire-<date>.json`.

`cargo bench -p rust-use-bench` adds criterion micro-benchmarks for individual calls.

## Claude Code runs

```sh
cargo build --release -p chess-demo -p rust-use-cli -p chess-mcp-baseline
./target/release/chess-demo &                 # the app must be running
bash bench/claude/run.sh --runs 3 --model sonnet
```

`run.sh` resets the board and the app's traffic counters before each session and runs Claude Code
headless with `--output-format stream-json`. It then verifies the final position.
[`claude/aggregate.py`](claude/aggregate.py) reads every session's `result` event (tokens including
cache, duration, cost, turns) and counts its `tool_use` blocks. It writes the medians to
`results/claude-<date>.json`. Raw transcripts stay in `results/raw/`, which is not committed.

## The race video

```sh
bash bench/video/record.sh            # needs macOS, ffmpeg, and the screen for ~1.5 minutes
```

This records the chess window during one real Claude Code session per interface, generic UI
automation over MCP and rust-use over MCP. Each window's run timer starts when the counters are
reset just before the session launches. The two recordings are then stacked side by side from that
shared start, so the video in the README is a real-time race. Output goes to `bench/video/out/`.

## Results

See [`results/`](results) and the table in the [README](../README.md#benchmark). Numbers are
self-reported and come from one machine. The commands above reproduce them.
