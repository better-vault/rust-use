#!/usr/bin/env python3
"""Aggregate Claude Code benchmark runs into medians per variant.

Reads bench/results/raw/<variant>-<n>.jsonl (Claude Code stream-json output), plus the
<variant>-<n>.run.json and <variant>-<n>.metrics.json files that run.sh writes next to each,
and prints a markdown table. Writes bench/results/claude-<date>.json.

Python standard library only.
"""

import argparse
import datetime
import json
import pathlib
import re
import statistics
import sys

VARIANTS = {
    "generic": "Generic UI automation (MCP)",
    "rust-use-cli": "rust-use (CLI)",
    "rust-use-mcp": "rust-use (MCP)",
}


def parse_session(path):
    """Return the measurements of one stream-json session."""
    tool_calls = 0
    result = None
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") == "assistant":
            content = event.get("message", {}).get("content", [])
            tool_calls += sum(1 for block in content if block.get("type") == "tool_use")
        elif event.get("type") == "result":
            result = event
    if result is None:
        return None
    usage = result.get("usage", {})
    fresh = usage.get("input_tokens", 0)
    cache_write = usage.get("cache_creation_input_tokens", 0)
    cache_read = usage.get("cache_read_input_tokens", 0)
    return {
        "tool_calls": tool_calls,
        "input_tokens": fresh + cache_write + cache_read,
        "input_fresh": fresh,
        "input_cache_write": cache_write,
        "input_cache_read": cache_read,
        "output_tokens": usage.get("output_tokens", 0),
        "duration_s": result.get("duration_ms", 0) / 1000,
        "cost_usd": result.get("total_cost_usd", 0.0),
        "turns": result.get("num_turns", 0),
        "is_error": bool(result.get("is_error", False)),
    }


def load_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def collect(raw):
    runs = {}
    for path in sorted(raw.glob("*.jsonl")):
        match = re.fullmatch(r"(.+)-(\d+)", path.stem)
        if not match or match.group(1) not in VARIANTS:
            continue
        session = parse_session(path)
        if session is None:
            continue
        stem = path.with_suffix("")
        run = load_json(stem.with_name(stem.name + ".run.json")) or {}
        session["verified"] = bool(run.get("verified")) and not session["is_error"]
        session["model"] = run.get("model")
        session["app"] = load_json(stem.with_name(stem.name + ".metrics.json"))
        runs.setdefault(match.group(1), []).append(session)
    return runs


def median(values):
    return statistics.median(values) if values else 0


def summarize(runs):
    summary = {}
    for variant, sessions in runs.items():
        keys = ["tool_calls", "input_tokens", "output_tokens", "duration_s", "cost_usd", "turns"]
        summary[variant] = {key: median([s[key] for s in sessions]) for key in keys}
        summary[variant]["runs"] = len(sessions)
        summary[variant]["verified"] = sum(s["verified"] for s in sessions)
    return summary


def fmt_int(value):
    return f"{round(value):,}"


def table(summary):
    rows = [
        "| | Tool calls | Input tokens (incl. cache) | Output tokens | Time | Cost | Verified |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for variant, label in VARIANTS.items():
        s = summary.get(variant)
        if not s:
            continue
        rows.append(
            f"| {label} | {fmt_int(s['tool_calls'])} | {fmt_int(s['input_tokens'])} | "
            f"{fmt_int(s['output_tokens'])} | {s['duration_s']:.0f} s | ${s['cost_usd']:.3f} | "
            f"{s['verified']}/{s['runs']} |"
        )
    return "\n".join(rows)


def main():
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--raw", type=pathlib.Path, default=here.parent / "results" / "raw")
    parser.add_argument("--out", type=pathlib.Path, default=here.parent / "results")
    parser.add_argument("--no-write", action="store_true", help="print the table only")
    args = parser.parse_args()

    runs = collect(args.raw)
    if not runs:
        print(f"no sessions found in {args.raw}", file=sys.stderr)
        return 1
    summary = summarize(runs)
    print(table(summary))
    if not args.no_write:
        date = datetime.date.today().isoformat()
        out = args.out / f"claude-{date}.json"
        out.write_text(json.dumps({"date": date, "summary": summary, "runs": runs}, indent=2) + "\n")
        print(f"\nwrote {out.relative_to(here.parent.parent)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
