//! Compare payload traffic and wall time for complete Opera Game replays.
use chess_demo::games::OPERA_GAME;
use chrono::{DateTime, Utc};
use rust_use_bench::{BenchResult, Session};
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{Instant, SystemTime},
};

fn main() -> BenchResult<()> {
    let mut rows = Vec::new();
    let mut measurements = Vec::new();
    println!("| Variant | Requests | Request bytes | Reply bytes | ≈tokens | Wall time (ms) |");
    println!("|---|---:|---:|---:|---:|---:|");
    for variant in [
        "rust-use (act per move)",
        "rust-use (one batched act)",
        "generic automation",
    ] {
        let mut session = Session::open()?;
        let start = Instant::now();
        if variant == "generic automation" {
            session.baseline()?;
        } else {
            session.request("snapshot")?;
            if variant == "rust-use (act per move)" {
                session.native()?;
            } else {
                let steps: Vec<_> = OPERA_GAME
                    .iter()
                    .map(|uci| format!("board move {uci}"))
                    .collect();
                session.request(&format!("act {}", steps.join("; ")))?;
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        if !session
            .app
            .board
            .read(|b| b.fen() == include_str!("../../opera-final.fen").trim())
            .unwrap_or(false)
        {
            return Err("wrong final position".into());
        }
        let t = session.traffic;
        let tokens = t.estimated_tokens();
        println!(
            "| {variant} | {} | {} | {} | {tokens} | {ms:.3} |",
            t.calls, t.request_bytes, t.reply_bytes
        );
        let values = json!({"requests":t.calls, "request_bytes":t.request_bytes, "reply_bytes":t.reply_bytes, "estimated_tokens":tokens,"wall_ms":ms});
        rows.push(json!({"variant":variant,"measurements":values}));
        measurements.push(values);
    }
    let mut ratios = Vec::new();
    for (i, label) in ["act per move", "batched act"].iter().enumerate() {
        let mut ratios_row = json!({});
        print!("| generic / {label} |");
        for field in [
            "requests",
            "request_bytes",
            "reply_bytes",
            "estimated_tokens",
            "wall_ms",
        ] {
            let ratio = measurements[2][field]
                .as_f64()
                .ok_or("missing measurement")?
                / measurements[i][field]
                    .as_f64()
                    .ok_or("missing measurement")?;
            print!(" {ratio:.2}× |");
            ratios_row[field] = json!(ratio);
        }
        println!();
        ratios.push(json!({"generic_over":label,"ratios":ratios_row}));
    }
    let date = DateTime::<Utc>::from(SystemTime::now()).date_naive();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("results");
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join(format!("wire-{date}.json")),
        serde_json::to_string_pretty(&json!({"variants":rows,"ratios":ratios}))? + "\n",
    )?;
    Ok(())
}
