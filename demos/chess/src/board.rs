//! Standard chess rules shared by the native bridge and click interface.
use rust_use::prelude::*;
use shakmaty::{
    Chess, EnPassantMode, Move, Piece, Position, Role, Square, fen::Fen, san::SanPlus, uci::UciMove,
};

/// Effect of a square click.
#[derive(Debug, PartialEq, Eq)]
pub enum ClickOutcome {
    /// A friendly piece is selected.
    Selected(Square),
    /// No piece is selected.
    Deselected,
    /// The selected piece moved, yielding this SAN notation.
    Played(String),
}

/// A chess position, its reversible move history and the human player's selection.
#[derive(Primitive)]
#[primitive(name = "board")]
pub struct Board {
    #[agent(value)]
    fen: String,
    turn: char,
    #[agent(flag)]
    check: bool,
    last: Option<String>,
    status: Option<&'static str>,
    #[agent(skip)]
    position: Chess,
    #[agent(skip)]
    history: Vec<(Chess, String, String)>,
    #[agent(skip)]
    selected: Option<Square>,
}

impl Default for Board {
    fn default() -> Self {
        let position = Chess::default();
        Self {
            fen: Fen::from_position(&position, EnPassantMode::Legal).to_string(),
            turn: 'w',
            check: false,
            last: None,
            status: None,
            position,
            history: Vec::new(),
            selected: None,
        }
    }
}

#[actions]
impl Board {
    /// Play a move in UCI notation, e.g. e2e4 or e7e8q.
    #[action(name = "move", enabled = Self::playing)]
    fn play(&mut self, uci: String) -> Result<String, ActionError> {
        let legal = self.position.legal_moves();
        if let Ok(parsed) = uci.parse::<UciMove>()
            && let Some(m) = legal.iter().find(|&&m| UciMove::from_standard(m) == parsed)
        {
            return Ok(self.apply(*m, uci));
        }
        let mut moves: Vec<_> = legal
            .into_iter()
            .map(|m| UciMove::from_standard(m).to_string())
            .collect();
        moves.sort_unstable();
        Err(ActionError::Rejected(format!(
            "illegal move {uci} (legal: {})",
            moves.join(" ")
        )))
    }
    /// Take back the last move.
    #[action(enabled = Self::can_undo)]
    fn undo(&mut self) {
        if let Some((previous, _, _)) = self.history.pop() {
            self.position = previous;
            self.sync();
        }
    }
    /// Start a new game.
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Board {
    fn playing(&self) -> bool {
        self.status.is_none()
    }
    /// Whether there is a move to take back.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
    /// Current position in FEN notation.
    pub fn fen(&self) -> &str {
        &self.fen
    }
    /// Current status text shared by the window and automation tree.
    pub fn status_text(&self) -> String {
        let side = if self.turn == 'w' { "White" } else { "Black" };
        match self.status {
            Some("checkmate") => format!("{side} checkmated"),
            Some("stalemate") => "Stalemate".into(),
            Some(_) => "Draw".into(),
            None if self.check => format!("{side} to move (check)"),
            None => format!("{side} to move"),
        }
    }
    /// Piece occupying a square.
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.position.board().piece_at(sq)
    }
    /// Square selected by the human player.
    pub fn selected(&self) -> Option<Square> {
        self.selected
    }
    /// Board destinations for clicks, including the king's castling destination.
    pub fn legal_targets(&self, from: Square) -> Vec<Square> {
        if !self.playing() {
            return Vec::new();
        }
        self.position
            .legal_moves()
            .into_iter()
            .map(UciMove::from_standard)
            .filter(|m| m.from() == Some(from) && m.promotion().is_none_or(|p| p == Role::Queen))
            .filter_map(UciMove::to)
            .collect()
    }
    /// Played moves in standard algebraic notation, one entry per ply.
    pub fn history_san(&self) -> Vec<String> {
        self.history.iter().map(|(_, _, san)| san.clone()).collect()
    }
    /// Select a friendly piece or play its selected move. Promotions default to a queen.
    pub fn click(&mut self, sq: Square) -> ClickOutcome {
        if self.playing() {
            if let Some(from) = self.selected
                && let Some(m) = self.position.legal_moves().into_iter().find(|&m| {
                    let u = UciMove::from_standard(m);
                    u.from() == Some(from)
                        && u.to() == Some(sq)
                        && u.promotion().is_none_or(|p| p == Role::Queen)
                })
            {
                return ClickOutcome::Played(self.apply(m, UciMove::from_standard(m).to_string()));
            }
            if self.selected != Some(sq)
                && self
                    .piece_at(sq)
                    .is_some_and(|p| p.color == self.position.turn())
            {
                self.selected = Some(sq);
                return ClickOutcome::Selected(sq);
            }
        }
        self.selected = None;
        ClickOutcome::Deselected
    }
    fn apply(&mut self, m: Move, uci: String) -> String {
        let previous = self.position.clone();
        let san = SanPlus::from_move_and_play_unchecked(&mut self.position, m).to_string();
        self.history.push((previous, uci, san.clone()));
        self.sync();
        san
    }
    fn sync(&mut self) {
        self.selected = None;
        self.fen = Fen::from_position(&self.position, EnPassantMode::Legal).to_string();
        self.turn = self.position.turn().char();
        self.check = self.position.is_check();
        self.last = self.history.last().map(|(_, uci, _)| uci.clone());
        // With no claim-draw control, only automatic draws end the game.
        self.status = if self.position.is_checkmate() {
            Some("checkmate")
        } else if self.position.is_stalemate() {
            Some("stalemate")
        } else if self.position.is_insufficient_material()
            || self.position.halfmoves() >= 150
            || self
                .history
                .iter()
                .filter(|(pos, _, _)| pos == &self.position)
                .nth(3)
                .is_some()
        {
            Some("draw")
        } else {
            None
        };
    }
}
