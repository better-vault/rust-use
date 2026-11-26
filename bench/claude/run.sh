#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 [--runs N] [--model M] [--variants generic,rust-use-cli,rust-use-mcp]"
}
runs=1
model=sonnet
variants=generic,rust-use-cli,rust-use-mcp
while (($#)); do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --runs|--model|--variants)
      [[ $# -ge 2 && -n $2 ]] || { usage >&2; exit 2; }
      case "$1" in
        --runs) runs=$2 ;;
        --model) model=$2 ;;
        --variants) variants=$2 ;;
      esac
      shift 2 ;;
    *) usage >&2; exit 2 ;;
  esac
done
[[ $runs =~ ^[1-9][0-9]*$ && $variants != *, ]] || { usage >&2; exit 2; }
IFS=, read -r -a selected <<< "$variants"
for i in "${!selected[@]}"; do
  case "${selected[i]}" in
    a|generic) selected[i]=generic ;;
    b|rust-use-cli) selected[i]=rust-use-cli ;;
    c|rust-use-mcp) selected[i]=rust-use-mcp ;;
    *) usage >&2; exit 2 ;;
  esac
done
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
raw="$root/bench/results/raw"
cd "$root"
cargo build --release -p chess-demo -p rust-use-cli -p chess-mcp-baseline
export PATH="$root/target/release:$PATH"
export RUST_USE_APP=chess
if ! rust-use list | grep -Fx chess >/dev/null; then
  echo "Start $root/target/release/chess-demo in another terminal, then retry." >&2
  exit 1
fi
command -v claude >/dev/null || { echo "Install Claude Code before running sessions." >&2; exit 1; }
mkdir -p "$raw"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
python3 - "$root" "$here" "$tmp" <<'PY'
import json, pathlib, sys
root, here, tmp = map(pathlib.Path, sys.argv[1:])
for template in here.glob("mcp-*.json"):
    config = json.loads(template.read_text())
    for server in config["mcpServers"].values():
        server["command"] = str(root / server["command"])
    (tmp / template.name).write_text(json.dumps(config))
PY
fen=$(cat "$here/../opera-final.fen")
failed=0
for variant in "${selected[@]}"; do
  case "$variant" in
    generic)
      cfg=baseline; tools=""; allowed='mcp__ui-automation__*'
      interface='Operate the app through the ui-automation tools.' ;;
    rust-use-cli)
      cfg=none; tools=Bash; allowed='Bash(rust-use:*)'
      interface='Operate the app with the `rust-use` command in the shell (start with `rust-use --help`).' ;;
    rust-use-mcp)
      cfg=rust-use; tools=""; allowed='mcp__rust-use__*'
      interface='Operate the app through the rust-use tools.' ;;
  esac
  prompt=$(cat "$here/prompt.md")
  prompt=${prompt//\{\{INTERFACE\}\}/$interface}
  n=0
  for ((run=1; run<=runs; run++)); do
    while :; do
      n=$((n+1))
      prefix="$raw/$variant-$n"
      [[ -e "$prefix.jsonl" ]] || break
    done
    work=$(mktemp -d "$tmp/session.XXXXXXXX")
    rust-use --app chess act board reset >/dev/null
    rust-use --app chess raw metrics reset >/dev/null
    rc=0
    (cd "$work" && claude -p "$prompt" --output-format stream-json --verbose --no-session-persistence \
      --setting-sources "" --strict-mcp-config --mcp-config "$tmp/mcp-$cfg.json" \
      --tools "$tools" --allowedTools "$allowed" --model "$model" --max-budget-usd 3) \
      > "$prefix.jsonl" || rc=$?
    # Capture counters before the verifier's snapshot adds a native request.
    rust-use --app chess raw metrics > "$prefix.metrics.json"
    snapshot=$(rust-use --app chess snapshot)
    success=false
    if [[ $snapshot == *status=checkmate* && $snapshot == *"=\"$fen\""* ]]; then
      success=true
    fi
    python3 - "$prefix" "$model" "$rc" "$success" "$snapshot" <<'PY'
import json, pathlib, sys
prefix, model, code, checkmate, snapshot = sys.argv[1:]
pathlib.Path(prefix + ".run.json").write_text(json.dumps(dict(
    model=model, exit_code=int(code), verified=checkmate=="true", snapshot=snapshot)))
PY
    [[ $success == true && $rc == 0 ]] || failed=1
  done
done
python3 "$here/aggregate.py"
exit "$failed"
