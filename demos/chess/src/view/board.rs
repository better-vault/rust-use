use super::theme::{
    BOARD_SIZE, CHECK_TINT, DARK, DOT_SIZE, FRAME_RADIUS, LABEL_SIZE, LAST_MOVE, LIGHT, PIECE_SIZE,
    SELECTED, SQUARE, TARGET,
};
use chess_demo::Board;
use chess_demo::board::{File, Rank, Square};
use gpui::{
    AssetSource, Div, FontWeight, ImageSource, MouseButton, Resource, SharedString, div, img,
    prelude::*, px, rgb, rgba,
};
use rust_use::prelude::{ChangeSource, Handle};
use std::borrow::Cow;

pub(crate) struct Pieces;

macro_rules! pieces {
    ($($piece:literal),*) => {
        &[
            $((
                concat!("pieces/", $piece, ".svg"),
                include_bytes!(concat!("../../assets/pieces/", $piece, ".svg")).as_slice(),
            ),)*
            ("check.svg", CHECK_TINT),
        ]
    }
}
const ASSETS: &[(&str, &[u8])] = pieces!(
    "wP", "wB", "wN", "wR", "wQ", "wK", "bP", "bB", "bN", "bR", "bQ", "bK"
);
impl AssetSource for Pieces {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| (*name).into())
            .collect())
    }
}

fn embedded(name: String) -> gpui::Img {
    img(ImageSource::Resource(Resource::Embedded(name.into())))
}

pub(super) fn board(state: &Board, handle: &Handle<Board>) -> Div {
    let targets = state
        .selected()
        .map(|from| state.legal_targets(from))
        .unwrap_or_default();
    let last = state.last_move();
    let checked = state.checked_king();
    div()
        .size(px(BOARD_SIZE))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(px(FRAME_RADIUS))
        .shadow_md()
        .children(Rank::ALL.into_iter().rev().map(|rank| {
            div().flex().children(File::ALL.map(|file| {
                let sq = Square::new(file, rank);
                let (base, ink) = if (sq.file() as usize + sq.rank() as usize) % 2 == 1 {
                    (LIGHT, DARK)
                } else {
                    (DARK, LIGHT)
                };
                let mut color = rgb(base);
                if state.selected() == Some(sq) {
                    color = color.blend(rgba(SELECTED));
                } else if last.is_some_and(|(a, b)| sq == a || sq == b) {
                    color = color.blend(rgba(LAST_MOVE));
                }
                let piece = state.piece_at(sq);
                let handle = handle.clone();
                let label = || {
                    div()
                        .absolute()
                        .text_size(px(LABEL_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(ink))
                };
                div()
                    .relative()
                    .size(px(SQUARE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color)
                    .when(sq == Square::A8, |d| d.rounded_tl(px(FRAME_RADIUS)))
                    .when(sq == Square::H8, |d| d.rounded_tr(px(FRAME_RADIUS)))
                    .when(sq == Square::A1, |d| d.rounded_bl(px(FRAME_RADIUS)))
                    .when(sq == Square::H1, |d| d.rounded_br(px(FRAME_RADIUS)))
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        handle.update(ChangeSource::User, |b| b.click(sq));
                    })
                    .when(checked == Some(sq), |d| {
                        d.child(embedded("check.svg".into()).absolute().size_full())
                    })
                    .when_some(piece, |d, p| {
                        d.child(
                            embedded(format!("pieces/{}{}.svg", p.color_char(), p.role_char()))
                                .size(px(PIECE_SIZE)),
                        )
                    })
                    .when(targets.contains(&sq), |d| {
                        let marker = div().absolute().rounded_full();
                        d.child(if piece.is_some() {
                            marker.size_full().border_8().border_color(rgba(TARGET))
                        } else {
                            marker.size(px(DOT_SIZE)).bg(rgba(TARGET))
                        })
                    })
                    .when(file == File::A, |d| {
                        d.child(label().top_0().left_1().child(char::from(rank).to_string()))
                    })
                    .when(rank == Rank::First, |d| {
                        d.child(
                            label()
                                .bottom_0()
                                .right_1()
                                .child(char::from(file).to_string()),
                        )
                    })
            }))
        }))
}

#[cfg(test)]
mod tests {
    use super::ASSETS;

    #[test]
    fn piece_svgs_parse() -> Result<(), String> {
        let options = usvg::Options::default();
        for &(path, bytes) in ASSETS
            .iter()
            .filter(|(path, _)| path.starts_with("pieces/"))
        {
            usvg::Tree::from_data(bytes, &options).map_err(|err| format!("{path}: {err}"))?;
        }
        Ok(())
    }
}
