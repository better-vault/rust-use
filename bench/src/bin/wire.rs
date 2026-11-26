//! Compare payload traffic and wall time for complete Opera Game replays.
use chess_demo::games::OPERA_GAME;
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
    let date = utc_date(SystemTime::now());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("results");
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join(format!("wire-{date}.json")),
        serde_json::to_string_pretty(&json!({"variants":rows,"ratios":ratios}))? + "\n",
    )?;
    Ok(())
}

/// `YYYY-MM-DD` in UTC, from Howard Hinnant's days-to-civil algorithm, so the bench needs no
/// date crate for one file name.
fn utc_date(now: SystemTime) -> String {
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
