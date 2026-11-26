#!/usr/bin/env bash
# Record Claude Code playing the Opera Game in the chess demo, once through generic UI automation
# over MCP and once through rust-use over MCP, then put the two runs side by side.
#
# Both runs are real Claude Code sessions, started the same way as bench/claude/run.sh. Each
# window's Status card shows the run timer, which starts when the counters are reset just before
# the session launches and stops at the final move. The two recordings start at that reset, so
# the side-by-side video is a fair race.
#
# Needs macOS (screencapture), ffmpeg, Claude Code and Screen Recording permission for the
# terminal. The chess window must stay visible and uncovered while recording.
#
#   bash bench/video/record.sh [--model sonnet] [--seconds 40]
set -euo pipefail

model=sonnet
seconds=40
while (($#)); do
  case "$1" in
    --model) model=$2; shift 2 ;;
    --seconds) seconds=$2; shift 2 ;;
    *) echo "usage: $0 [--model M] [--seconds N]" >&2; exit 2 ;;
  esac
done

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
out="$here/out"
mkdir -p "$out"
cd "$root"
cargo build --release -q -p chess-demo -p rust-use-cli -p chess-mcp-baseline
export PATH="$root/target/release:$PATH"
tmp=$(mktemp -d)
trap 'pkill -f "target/release/chess-demo" || true; rm -rf "$tmp"' EXIT

cat > "$tmp/window.swift" <<'EOF'
import CoreGraphics
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list where (w[kCGWindowOwnerName as String] as? String) == "chess-demo" {
  let b = w[kCGWindowBounds as String] as! [String: Any]
  if (b["Width"] as! Double) > 500 {
    print("\(Int(b["X"] as! Double)),\(Int(b["Y"] as! Double)),\(Int(b["Width"] as! Double)),\(Int(b["Height"] as! Double))")
    break
  }
}
EOF

prompt=$(cat bench/claude/prompt.md)
for cfg in mcp-baseline.json mcp-rust-use.json; do
  python3 - "$root" "bench/claude/$cfg" "$tmp/$cfg" <<'PY'
import json, pathlib, sys
root, src, dst = sys.argv[1:]
config = json.loads(pathlib.Path(src).read_text())
for server in config["mcpServers"].values():
    server["command"] = str(pathlib.Path(root) / server["command"])
pathlib.Path(dst).write_text(json.dumps(config))
PY
done

record() { # name banner config allowed interface
  local name=$1 banner=$2 cfg=$3 allowed=$4 interface=$5
  pkill -f "target/release/chess-demo" || true
  sleep 1
  RUST_USE_CHESS_BANNER="$banner" chess-demo >/dev/null 2>&1 &
  sleep 3
  local rect
  rect=$(swift "$tmp/window.swift")
  [[ -n $rect ]] || { echo "chess window not found" >&2; exit 1; }
  rust-use --app chess act board reset >/dev/null
  screencapture -v -V "$seconds" -R"$rect" "$out/$name.mov" &
  local rec=$!
  sleep 1
  rust-use --app chess raw metrics reset >/dev/null
  local work
  work=$(mktemp -d "$tmp/session.XXXX")
  (cd "$work" && claude -p "${prompt//\{\{INTERFACE\}\}/$interface}" \
    --output-format stream-json --verbose --no-session-persistence --setting-sources "" \
    --strict-mcp-config --mcp-config "$tmp/$cfg" --tools "" --allowedTools "$allowed" \
    --model "$model" --max-budget-usd 3) > "$out/$name.jsonl" || true
  wait "$rec"
  rust-use --app chess snapshot | grep -q "status=checkmate" \
    || echo "warning: $name did not finish the game" >&2
}

record generic "Generic UI automation · MCP" mcp-baseline.json 'mcp__ui-automation__*' \
  'Operate the app through the ui-automation tools.'
record rust-use "rust-use · MCP" mcp-rust-use.json 'mcp__rust-use__*' \
  'Operate the app through the rust-use tools.'

# Both recordings start one second before their reset; drop that second so t=0 is the start
# signal for both, then stack them side by side at 720 px tall.
ffmpeg -loglevel error -y -ss 1 -i "$out/generic.mov" -ss 1 -i "$out/rust-use.mov" \
  -filter_complex "[0:v]scale=-2:720,fps=30[a];[1:v]scale=-2:720,fps=30[b];[a][b]hstack=inputs=2,format=yuv420p[v]" \
  -map "[v]" -c:v libx264 -crf 22 -preset slow -movflags +faststart "$out/race.mp4"

# A GIF for the README: 12 fps, 960 px wide, one shared palette.
ffmpeg -loglevel error -y -i "$out/race.mp4" \
  -vf "fps=12,scale=960:-2:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=4" \
  "$out/race.gif"

ls -lh "$out"/race.*
