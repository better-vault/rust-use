//! Protocol examples and screen-size regression budget.
use insta::assert_snapshot;
use rust_use_agent::{SnapshotOptions, UiAgentExt, step};
use rust_use_core::{
    ActionError, ActionSpec, Args, Behavior, ChangeSource, Describe, List, Page, ParamSpec,
    Primitive, Query, Ui, Value,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const fn action(name: &'static str, doc: &'static str, params: &'static [ParamSpec]) -> ActionSpec {
    ActionSpec::new(name, doc, params)
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
                const { &[ParamSpec::new("volume", "int")] },
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
                const { &[ParamSpec::new("uci", "str")] },
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
    fn queryable(&self) -> bool {
        true
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
    assert_snapshot!(agent.handle("snapshot --from @2 --limit 1"),@r#"
    rev 7
    @2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
    more 1 from=@3
    "#);
    assert_snapshot!(agent.handle("snapshot --from @3 --limit 1"),@r#"
    rev 7
    @3 select "Country" =Sweden open ?query
    "#);
    assert_snapshot!(agent.handle("query --kind select SWE"),@r#"
    rev 7
    @3 select "Country" =Sweden open ?query
    "#);
    assert_snapshot!(agent.handle("query --text \"CHESS BOARD\""),@r#"
    rev 7
    @2 board "Chess board" ="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" turn=b check !move(uci) !undo
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
    assert_eq!(agent.handle("diff 7"), agent.diff_since(7).to_string());
    assert_eq!(agent.handle("diff"), "rev 8");
    assert_eq!(agent.handle("diff 8"), "rev 8");
    assert!(agent.handle("diff invalid").starts_with("err"));
}

fn large_screen() -> Ui {
    let ui = Ui::new();
    for id in 1..=250 {
        if id % 2 == 0 {
            ui.mount(Mute(false));
        } else {
            ui.mount(Select);
        }
    }
    ui
}

#[test]
fn snapshot_pages() {
    let agent = large_screen().agent();
    for (request, first, last, more) in [
        ("snapshot", 1, 100, Some("more 150 from=@101")),
        ("snapshot --from @101", 101, 200, Some("more 50 from=@201")),
        ("snapshot --from @201", 201, 250, None),
    ] {
        let reply = agent.handle(request);
        let mut lines = reply.lines();
        assert_eq!(lines.next(), Some("rev 250"));
        for id in first..=last {
            assert!(
                lines
                    .next()
                    .is_some_and(|s| s.starts_with(&format!("@{id} ")))
            );
        }
        assert_eq!(lines.next(), more);
        assert_eq!(lines.next(), None);
    }
    assert_eq!(agent.snapshot().components.len(), 100);
    assert_eq!(
        agent.snapshot().to_json()["more"],
        serde_json::json!({"remaining":150,"from":101})
    );
    assert_eq!(agent.diff_since(0).changed.len(), 250);
}

#[test]
fn component_search_pages() {
    let agent = large_screen().agent();
    let first = agent.search(Some("select"), None, SnapshotOptions::default());
    assert_eq!(first.components.len(), 100);
    assert!(first.components.iter().all(|c| c.name == "select"));
    let reply = first.to_string();
    assert!(reply.ends_with("\nmore 25 from=@201"));
    assert_eq!(agent.handle("query --kind select"), reply);
    assert_eq!(agent.handle("query --text sWeD"), reply);
    assert_eq!(agent.handle("query --text SeLEcT"), reply);

    // Start at an id which does not match and count only later matches.
    let reply = agent.handle("query --kind select sw --from @200 --limit 2");
    assert_snapshot!(reply,@"
    rev 250
    @201 select =Sweden open ?query
    @203 select =Sweden open ?query
    more 23 from=@205
    ");
    let last = agent.handle("query --text sw --from @201");
    assert_eq!(last.lines().count(), 26);
    assert!(last.ends_with("\n@249 select =Sweden open ?query"));
    assert_eq!(agent.handle("query --kind mute-button Sweden"), "rev 250");
    assert_eq!(agent.diff().changed.len(), 250);
}

#[test]
fn invalid_component_page_args() {
    let agent = screen().agent();
    for request in [
        "snapshot --from",
        "snapshot --from 1",
        "snapshot --limit -1",
        "snapshot --verbose extra",
        "query --text",
        "query --kind",
        "query --kind select \"a\"extra",
        "query --text S --from @oops",
    ] {
        assert!(agent.handle(request).starts_with("err"), "{request}");
    }
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
    let mut snapshot = agent.snapshot();
    let ordinary = snapshot.to_string().len();
    let first = &mut snapshot.components[0].actions[0];
    *first = first.confirm("Toggle the microphone");
    assert_eq!(snapshot.to_string().len(), ordinary + 1);
    assert_eq!(snapshot.components.len(), 20);
    assert!(snapshot.to_string().len() < 1400);
    let reply = agent.handle("act @1 toggle");
    assert_eq!(reply.lines().filter(|s| s.starts_with('@')).count(), 1);
    assert!(reply.starts_with("ok\n=> 1 true\nrev 21\n@1 "));
}

#[test]
fn large_list_snapshot_budget() {
    const SNAPSHOTS: u32 = 100;

    #[derive(rust_use_macros::Primitive)]
    #[primitive(no_actions)]
    struct LargeList {
        #[behavior]
        items: List<u64>,
    }

    let ui = Ui::new();
    ui.mount(LargeList {
        items: List::new((0..100_000).collect()),
    });
    let agent = ui.agent();
    let snapshot = agent.snapshot();
    assert!(snapshot.to_string().ends_with(" ?query"));
    assert_eq!(snapshot.to_json()["components"][0]["query"], true);
    let diff = agent.diff_since(0);
    assert!(diff.to_string().ends_with(" ?query"));
    assert_eq!(diff.to_json()["changed"][0]["query"], true);

    let start = Instant::now();
    for _ in 0..SNAPSHOTS {
        black_box(agent.snapshot());
    }
    let elapsed = start.elapsed();
    println!("Mean snapshot time: {:?}", elapsed / SNAPSHOTS);
    assert!(elapsed < Duration::from_millis(100), "took {elapsed:?}");
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
        "err unknown command `help` (try: hello, snapshot, diff, act, query)"
    );
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn mcp_tools_return_compact_replies() -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::{ServiceExt, model::CallToolRequestParams};
    use rust_use_agent::mcp::McpServer;
    use serde_json::json;

    let ui = screen();
    let expected = screen().agent();
    let (a, b) = tokio::io::duplex(8192);
    let (server, client) = tokio::join!(McpServer::new(ui.agent()).serve(a), ().serve(b));
    let server = server?;
    let client = client?;
    let tools = client.list_all_tools().await?;
    let mut names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(names, ["act", "query", "snapshot"]);
    assert!(
        tools
            .iter()
            .map(|t| t.description.as_deref().unwrap_or_default().len())
            .sum::<usize>()
            <= 600
    );

    for (tool, args, request) in [
        ("snapshot", json!({}), "snapshot"),
        ("act", json!({"script":"Mute toggle"}), "act Mute toggle"),
        ("snapshot", json!({"verbose":true}), "snapshot --verbose"),
        (
            "snapshot",
            json!({"from":"@2","limit":1,"verbose":true}),
            "snapshot --verbose --from @2 --limit 1",
        ),
        (
            "query",
            json!({"ref":"Country","text":"sw","from":1}),
            "query Country sw from=1",
        ),
        ("act", json!({"script":"Mute missing"}), "act Mute missing"),
    ] {
        let reply = client
            .call_tool(
                CallToolRequestParams::new(tool).with_arguments(serde_json::from_value(args)?),
            )
            .await?;
        let text = expected.handle(request);
        assert_eq!(reply.is_error.unwrap_or(false), text.starts_with("err"));
        assert_eq!(reply.content.len(), 1);
        assert_eq!(reply.content[0].as_text().map(|c| &c.text), Some(&text));
    }
    client.cancel().await?;
    server.waiting().await?;
    Ok(())
}

#[test]
fn null_primary_value_is_omitted() {
    struct Player(Option<String>);
    impl rust_use_core::Behavior for Player {
        fn describe(&self, out: &mut rust_use_core::Describe) {
            out.value(&self.0);
        }
    }
    impl rust_use_core::Primitive for Player {
        const NAME: &'static str = "player";
    }
    let ui = rust_use_core::Ui::new();
    let player = ui.mount(Player(None));
    let agent = rust_use_agent::UiAgentExt::agent(&ui);
    assert_eq!(agent.snapshot().to_string(), "rev 1\n@1 player");
    player.update(rust_use_core::ChangeSource::Program, |p| {
        p.0 = Some("Intro".into());
    });
    assert_eq!(agent.snapshot().to_string(), "rev 2\n@1 player =Intro");
}
