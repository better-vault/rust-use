//! Compact accessibility tree and element clicks for generic automation.
//! Grid cells keep the same ids across moves and resets, starting with n6 at a8.
use crate::Board;
use crate::board::Square;
use rust_use::prelude::*;
use serde_json::json;

const FIRST_CELL: u32 = 6;

fn tree(board: &Board) -> Value {
    let cells: Vec<_> = Square::ALL
        .into_iter()
        .map(|index| {
            let square = index.flip_rank();
            let id = FIRST_CELL + index as u32;
            let value = board
                .piece_at(square)
                .map_or_else(String::new, crate::board::Piece::name);
            let x = 64 * index.file() as u32;
            let y = 64 * index.rank() as u32;
            json!({
                "id": format!("n{id}"),
                "role": "gridcell",
                "name": square.to_string(),
                "value": value,
                "selected": board.selected() == Some(square),
                "bounds": [x, y, 64, 64],
            })
        })
        .collect();
    let moves: Vec<_> = board
        .history_san()
        .chunks(2)
        .enumerate()
        .map(|(i, pair)| json!({"role":"listitem","name":format!("{}. {}",i+1,pair.join(" "))}))
        .collect();
    json!({"role":"window","name":"rust-use chess","children":[
        {"id":"n1","role":"toolbar","children":[{"id":"n2","role":"button","name":"New game"},
        {"id":"n3","role":"button","name":"Undo","disabled":!board.can_undo()}]},
        {"id":"n4","role":"status","name":board.status_text()},
        {"id":"n5","role":"grid","name":"Chess board","children":cells},
        {"id":"n70","role":"list","name":"Moves","children":moves}
    ]})
}

fn click(board: &Handle<Board>, id: &str) -> Result<(), String> {
    match id {
        "n2" | "n3" => board
            .invoke(
                if id == "n2" { "reset" } else { "undo" },
                (),
                ChangeSource::User,
            )
            .map(|_| ())
            .map_err(|e| e.to_string()),
        _ => {
            let number = id
                .strip_prefix('n')
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| (FIRST_CELL..FIRST_CELL + 64).contains(n) && id == format!("n{n}"))
                .ok_or_else(|| format!("no clickable element {id}"))?;
            let square = Square::index((number - FIRST_CELL) as usize).flip_rank();
            board
                .update(ChangeSource::User, |b| b.click(square))
                .map(|_| ())
                .ok_or_else(|| "board unavailable".into())
        }
    }
}

/// Handle the text following the IPC command word `a11y` and return one JSON payload.
pub fn handle(board: &Handle<Board>, args: &str) -> String {
    let mut words = args.split_whitespace();
    let reply = match (words.next(), words.next()) {
        (Some("tree"), None) => board.read(tree).ok_or_else(|| "board unavailable".into()),
        (Some("click"), Some(id)) if words.next().is_none() => {
            click(board, id).map(|()| json!({"ok":true}))
        }
        (Some("type"), _) => Err("no focused text field".into()),
        _ => Err("expected tree, click <id>, or type <text>".into()),
    };
    reply
        .unwrap_or_else(|e: String| json!({"ok":false,"error":e}))
        .to_string()
}
