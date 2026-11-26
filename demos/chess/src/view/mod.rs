mod board;
mod panel;
mod theme;

pub(crate) use board::Pieces;
use chess_demo::ChessApp;
use gpui::{
    App, Context, FocusHandle, KeyBinding, ScrollHandle, Task, Window, actions, div, prelude::*,
    px, rgb,
};
use rust_use::prelude::{ChangeSource, UiBinding, UiExt};

actions!(chess, [Undo, NewGame]);

pub(crate) struct ChessWindow {
    game: ChessApp,
    focus: FocusHandle,
    /// Scroll position of the move list, kept so new moves can scroll it to the bottom.
    moves: ScrollHandle,
    /// Number of plies shown last frame; the list follows the game when it grows.
    plies: usize,
    _binding: UiBinding,
    _traffic: Task<()>,
}
impl ChessWindow {
    pub(crate) fn bind_keys(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("cmd-z", Undo, None),
            KeyBinding::new("cmd-n", NewGame, None),
        ]);
    }
    pub(crate) fn new(game: ChessApp, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        let binding = game.ui.bind(cx);
        // Read-only IPC requests change the counters without changing the board.
        let updates = game.metrics.updates();
        let traffic = cx.spawn(async move |this, cx| {
            while updates.recv().await.is_ok() {
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            game,
            focus,
            moves: ScrollHandle::new(),
            plies: 0,
            _binding: binding,
            _traffic: traffic,
        }
    }
    fn invoke(&self, name: &str) {
        if let Err(err) = self.game.board.invoke(name, (), ChangeSource::User) {
            eprintln!("Chess action {name}: {err}");
        }
    }
}
impl Render for ChessWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let plies = self.game.board.read(|b| b.history_san().len()).unwrap_or(0);
        if plies > self.plies {
            self.moves.scroll_to_bottom();
        }
        self.plies = plies;
        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Undo, _, _| {
                if this.game.board.read(chess_demo::Board::can_undo) == Some(true) {
                    this.invoke("undo");
                }
            }))
            .on_action(cx.listener(|this, _: &NewGame, _, _| this.invoke("reset")))
            .size_full()
            .pt(px(28.))
            .bg(rgb(theme::BACKGROUND))
            .text_color(rgb(theme::TEXT))
            .flex()
            .items_center()
            .justify_center()
            .children(
                self.game
                    .board
                    .read(|state| {
                        [
                            board::board(state, &self.game.board),
                            panel::panel(state, &self.game, &self.moves),
                        ]
                    })
                    .into_iter()
                    .flatten(),
            )
    }
}
