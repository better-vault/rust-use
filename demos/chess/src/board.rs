//! Standard chess rules shared by the native bridge and click interface.
//!
//! Rules come from `cozy-chess` (MIT). It has no SAN output, so a small `san` function formats moves for the
//! move list and for `move` replies.
use cozy_chess::util::{display_uci_move, parse_uci_move};
use cozy_chess::{Board as Position, Move};
pub use cozy_chess::{Color, File, Piece as Role, Rank, Square};
use rust_use::prelude::*;

/// A piece on the board: its colour and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// Side the piece belongs to.
    pub color: Color,
    /// Kind of piece.
    pub role: Role,
}

impl Piece {
    /// `w` or `b`, as used in piece asset names.
    pub fn color_char(self) -> char {
        match self.color {
            Color::White => 'w',
            Color::Black => 'b',
        }
    }

    /// Upper-case piece letter: `K Q R B N P`.
    pub fn role_char(self) -> char {
        char::from(self.role).to_ascii_uppercase()
    }

    /// Spoken name, e.g. `white pawn`, as an accessibility tree would show it.
    pub fn name(self) -> String {
        let color = match self.color {
            Color::White => "white",
            Color::Black => "black",
        };
        let role = match self.role {
            Role::Pawn => "pawn",
            Role::Knight => "knight",
            Role::Bishop => "bishop",
            Role::Rook => "rook",
            Role::Queen => "queen",
            Role::King => "king",
        };
        format!("{color} {role}")
    }
}

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
    position: Position,
    #[agent(skip)]
    history: Vec<(Position, String, String)>,
    #[agent(skip)]
    selected: Option<Square>,
}

impl Default for Board {
    fn default() -> Self {
        let position = Position::default();
        Self {
            fen: position.to_string(),
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
        if let Ok(m) = parse_uci_move(&self.position, &uci)
            && self.position.is_legal(m)
        {
            return Ok(self.apply(m, uci));
        }
        let mut moves: Vec<_> = legal_moves(&self.position)
            .into_iter()
            .map(|m| display_uci_move(&self.position, m).to_string())
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

    /// Whether the game is still in progress.
    pub fn can_play(&self) -> bool {
        self.playing()
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
        Some(Piece {
            color: self.position.color_on(sq)?,
            role: self.position.piece_on(sq)?,
        })
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
        legal_moves(&self.position)
            .into_iter()
            .filter(|m| m.from == from && m.promotion.is_none_or(|p| p == Role::Queen))
            .map(|m| self.click_target(m))
            .collect()
    }

    /// Origin and destination of the last move, in board coordinates.
    pub fn last_move(&self) -> Option<(Square, Square)> {
        let uci = self.last.as_ref()?;
        Some((uci.get(0..2)?.parse().ok()?, uci.get(2..4)?.parse().ok()?))
    }

    /// Square of the king under attack.
    pub fn checked_king(&self) -> Option<Square> {
        self.check
            .then(|| self.position.king(self.position.side_to_move()))
    }

    /// Played moves in standard algebraic notation, one entry per ply.
    pub fn history_san(&self) -> Vec<String> {
        self.history.iter().map(|(_, _, san)| san.clone()).collect()
    }

    /// Select a friendly piece or play its selected move. Promotions default to a queen.
    pub fn click(&mut self, sq: Square) -> ClickOutcome {
        if self.playing() {
            if let Some(from) = self.selected
                && let Some(m) = legal_moves(&self.position).into_iter().find(|&m| {
                    m.from == from
                        && self.click_target(m) == sq
                        && m.promotion.is_none_or(|p| p == Role::Queen)
                })
            {
                let uci = display_uci_move(&self.position, m).to_string();
                return ClickOutcome::Played(self.apply(m, uci));
            }
            if self.selected != Some(sq)
                && self
                    .piece_at(sq)
                    .is_some_and(|p| p.color == self.position.side_to_move())
            {
                self.selected = Some(sq);
                return ClickOutcome::Selected(sq);
            }
        }
        self.selected = None;
        ClickOutcome::Deselected
    }

    /// The square a human clicks to make `m`: for castling, the king's destination rather than
    /// the rook square cozy-chess encodes it as.
    fn click_target(&self, m: Move) -> Square {
        let uci = display_uci_move(&self.position, m).to_string();
        uci.get(2..4).and_then(|s| s.parse().ok()).unwrap_or(m.to)
    }

    fn apply(&mut self, m: Move, uci: String) -> String {
        let previous = self.position.clone();
        let san = san(&previous, m);
        self.position.play_unchecked(m);
        let san = match outcome(&self.position) {
            Some("checkmate") => format!("{san}#"),
            _ if !self.position.checkers().is_empty() => format!("{san}+"),
            _ => san,
        };
        self.history.push((previous, uci, san.clone()));
        self.sync();
        san
    }

    fn sync(&mut self) {
        self.selected = None;
        self.fen = self.position.to_string();
        self.turn = match self.position.side_to_move() {
            Color::White => 'w',
            Color::Black => 'b',
        };
        self.check = !self.position.checkers().is_empty();
        self.last = self.history.last().map(|(_, uci, _)| uci.clone());
        // With no claim-draw control, only automatic draws end the game: fivefold repetition,
        // the 75-move rule and insufficient material.
        let hash = self.position.hash();
        let repeated = self
            .history
            .iter()
            .filter(|(pos, _, _)| pos.hash() == hash)
            .nth(3)
            .is_some();
        self.status = outcome(&self.position).or_else(|| {
            (repeated
                || self.position.halfmove_clock() >= 150
                || insufficient_material(&self.position))
            .then_some("draw")
        });
    }
}

/// Every legal move in the position, castling encoded king-to-rook as cozy-chess does.
fn legal_moves(position: &Position) -> Vec<Move> {
    let mut moves = Vec::new();
    position.generate_moves(|piece_moves| {
        moves.extend(piece_moves);
        false
    });
    moves
}

/// `checkmate` or `stalemate` when the side to move has no legal move.
fn outcome(position: &Position) -> Option<&'static str> {
    if position.generate_moves(|_| true) {
        None
    } else if position.checkers().is_empty() {
        Some("stalemate")
    } else {
        Some("checkmate")
    }
}

/// Bare kings, or kings and a single minor piece: no sequence of moves can mate.
fn insufficient_material(position: &Position) -> bool {
    let others = position.occupied() & !position.pieces(Role::King);
    match others.len() {
        0 => true,
        1 => !(others & (position.pieces(Role::Knight) | position.pieces(Role::Bishop))).is_empty(),
        _ => false,
    }
}

/// Standard algebraic notation for `m` in `position`, without the check suffix.
fn san(position: &Position, m: Move) -> String {
    let Some(role) = position.piece_on(m.from) else {
        return String::new();
    };
    let mover = position.side_to_move();
    if role == Role::King && position.color_on(m.to) == Some(mover) {
        return if m.to.file() > m.from.file() {
            "O-O"
        } else {
            "O-O-O"
        }
        .into();
    }
    let capture = position.color_on(m.to).is_some_and(|c| c != mover)
        || (role == Role::Pawn && m.from.file() != m.to.file());
    let mut out = String::new();
    if role == Role::Pawn {
        if capture {
            out.push(char::from(m.from.file()));
        }
    } else {
        out.push(char::from(role).to_ascii_uppercase());
        let rivals: Vec<Square> = legal_moves(position)
            .into_iter()
            .filter(|o| o.to == m.to && o.from != m.from && position.piece_on(o.from) == Some(role))
            .map(|o| o.from)
            .collect();
        if !rivals.is_empty() {
            if rivals.iter().all(|s| s.file() != m.from.file()) {
                out.push(char::from(m.from.file()));
            } else if rivals.iter().all(|s| s.rank() != m.from.rank()) {
                out.push(char::from(m.from.rank()));
            } else {
                out.push_str(&m.from.to_string());
            }
        }
    }
    if capture {
        out.push('x');
    }
    out.push_str(&m.to.to_string());
    if let Some(promotion) = m.promotion {
        out.push('=');
        out.push(char::from(promotion).to_ascii_uppercase());
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn play_all(moves: &[&str]) -> Board {
        let mut board = Board::default();
        for m in moves {
            board.play((*m).to_owned()).expect("legal move");
        }
        board
    }

    #[test]
    fn san_covers_castling_captures_disambiguation_and_mate() {
        let board = play_all(crate::games::OPERA_GAME);
        let san = board.history_san();
        assert_eq!(san[0], "e4");
        assert_eq!(san[6], "dxe5");
        assert_eq!(san[7], "Bxf3");
        assert_eq!(san[21], "Nbd7");
        assert_eq!(san[22], "O-O-O");
        assert_eq!(san[20], "Bxb5+");
        assert_eq!(san.last().map(String::as_str), Some("Rd8#"));
        assert_eq!(board.status, Some("checkmate"));
    }

    #[test]
    fn promotion_and_kingside_castling() {
        let mut board = Board {
            position: "4k3/P7/8/8/8/8/8/4K2R w K - 0 1".parse().expect("fen"),
            ..Board::default()
        };
        board.sync();
        assert_eq!(board.play("a7a8q".into()), Ok("a8=Q+".into()));
        board.play("e8d7".into()).expect("legal");
        assert_eq!(board.play("e1g1".into()), Ok("O-O".into()));
    }
}
