//! Registry and server lifetime for a single chess window.
use crate::{Board, Metrics, a11y};
use rust_use::prelude::*;

/// One shared game and its automation endpoint.
pub struct ChessApp {
    /// Registry observed by the window and native agent.
    pub ui: Ui,
    /// The only mounted component.
    pub board: Handle<Board>,
    /// Request counters for both interfaces.
    pub metrics: Metrics,
    server: Option<ServerGuard>,
}
impl Default for ChessApp {
    fn default() -> Self {
        Self::new()
    }
}
impl ChessApp {
    /// Create a game in the standard starting position.
    pub fn new() -> Self {
        let ui = Ui::new();
        let board = ui.mount(Board::default()).label("Chess board");
        Self {
            ui,
            board,
            metrics: Metrics::default(),
            server: None,
        }
    }
    /// Serve native requests and baseline commands on a named local socket.
    ///
    /// # Errors
    /// Returns an error for an invalid name or failure to start the listener.
    pub fn serve(&mut self, name: &str) -> Result<(), IpcError> {
        let board = self.board.clone();
        let metrics = self.metrics.clone();
        self.server = Some(
            Server::new(&self.ui)
                .endpoint(Endpoint::new(name)?)
                .command("a11y", move |args| a11y::handle(&board, args))
                .observe(move |log| metrics.observe(log))
                .spawn()?,
        );
        Ok(())
    }
}
