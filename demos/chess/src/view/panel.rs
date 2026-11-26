use super::theme::{
    BACKGROUND, BOARD_SIZE, BUTTON, BUTTON_HOVER, CARD, DARK, MUTED, PANEL_WIDTH, TEXT,
};
use chess_demo::{
    Board, ChessApp,
    metrics::{Traffic, Transport},
};
use gpui::{Div, FontFeatures, FontWeight, ScrollHandle, TextStyle, div, prelude::*, px, rgb};
use rust_use::prelude::HandleExt;

fn card() -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .p_3()
        .gap_2()
        .bg(rgb(CARD))
        .rounded_lg()
}
fn heading(label: &'static str) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

/// The move list takes whatever height the fixed cards leave and scrolls inside it.
fn moves(state: &Board, scroll: &ScrollHandle) -> Div {
    let history = state.history_san();
    let mut list = div()
        .id("move-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .text_sm();
    for (index, pair) in history.chunks(2).enumerate() {
        list = list.child(
            div()
                .flex()
                .py_0p5()
                .child(
                    div()
                        .w_10()
                        .text_color(rgb(MUTED))
                        .child(format!("{}.", index + 1)),
                )
                .children(pair.iter().map(|san| div().flex_1().child(san.clone()))),
        );
    }
    card()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(heading("Moves"))
        .child(list)
}

fn column(name: &'static str, traffic: Traffic, active: bool) -> Div {
    let latency = traffic.mean_latency();
    let time = if traffic.calls == 0 {
        "—".to_owned()
    } else if latency.as_millis() == 0 {
        format!("{} µs", latency.as_micros())
    } else {
        format!("{:.1} ms", latency.as_secs_f64() * 1000.)
    };
    let rows = [
        ("Calls", traffic.calls.to_string()),
        ("Bytes in", traffic.request_bytes.to_string()),
        ("Bytes out", traffic.reply_bytes.to_string()),
        ("≈Tokens", traffic.estimated_tokens().to_string()),
        ("Latency", time),
    ];
    let mut value_font = TextStyle::default().font();
    value_font.features = FontFeatures(vec![("tnum".into(), 1)].into());
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .child(
            div()
                .pb_1()
                .border_b_2()
                .border_color(rgb(if active { DARK } else { BACKGROUND }))
                .font_weight(FontWeight::SEMIBOLD)
                .child(name),
        )
        .children(rows.into_iter().map(|(label, value)| {
            // Flex rows rather than a CSS grid: gpui 0.2.2 over-measures the height of a grid
            // of text cells, which made this card swallow the move list.
            div()
                .flex()
                .justify_between()
                .gap_2()
                .child(div().text_color(rgb(MUTED)).child(label))
                .child(div().font(value_font.clone()).child(value))
        }))
}

fn traffic(game: &ChessApp) -> Div {
    let metrics = game.metrics.clone();
    let view = metrics.snapshot();
    card()
        .flex_none()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(heading("Agent traffic"))
                .child(
                    div()
                        .id("reset-metrics")
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .cursor_pointer()
                        .on_click(move |_, _, _| metrics.reset())
                        .child("Reset"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(column(
                    "rust-use",
                    view.native,
                    view.last == Some(Transport::Native),
                ))
                .child(column(
                    "Generic automation",
                    view.baseline,
                    view.last == Some(Transport::Baseline),
                )),
        )
}

pub(super) fn panel(state: &Board, game: &ChessApp, moves_scroll: &ScrollHandle) -> Div {
    let button = |enabled| {
        div()
            .flex_1()
            .rounded(px(8.))
            .py_2()
            .text_center()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .bg(rgb(BUTTON))
            .text_color(rgb(TEXT))
            .when_else(
                enabled,
                |b| b.cursor_pointer().hover(|s| s.bg(rgb(BUTTON_HOVER))),
                |b| b.opacity(0.4),
            )
    };
    let can_undo = state.can_undo();
    div()
        .w(px(PANEL_WIDTH))
        .h(px(BOARD_SIZE))
        .p_4()
        .gap_3()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .mb_1()
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("rust-use chess"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child("Agents play through behaviors, not pixels"),
                ),
        )
        .child(
            card()
                .flex_none()
                .child(heading("Status"))
                .child(div().text_lg().child(state.status_text())),
        )
        .child(moves(state, moves_scroll))
        .child(traffic(game))
        .child(
            div()
                .flex_none()
                .flex()
                .gap_2()
                .child(
                    button(true)
                        .id("new-game")
                        .on_click(game.board.on_click("reset", ()))
                        .child("New game"),
                )
                .child(
                    button(can_undo)
                        .id("undo")
                        .when(can_undo, |b| b.on_click(game.board.on_click("undo", ())))
                        .child("Undo"),
                ),
        )
}
