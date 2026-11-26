//! Protocol examples and screen-size regression budget.
use insta::assert_snapshot;
use rust_use_agent::{UiAgentExt, step};
use rust_use_core::{
    ActionError, ActionSpec, Args, Behavior, ChangeSource, Describe, Page, ParamSpec, Primitive,
    Query, Ui, Value,
};

const fn action(name: &'static str, doc: &'static str, params: &'static [ParamSpec]) -> ActionSpec {
    ActionSpec { name, doc, params }
}
struct Mute(bool);
impl Primitive for Mute {
    const NAME: &'static str = "mute-button";
}
impl Behavior for Mute {
    fn describe(&self, d: &mut Describe) {
        d.value(self.0).attr("volume", 40);
    }
    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            action("toggle", "Mute or unmute.", &[]),
            action(
                "set_volume",
                "Set volume.",
                &[ParamSpec {
                    name: "volume",
                    ty: "int",
                    optional: false,
                }],
            ),
        ]
    }
    fn invoke(&mut self, a: &str, _: &Args) -> Option<Result<Value, ActionError>> {
        if a == "toggle" {
            self.0 = !self.0;
            Some(Ok(self.0.into()))
        } else {
            None
        }
    }
}

const BLACK: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
const WHITE: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
struct Board(bool);
impl Primitive for Board {
    const NAME: &'static str = "board";
}
impl Behavior for Board {
    fn describe(&self, d: &mut Describe) {
        d.value(if self.0 { WHITE } else { BLACK })
            .attr("turn", if self.0 { "w" } else { "b" })
            .flag("check", !self.0);
    }
    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            action(
                "move",
                "Move a piece.",
                &[ParamSpec {
                    name: "uci",
                    ty: "str",
                    optional: false,
                }],
            ),
            action("undo", "Undo.", &[]),
        ]
    }
    fn invoke(&mut self, a: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        if a != "move" {
            return None;
        }
        Some(
            args.get::<String>(0, "uci")
                .and_then(|uci| match uci.as_str() {
                    "e2e4" | "e7e5" => {
                        self.0 = uci == "e7e5";
                        Ok(Value::Null)
                    }
                    _ => Err(ActionError::Rejected(
                        "illegal move (legal: a2a3 a2a4 b1a3 b1c3 …)".into(),
                    )),
                }),
        )
    }
}
struct Select;
impl Primitive for Select {
    const NAME: &'static str = "select";
}
impl Behavior for Select {
    fn describe(&self, d: &mut Describe) {
        d.value("Sweden").flag("open", true);
    }
    fn query(&self, q: &Query) -> Option<Page> {
        Some(Page::from_iter(
            ["Sweden", "Switzerland", "Japan"]
                .into_iter()
                .filter(|s| q.text.as_ref().is_none_or(|t| s.to_lowercase().contains(t)))
                .map(Value::from),
            q,
        ))
    }
}
fn screen() -> Ui {
    let ui = Ui::new();
    let _ = ui.mount(Mute(false)).label("Mute");
    let _ = ui.mount(Board(false)).label("Chess board");
    let _ = ui.mount(Select).label("Country");
    ui
}

#[test]
fn format_examples() {
    let ui = screen();
    let agent = ui.agent();
    ui.mount(Select).unmount();
    let other = screen();
    let temporary = other.mount(Select);
    let diff_session = other.agent();
    diff_session.snapshot();
    diff_session.handle("act board move e7e5");
    temporary.unmount();
    assert_snapshot!(diff_session.diff_since(7),@r#"
    rev 9
    @2 board "Chess board" ="rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" turn=w !move(uci) !undo
    -@4
    "#);
    assert_snapshot!(agent.handle("act board move e7e5"),@r#"
    ok
    rev 9
    @2 board "Chess board" ="rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2" turn=w !move(uci) !undo
    "#);

    let ui = screen();
    let agent = ui.agent();
    agent.handle("act board move e2e4");
    assert_snapshot!(agent.snapshot(),@r#"
    rev 7
    @1 mute-button "Mute" =false volume=40 !toggle !set_volume(volume:int)
    @2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
    @3 select "Country" =Sweden open ?query
    "#);
    assert_snapshot!(agent.handle("act \"Chess board\" move e2e4; # another move\n@2 move e2e5"),@r#"
    err 2 @2 move e2e5: illegal move (legal: a2a3 a2a4 b1a3 b1c3 …)
    rev 8
    @2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
    "#);
    assert_snapshot!(agent.handle("query \"Country\" sw"),@"
    Sweden
    Switzerland
    total 2
    ");
    assert_snapshot!(agent.handle("query \"Country\" sw from=50"),@"total 2");
    assert_eq!(agent.handle("diff"), "rev 8");
}

#[test]
fn compact_screen_budget() {
    let ui = Ui::new();
    for i in 0..20 {
        match i % 3 {
            0 => {
                ui.mount(Mute(false));
            }
            1 => {
                ui.mount(Board(false));
            }
            _ => {
                ui.mount(Select);
            }
        }
    }
    let agent = ui.agent();
    let snapshot = agent.snapshot();
    assert_eq!(snapshot.components.len(), 20);
    assert!(snapshot.to_string().len() < 1400);
    let reply = agent.handle("act @1 toggle");
    assert_eq!(reply.lines().filter(|s| s.starts_with('@')).count(), 1);
    assert!(reply.starts_with("ok\n=> 1 true\nrev 21\n@1 "));
}

#[test]
fn interceptor_lifetime_and_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let ui = screen();
    let agent = ui
        .agent()
        .allow(|r| r.action != "toggle")
        .allow(|r| r.id.get() != 2);
    assert!(agent.act(step("Mute", "toggle", ())).is_err());
    assert!(agent.act("board move e2e4").is_err());
    ui.invoke("Mute", "toggle", (), ChangeSource::User)?;
    assert_eq!(agent.diff_since(6).changed.len(), 1);
    assert_eq!(agent.diff().changed.len(), 1);
    drop(agent);
    ui.invoke("Mute", "toggle", (), ChangeSource::Agent)?;
    Ok(())
}

#[test]
fn verbose_snapshot_and_structured_content() {
    let ui = Ui::new();
    let button = ui.mount(Mute(false));
    let agent = ui.agent();
    assert_snapshot!(agent.handle("snapshot --verbose"),@r"
    rev 1
    @1 mute-button =false volume=40 !toggle !set_volume(volume:int)
      # Mute or unmute.
      # Set volume.
    ");
    let json = agent.snapshot().to_json();
    assert_eq!(
        json["components"][0]["actions"][1]["params"][0]["type"],
        "int"
    );
    assert_eq!(json["components"][0]["attrs"]["volume"], 40);
    assert_eq!(agent.handle("query @1"), "not queryable");
    assert_eq!(
        agent.handle("act @1 play"),
        "err 1 @1 play: `mute-button` has no action `play` (available: toggle,set_volume)\nrev 1"
    );
    button.unmount();
    let diff = agent.diff().to_json();
    assert_eq!(diff["removed"], serde_json::json!([1]));
}

#[test]
fn invalid_scripts_do_not_start_dispatch() {
    let ui = screen();
    let agent = ui.agent();
    for script in [
        "Mute toggle; board",
        "Mute toggle\nboard move [a,]",
        "board move \"bad",
        "@999999999999 move",
        "board move [x]y",
    ] {
        assert!(matches!(
            agent.act(script),
            Err(rust_use_agent::ActError::Parse { .. })
        ));
        assert_eq!(ui.revision(), 6);
    }
    assert_eq!(
        agent.handle("help"),
        "err unknown command `help` (try: snapshot, diff, act, query)"
    );
}
