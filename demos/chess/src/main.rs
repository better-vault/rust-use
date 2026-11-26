//! Interactive GPUI window for the chess benchmark.
mod view;

use chess_demo::ChessApp;
use gpui::{
    Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use view::{ChessWindow, Pieces};

fn main() -> gpui::Result<()> {
    let mut game = ChessApp::new();
    game.serve("chess")?;
    Application::new().with_assets(Pieces).run(move |cx| {
        ChessWindow::bind_keys(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(980.), px(720.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("rust-use chess".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        match cx.open_window(options, |window, cx| {
            cx.new(|cx| ChessWindow::new(game, window, cx))
        }) {
            Ok(_) => {
                cx.activate(true);
                eprintln!(
                    "rust-use chess: agents can connect with `rust-use --app chess snapshot`"
                );
            }
            Err(err) => {
                eprintln!("Could not open chess window: {err}");
                cx.quit();
            }
        }
    });
    Ok(())
}
