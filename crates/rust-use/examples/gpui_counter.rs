//! A GPUI counter driven by buttons and an agent on a background thread.

use std::{thread, time::Duration};

use gpui::{
    Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use rust_use::prelude::*;

#[derive(Primitive, Default)]
struct Counter {
    #[agent(value)]
    count: u64,
}

#[actions]
impl Counter {
    /// Add one to the count.
    fn increment(&mut self) {
        self.count = self.count.saturating_add(1);
    }

    /// Return the count to zero.
    fn reset(&mut self) {
        self.count = 0;
    }
}

struct CounterView {
    counter: Handle<Counter>,
    _binding: UiBinding,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let counter = &self.counter;
        let count = counter.read(|counter| counter.count).unwrap_or_default();
        let increment = button("increment", "Increment", 0x1d_4e_d8)
            .on_click(counter.on_click("increment", ()));
        let reset = button("reset", "Reset", 0x33_41_55).on_click(counter.on_click("reset", ()));
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x11_18_27))
            .text_color(rgb(0xf8_fa_fc))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .rounded_xl()
                    .bg(rgb(0x1e_29_3b))
                    .px_8()
                    .py_6()
                    .shadow_lg()
                    .child(div().text_size(px(48.)).child(count.to_string()))
                    .child(div().flex().gap_2().child(increment).child(reset))
                    .child("An agent adds one every two seconds."),
            )
    }
}

fn button(id: &'static str, label: &'static str, color: u32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .rounded_md()
        .px_4()
        .py_2()
        .cursor_pointer()
        .bg(rgb(color))
        .hover(|style| style.bg(rgb(0x47_55_69)))
        .child(label)
}

fn main() {
    Application::new().run(|cx| {
        let ui = Ui::new();
        let counter = ui.mount(Counter::default());
        let bounds = Bounds::centered(None, size(px(460.), px(320.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| CounterView {
                    counter,
                    _binding: ui.bind(cx),
                })
            },
        );
        if let Err(error) = window {
            eprintln!("Could not open the counter window: {error}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);

        let worker = thread::Builder::new().spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(2));
                if let Err(error) = ui.agent().act("counter increment") {
                    eprintln!("Counter agent action failed: {error}");
                    break;
                }
            }
        });
        if let Err(error) = worker {
            eprintln!("Could not start the counter agent: {error}");
        }
    });
}
