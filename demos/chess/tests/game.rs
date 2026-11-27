//! Exercise the same mounted board through the two benchmark interfaces.
use chess_demo::{Board, ChessApp, a11y, games::OPERA_GAME, metrics::Transport};
use rust_use::prelude::*;
use serde_json::json;
use std::{
    error::Error,
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn atomic_moves_restore_the_start_position() {
    let app = ChessApp::new();
    let agent = app.ui.agent();
    let start = agent.snapshot().to_string();
    let reply = agent.handle("act atomic; board move e2e4; board move e2e5");
    assert!(reply.starts_with("err 2 board move e2e5: illegal move"));
    assert_eq!(reply.lines().nth(1), Some("rolled back"));
    assert_eq!(reply.lines().nth(3), start.lines().nth(1));
    assert_eq!(
        app.board.read(|board| board.fen().to_owned()),
        Some(Board::default().fen().to_owned())
    );
    assert_eq!(app.board.read(Board::can_undo), Some(false));
}

#[test]
fn starting_snapshot_and_undo() -> TestResult {
    let app = ChessApp::new();
    let agent = app.ui.agent();
    let start = agent.snapshot().to_string();
    assert_eq!(
        start,
        "rev 2\n@1 board \"Chess board\" =\"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1\" turn=w !move(uci) !reset"
    );
    assert!(agent.act("board undo").is_err());
    let err = agent
        .act("board move e2e5")
        .err()
        .ok_or("accepted illegal move")?;
    assert!(err.to_string().contains("illegal move e2e5 (legal: a2a3 a2a4 b1a3 b1c3 b2b3 b2b4 c2c3 c2c4 d2d3 d2d4 e2e3 e2e4 f2f3 f2f4 g1f3 g1h3 g2g3 g2g4 h2h3 h2h4)"));
    let outcome = agent.act("board move e2e4")?;
    assert_eq!(outcome.values, [(1, json!("e4"))]);
    assert!(agent.snapshot().to_string().contains("!undo"));
    agent.act("board undo")?;
    assert_eq!(
        agent.snapshot().to_string().lines().nth(1),
        start.lines().nth(1)
    );
    Ok(())
}

#[test]
fn opera_game_on_both_interfaces() -> TestResult {
    let native = ChessApp::new();
    let agent = native.ui.agent();
    let baseline = ChessApp::new();
    for &uci in OPERA_GAME {
        let tree: Value = serde_json::from_str(&a11y::handle(&baseline.board, "tree"))?;
        let grid = tree["children"]
            .as_array()
            .and_then(|nodes| nodes.iter().find(|n| n["role"] == "grid"))
            .ok_or("missing grid")?;
        let cells = grid["children"].as_array().ok_or("missing cells")?;
        assert_eq!(cells.len(), 64);
        for name in [&uci[..2], &uci[2..4]] {
            let id = cells
                .iter()
                .find(|c| c["name"] == name)
                .and_then(|c| c["id"].as_str())
                .ok_or("missing cell id")?;
            assert_eq!(
                a11y::handle(&baseline.board, &format!("click {id}")),
                r#"{"ok":true}"#
            );
        }
        agent.act(format!("board move {uci}"))?;
        let fen = |b: &Board| b.fen().to_owned();
        assert_eq!(baseline.board.read(fen), native.board.read(fen), "{uci}");
    }
    let snapshot = agent.snapshot().to_string();
    assert!(snapshot.contains("status=checkmate"));
    assert!(!snapshot.contains("!move"));
    assert!(agent.act("board move a7a6").is_err());
    let tree: Value = serde_json::from_str(&a11y::handle(&baseline.board, "tree"))?;
    assert_eq!(tree["children"][3]["children"][16]["name"], "17. Rd8#");
    Ok(())
}

#[test]
fn ipc_accounts_for_each_payload() -> TestResult {
    let name = format!("chess-test-{}", std::process::id());
    let mut app = ChessApp::new();
    app.serve(&name)?;
    let mut client = Client::connect(&Endpoint::new(&name)?)?;
    let act = "act board move e2e4";
    let reply = client.request(act)?;
    assert!(reply.starts_with("ok\n=> 1 e4\n"));
    let tree = client.request("a11y tree")?;
    let json: Value = serde_json::from_str(&tree)?;
    assert_eq!(json["children"][1]["name"], "Black to move");
    // The observer runs after writing, so the client can receive the tree first.
    let start = Instant::now();
    loop {
        let view = app.metrics.snapshot();
        if view.baseline.calls > 0 {
            assert_eq!(view.last, Some(Transport::Baseline));
            for (traffic, req, res) in [
                (view.native, act, &reply),
                (view.baseline, "a11y tree", &tree),
            ] {
                assert_eq!(traffic.calls, 1);
                assert_eq!(
                    (traffic.request_bytes, traffic.reply_bytes),
                    (req.len(), res.len())
                );
                assert_eq!(
                    traffic.estimated_tokens(),
                    (req.len() + res.len()).div_ceil(4)
                );
                assert_eq!(traffic.mean_latency(), traffic.total_latency);
            }
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "observer timed out"
        );
        std::thread::yield_now();
    }
    app.metrics.reset();
    assert_eq!(app.metrics.snapshot().last, None);
    Ok(())
}

#[test]
fn cell_geometry_selection_and_toolbar() -> TestResult {
    let app = ChessApp::new();
    let command = |args| a11y::handle(&app.board, args);
    let read = || serde_json::from_str::<Value>(&command("tree"));
    let start = read()?;
    let cells = &start["children"][2]["children"];
    assert_eq!(
        cells[0],
        json!({"id":"n6","role":"gridcell","name":"a8","value":"black rook","selected":false,"bounds":[0,0,64,64]})
    );
    assert_eq!(cells[63]["bounds"], json!([448, 448, 64, 64]));
    assert_eq!(cells[36]["value"], "");
    assert_eq!(start["children"][0]["children"][1]["disabled"], true);
    let ok = r#"{"ok":true}"#;
    assert_eq!(command("click n58"), ok); // e2
    assert_eq!(read()?["children"][2]["children"][52]["selected"], true);
    assert_eq!(command("click n42"), ok); // e4
    assert_eq!(command("click n3"), ok);
    assert_eq!(read()?, start);
    assert_eq!(command("click n58"), ok);
    assert_eq!(command("click n2"), ok);
    assert_eq!(read()?, start);
    let typed: Value = serde_json::from_str(&command("type e2e4"))?;
    assert_eq!(
        typed,
        json!({"ok": false, "error": "no focused text field"})
    );
    Ok(())
}
