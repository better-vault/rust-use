//! Shared IPC fixtures for the scripted chess benchmarks.
use chess_demo::{ChessApp, games::OPERA_GAME, metrics::Traffic};
use rust_use::prelude::*;
use std::error::Error;

/// Fallible benchmark operation.
pub type BenchResult<T> = Result<T, Box<dyn Error>>;

/// Persistent client and a private app server. No window is needed.
pub struct Session {
    client: Client,
    /// Payload bytes observed by the client.
    pub traffic: Traffic,
    /// Game state, available for out-of-band verification.
    pub app: ChessApp,
}

impl Session {
    /// Open a fresh starting position on a process-specific endpoint.
    ///
    /// # Errors
    /// Returns socket setup errors.
    pub fn open() -> BenchResult<Self> {
        let mut app = ChessApp::new();
        let name = format!("bench-{}", std::process::id());
        app.serve(&name)?;
        Ok(Self {
            client: Client::connect(&Endpoint::new(&name)?)?,
            traffic: Traffic::default(),
            app,
        })
    }

    /// Exchange one payload and count its UTF-8 bytes.
    ///
    /// # Errors
    /// Returns transport and application errors.
    pub fn request(&mut self, command: &str) -> BenchResult<String> {
        let reply = self.client.request(command)?;
        self.traffic.calls += 1;
        self.traffic.request_bytes += command.len();
        self.traffic.reply_bytes += reply.len();
        if reply.starts_with("err")
            || (command.starts_with("a11y click ") && reply != r#"{"ok":true}"#)
        {
            return Err(format!("{command}: {reply}").into());
        }
        Ok(reply)
    }

    /// Play all plies using one action per move.
    ///
    /// # Errors
    /// Returns the first rejected request.
    pub fn native(&mut self) -> BenchResult<()> {
        for uci in OPERA_GAME {
            self.request(&format!("act board move {uci}"))?;
        }
        Ok(())
    }

    /// Read the current tree before every move and confirm each previous move via the move list.
    ///
    /// # Errors
    /// Returns an error for a missing element, missed move, or failed request.
    pub fn baseline(&mut self) -> BenchResult<()> {
        for (ply, uci) in OPERA_GAME.iter().enumerate() {
            let tree = self.tree_at(ply)?;
            let cells = children_by_role(&tree, "grid")?;
            for square in [&uci[..2], &uci[2..]] {
                let id = cells
                    .iter()
                    .find(|cell| cell["name"] == square)
                    .and_then(|cell| cell["id"].as_str())
                    .ok_or("missing square")?;
                self.request(&format!("a11y click {id}"))?;
            }
        }
        let final_tree = self.tree_at(OPERA_GAME.len())?;
        if !children_by_role(&final_tree, "list")?
            .last()
            .is_some_and(|row| row["name"] == "17. Rd8#")
        {
            return Err("missing final checkmate".into());
        }
        Ok(())
    }

    fn tree_at(&mut self, ply: usize) -> BenchResult<Value> {
        let tree = serde_json::from_str(&self.request("a11y tree")?)?;
        let moves = children_by_role(&tree, "list")?;
        let last_words = moves
            .last()
            .and_then(|row| row["name"].as_str())
            .map_or(0, |row| row.split_whitespace().count());
        if moves.len() != ply.div_ceil(2) || (ply > 0 && last_words != 3 - ply % 2) {
            return Err(format!("unexpected move history at ply {ply}").into());
        }
        Ok(tree)
    }
}

fn children_by_role<'a>(tree: &'a Value, role: &str) -> BenchResult<&'a Vec<Value>> {
    tree["children"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["role"] == role))
        .and_then(|node| node["children"].as_array())
        .ok_or_else(|| format!("missing {role}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_position_matches_recorded_game() -> BenchResult<()> {
        let app = ChessApp::new();
        let agent = app.ui.agent();
        let steps: Vec<_> = OPERA_GAME
            .iter()
            .map(|uci| format!("board move {uci}"))
            .collect();
        agent.act(steps.join("; "))?;
        let snapshot = agent.snapshot().to_string();
        assert!(snapshot.contains("status=checkmate"));
        assert!(
            snapshot.contains(include_str!("../opera-final.fen").trim()),
            "{snapshot}"
        );
        Ok(())
    }
}
