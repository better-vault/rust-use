//! Criterion comparisons on a private, persistent chess server.
use criterion::{Criterion, criterion_main};
use rust_use::prelude::*;
use rust_use_bench::{BenchResult, Session};
use std::time::{Duration, Instant};

fn checked<T>(result: BenchResult<T>) -> T {
    result.unwrap_or_else(|error| {
        eprintln!("benchmark failed: {error}");
        std::process::exit(1)
    })
}

// Restore the initial position outside the timed single-operation samples.
fn sample<T>(
    iterations: u64,
    mut operation: impl FnMut() -> BenchResult<(T, Duration)>,
) -> Duration {
    (0..iterations)
        .map(|_| {
            let (value, elapsed) = checked(operation());
            std::hint::black_box(value);
            elapsed
        })
        .sum()
}

fn transport(c: &mut Criterion) {
    let mut s = checked(Session::open());
    let agent = s.app.ui.agent();
    c.bench_function("in_process/snapshot", |b| b.iter(|| agent.snapshot()));
    c.bench_function("in_process/act_move", |b| {
        b.iter_custom(|n| {
            sample(n, || {
                agent.act("board reset")?;
                let start = Instant::now();
                Ok((agent.act("board move e2e4")?, start.elapsed()))
            })
        });
    });
    for (name, command) in [
        ("ipc/snapshot", "snapshot"),
        ("baseline_ipc/tree", "a11y tree"),
    ] {
        checked(s.request("act board reset"));
        c.bench_function(name, |b| b.iter(|| checked(s.request(command))));
    }
    for (name, command) in [
        ("ipc/act_move", "act board move e2e4"),
        ("baseline_ipc/click", "a11y click n58"),
    ] {
        c.bench_function(name, |b| {
            b.iter_custom(|n| {
                sample(n, || {
                    s.request("act board reset")?;
                    let start = Instant::now();
                    Ok((s.request(command)?, start.elapsed()))
                })
            });
        });
    }
    for (name, replay) in [
        (
            "replay/opera_native",
            Session::native as fn(&mut Session) -> BenchResult<()>,
        ),
        ("replay/opera_baseline", Session::baseline),
    ] {
        c.bench_function(name, |b| {
            b.iter(|| {
                checked(s.request("act board reset"));
                checked(replay(&mut s));
            });
        });
    }
}

fn benches() {
    transport(&mut Criterion::default().configure_from_args());
}
criterion_main!(benches);
