//! Embed each built-in behavior in a primitive and drive its public agent interface.
#![cfg(feature = "macros")]

use rust_use::InvokeError;
use rust_use::prelude::*;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Primitive)]
#[primitive(no_actions)]
struct Control<B: Behavior> {
    #[behavior]
    input: B,
}

struct Mounted<B: Behavior> {
    ui: Ui,
    control: Handle<Control<B>>,
}
impl<B: Behavior> Mounted<B> {
    fn new(input: B) -> Self {
        let ui = Ui::new();
        let control = ui.mount(Control { input });
        Self { ui, control }
    }
    fn act(&self, name: &str, args: impl Into<Args>) -> Result<Value, InvokeError> {
        self.ui
            .invoke(self.control.id(), name, args, ChangeSource::Agent)
    }
    fn check(&self, expected: &Describe, names: &[&str]) -> Result<(), InvokeError> {
        let info = self.ui.component(self.control.id())?;
        assert_eq!(&info.describe, expected);
        assert_eq!(
            info.actions.iter().map(|s| s.name).collect::<Vec<_>>(),
            names
        );
        Ok(())
    }
}

#[test]
fn press() -> Result<(), InvokeError> {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let mut press = Press::new().on_press(move || {
        seen.fetch_add(1, Ordering::Relaxed);
    });
    press.set_pressed(true);
    let button = Mounted::new(press);
    button.act("press", ())?;
    button.check(Describe::new().flag("pressed", true), &["press"])?;
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(button.control.read(|c| c.input.presses), Some(1));
    Ok(())
}
#[test]
fn toggle() -> Result<(), InvokeError> {
    let switch = Mounted::new(Toggle::default());
    switch.act("toggle", ())?;
    switch.check(Describe::new().value(true), &["toggle", "set"])?;
    switch.act("set", (false,))?;
    switch.check(Describe::new().value(false), &["toggle", "set"])
}
#[test]
fn field() -> Result<(), InvokeError> {
    let field = Mounted::new(Field::new(0).with_validator(|v| {
        if *v < 0 {
            Err("negative".into())
        } else {
            Ok(())
        }
    }));
    field.act("set", (12,))?;
    assert_eq!(
        field.act("set", (-1,)),
        Err(ActionError::Rejected("negative".into()).into())
    );
    field.check(Describe::new().value(12), &["set"])
}
#[test]
fn selection() -> Result<(), InvokeError> {
    let single = Mounted::new(Selection::<u8>::default());
    single.check(&Describe::new(), &["select"])?;
    assert_eq!(
        single.act("clear", ()),
        Err(ActionError::Disabled("clear".into()).into())
    );
    single.act("select", (4,))?;
    single.check(Describe::new().attr("sel", 4), &["select", "clear"])?;
    assert_eq!(
        single.act("deselect", (4,)),
        Err(ActionError::Disabled("deselect".into()).into())
    );
    let multi = Mounted::new(Selection::<u8>::new(SelectionMode::Multiple));
    for key in [3, 5, 3] {
        multi.act("select", (key,))?;
    }
    multi.check(
        Describe::new().attr("sel", [3, 5]),
        &["select", "deselect", "clear"],
    )?;
    multi.act("deselect", (3,))?;
    multi.check(
        Describe::new().attr("sel", [5]),
        &["select", "deselect", "clear"],
    )?;
    multi.act("clear", ())?;
    multi.check(&Describe::new(), &["select", "deselect"])
}
#[test]
#[allow(
    clippy::float_cmp,
    reason = "The expected values are exactly representable."
)]
fn range() {
    let mut range = Range::new(0.0, 100.0).step(5.0);
    assert_eq!(range.bounds(), (0.0, 0.0));
    assert_eq!(range.set(12.0), 10.0);
    assert_eq!(range.set(13.0), 15.0);
    range.increment();
    assert_eq!(range.value(), 20.0);
    range.decrement();
    assert_eq!(range.value(), 15.0);
    assert_eq!(range.set(-1.0), 0.0);
    assert_eq!(range.set(101.0), 100.0);
    assert_eq!(range.set(f64::NAN), 100.0);

    let mut offset = Range::new(2.0, 14.0).step(5.0);
    assert_eq!(offset.set(9.0), 7.0);
    assert_eq!(offset.set(15.0), 14.0);

    let mut continuous = Range::new(0.0, 1.0).step(0.0);
    assert_eq!(continuous.set(0.375), 0.375);
    let mut describe = Describe::new();
    continuous.describe(&mut describe);
    assert_eq!(
        &describe,
        Describe::new()
            .value(0.375)
            .attr("min", 0)
            .attr("max", 1)
            .attr("step", 0)
    );
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "The expected values are exactly representable."
)]
fn range_span() -> Result<(), ActionError> {
    let mut range = Range::new(0.0, 100.0).step(5.0).span(79.0, 21.0);
    assert_eq!(range.bounds(), (20.0, 80.0));
    assert_eq!(range.set_span(21.0, 19.0)?, (20.0, 20.0));
    assert!(matches!(
        range.set_span(25.0, 15.0),
        Err(ActionError::Rejected(_))
    ));
    assert!(matches!(
        range.set_span(10.0, f64::NAN),
        Err(ActionError::InvalidArg { name: "end", .. })
    ));
    assert_eq!(range.bounds(), (20.0, 20.0));
    assert_eq!(range.set(30.0), 20.0);
    range.decrement();
    assert_eq!(range.bounds(), (15.0, 20.0));
    Ok(())
}

#[cfg(feature = "agent")]
#[test]
fn range_agent() -> Result<(), Box<dyn std::error::Error>> {
    let slider = Mounted::new(Range::new(0.0, 100.0));
    let agent = slider.ui.agent();
    agent.act("@1 set 40")?;
    assert_eq!(
        agent.snapshot().to_string(),
        "rev 2\n@1 control =40 min=0 max=100 !set(value:float)"
    );
    assert_eq!(
        slider.act("set_span", (20, 80)),
        Err(ActionError::Disabled("set_span".into()).into())
    );
    assert!(matches!(
        slider.act("set", (f64::NAN,)),
        Err(InvokeError::Action(ActionError::InvalidArg {
            name: "value",
            ..
        }))
    ));
    assert!(matches!(
        agent.act("@1 set NaN"),
        Err(rust_use::ActError::Step {
            source: InvokeError::Action(ActionError::InvalidArg { name: "value", .. }),
            ..
        })
    ));
    let reply = agent.act("@1 set 150")?;
    assert_eq!(reply.values, vec![(1, json!(100))]);
    assert!(reply.to_string().starts_with("ok\n=> 1 100\n"));

    let slider = Mounted::new(Range::new(0.0, 100.0).step(5.0).span(0.0, 100.0));
    let agent = slider.ui.agent();
    agent.act("@1 set_span 21 81")?;
    assert_eq!(
        agent.snapshot().to_string(),
        concat!(
            "rev 2\n@1 control =[20,80] min=0 max=100 step=5 ",
            "!set(value:float) !set_span(start:float,end:float)"
        )
    );
    Ok(())
}

#[test]
fn tabs() -> Result<(), ActionError> {
    let mut tabs = Tabs::new(["a", "b", "c"]);
    assert_eq!(tabs.tabs(), &["a", "b", "c"]);
    assert_eq!(tabs.selected(), "a");
    tabs.prev();
    assert_eq!(tabs.selected(), "c");
    tabs.next();
    assert_eq!(tabs.selected(), "a");
    tabs.select("b")?;
    assert!(tabs.is_visible("b"));
    assert!(!tabs.is_visible("a"));

    let mut empty = Tabs::new([] as [&str; 0]);
    empty.next();
    empty.prev();
    assert_eq!(empty.selected(), "");
    assert!(!empty.is_visible(""));
    Ok(())
}

#[cfg(feature = "agent")]
#[test]
fn tabs_agent() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Mounted::new(Tabs::new(["a", "b", "c"]));
    let agent = settings.ui.agent();
    agent.act("@1 select b")?;
    assert_eq!(
        agent.snapshot().to_string(),
        "rev 2\n@1 control =b tabs=[a,b,c] !select(tab)"
    );
    assert_eq!(
        agent.act("@1 select d").err().map(|err| err.to_string()),
        Some("step 1: unknown tab `d` (available: [\"a\", \"b\", \"c\"])".into())
    );
    assert_eq!(
        settings.control.read(|c| c.input.is_visible("b")),
        Some(true)
    );
    Ok(())
}

#[test]
fn list() -> Result<(), InvokeError> {
    let list = Mounted::new(List::new(vec!["Lemon", "Lime", "Pear"]));
    list.act("move", (1, 0))?;
    list.check(Describe::new().attr("n", 3), &["move"])?;
    let page = list.ui.query(
        list.control.id(),
        &Query {
            text: Some("L".into()),
            from: 0,
            limit: 1,
        },
    )?;
    assert_eq!(
        page,
        Some(Page {
            items: vec![json!("Lime")],
            total: 2,
            next: Some(1)
        })
    );
    Ok(())
}

#[test]
fn combobox_typed() -> Result<(), ActionError> {
    let mut country = Combobox::new(vec!["Sweden", "Switzerland"]);
    assert_eq!(country.options().items, ["Sweden", "Switzerland"]);
    assert_eq!(country.value(), None);
    assert_eq!(country.text(), None);
    assert!(!country.is_open());

    country.open();
    assert!(matches!(
        country.pick("sweden"),
        Err(ActionError::Rejected(_))
    ));
    assert!(country.is_open());
    assert_eq!(country.value(), None);
    country.pick("Sweden")?;
    assert_eq!(country.value(), Some(&"Sweden"));
    assert!(!country.is_open());

    country.open();
    country.close();
    assert_eq!(country.value(), Some(&"Sweden"));
    assert!(!country.is_open());
    country.clear();
    assert_eq!(country.value(), None);
    Ok(())
}

#[test]
fn combobox_picks_and_suggests_matching_options() -> Result<(), InvokeError> {
    let options = [
        "North",
        "north",
        "Northeast",
        "Northwest",
        "Far North",
        "True North",
        "South",
    ];
    let mut box_input = Combobox::new(options.map(str::to_owned).to_vec());
    box_input.open();
    let choice = Mounted::new(box_input);
    choice.check(
        Describe::new().attr("n", options.len()).flag("open", true),
        &["pick"],
    )?;
    assert!(matches!(
        choice.act("pick", ("nort",)),
        Err(InvokeError::Action(ActionError::Rejected(reason)))
            if reason.ends_with(r#"["North","north","Northeast","Northwest","Far North"]"#)
    ));
    assert_eq!(
        choice.ui.query(
            choice.control.id(),
            &Query {
                text: Some("nort".into()),
                from: 4,
                limit: 1,
            }
        )?,
        Some(Page {
            items: vec![json!("Far North")],
            total: 6,
            next: Some(5)
        }),
    );

    for (input, expected) in [("north", "north"), ("NORTH", "North")] {
        choice.act("pick", (input,))?;
        choice.check(
            Describe::new().value(expected).attr("n", options.len()),
            &["pick", "clear"],
        )?;
        assert_eq!(
            choice.control.read(|c| c.input.value().cloned()),
            Some(Some(expected.to_owned())),
        );
    }
    choice.act("clear", ())?;
    choice.check(Describe::new().attr("n", options.len()), &["pick"])?;

    let number = Mounted::new(Combobox::new(vec![12, 123]));
    number.act("pick", (12,))?;
    assert_eq!(
        number.control.read(|c| c.input.value().copied()),
        Some(Some(12))
    );
    assert!(matches!(
        number.act("pick", ("12",)),
        Err(InvokeError::Action(ActionError::Rejected(reason))) if reason.ends_with("[12,123]")
    ));
    Ok(())
}

#[test]
fn combobox_free_text_and_clear_are_gated() -> Result<(), InvokeError> {
    let options = vec!["Sweden".to_owned()];
    let locked = Mounted::new(Combobox::new(options.clone()));
    assert_eq!(
        locked.act("type", ("Elsewhere",)),
        Err(ActionError::Disabled("type".into()).into())
    );

    let editable = Mounted::new(Combobox::new(options).free_text(true));
    editable.check(Describe::new().attr("n", 1), &["pick", "type"])?;
    assert_eq!(
        editable.act("clear", ()),
        Err(ActionError::Disabled("clear".into()).into())
    );
    editable.act("pick", ("Sweden",))?;
    for text in ["Elsewhere", ""] {
        editable.act("type", (text,))?;
        editable.check(
            Describe::new().value(text).attr("n", 1),
            &["pick", "type", "clear"],
        )?;
        assert_eq!(
            editable
                .control
                .read(|c| (c.input.value().cloned(), c.input.text().map(str::to_owned))),
            Some((None, Some(text.to_owned()))),
        );
    }
    editable.act("pick", ("sweden",))?;
    assert_eq!(
        editable.control.read(|c| c.input.text().is_none()),
        Some(true)
    );
    editable.act("type", ("Elsewhere",))?;
    editable.act("clear", ())?;
    editable.check(Describe::new().attr("n", 1), &["pick", "type"])
}

#[test]
fn transfer() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Primitive)]
    #[primitive(no_actions)]
    struct Kanban {
        #[behavior]
        board: Transfer<String>,
    }

    let ui = Ui::new();
    let kanban = ui
        .mount(Kanban {
            board: Transfer::new()
                .with_list("todo", vec!["Fix login".into(), "Write tests".into()])
                .with_list("done", vec!["Add lint".into()]),
        })
        .label("board");
    assert!(matches!(
        ui.invoke("board", "move", ("Fix login", "done", -1), ChangeSource::Agent),
        Err(InvokeError::Action(ActionError::Rejected(reason))) if reason.contains("out of range")
    ));
    assert!(matches!(
        ui.invoke("board", "move", ("Missing item", "done"), ChangeSource::Agent),
        Err(InvokeError::Action(ActionError::Rejected(reason))) if reason.contains("item not found")
    ));
    ui.invoke(
        "board",
        "move",
        ("Write tests", "done", 0),
        ChangeSource::Agent,
    )?;
    assert_eq!(
        kanban.read(|k| k.board.list("done").map(<[String]>::to_vec)),
        Some(Some(vec!["Write tests".into(), "Add lint".into()]))
    );

    #[cfg(feature = "agent")]
    {
        let agent = ui.agent();
        agent.act(r#"board move "Fix login" done"#)?;
        assert_eq!(
            agent.snapshot().to_string(),
            "rev 4\n@1 kanban \"board\" todo=0 done=3 !move(item:any,to,at:int?) ?query"
        );
        assert_eq!(
            kanban.read(|k| k.board.list("done").map(<[String]>::to_vec)),
            Some(Some(vec![
                "Write tests".into(),
                "Add lint".into(),
                "Fix login".into()
            ]))
        );
    }
    Ok(())
}

#[test]
fn roving_focus() -> Result<(), InvokeError> {
    let focus = Mounted::new(RovingFocus::new(Orientation::Horizontal, 2));
    focus.check(&Describe::new(), &["focus"])?;
    focus.act("focus", (1,))?;
    focus.check(Describe::new().attr("focus", 1), &["focus"])
}

#[cfg(feature = "agent")]
#[test]
fn file_tree() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Default)]
    struct FileData;

    #[derive(Primitive)]
    #[primitive(no_actions)]
    struct FileTree {
        #[behavior]
        tree: Tree<FileData>,
    }

    let ui = Ui::new();
    let files = ui
        .mount(FileTree {
            tree: Tree::from_paths(["src/agent/format.rs", "src/lib.rs", "README.md"]),
        })
        .label("files");
    let agent = ui.agent();
    assert_eq!(
        agent.handle("query files format"),
        "src/agent/format.rs\ntotal 1"
    );
    assert_eq!(files.read(|f| f.tree.is_expanded("src")), Some(false));

    agent.act("files select src/agent/format.rs")?;
    assert_eq!(
        agent.snapshot().to_string(),
        concat!(
            "rev 3\n@1 file-tree \"files\" sel=src/agent/format.rs open=2 ",
            "!expand(path) !collapse(path) !select(path) !move(path,parent,at:int?) ?query"
        )
    );
    assert_eq!(
        files.read(|f| f.tree.selected().map(str::to_owned)),
        Some(Some("src/agent/format.rs".into()))
    );
    agent.act(r#"files move src/agent "" 0"#)?;
    assert_eq!(
        files.read(|f| f.tree.selected().map(str::to_owned)),
        Some(Some("agent/format.rs".into()))
    );
    Ok(())
}

#[test]
fn typeahead() -> Result<(), InvokeError> {
    let search = Mounted::new(Typeahead::default());
    assert!(matches!(
        search.act("type", ("x",)),
        Err(InvokeError::UnknownAction { .. })
    ));
    search.check(&Describe::new(), &[])
}
#[test]
fn popup() -> Result<(), InvokeError> {
    let menu = Mounted::new(Popup::default());
    menu.check(&Describe::new(), &["open"])?;
    menu.act("open", ())?;
    menu.check(Describe::new().flag("open", true), &["close"])?;
    assert_eq!(
        menu.act("open", ()),
        Err(ActionError::Disabled("open".into()).into())
    );
    menu.act("close", ())?;
    menu.check(&Describe::new(), &["open"])
}
#[test]
fn drag_drop() -> Result<(), InvokeError> {
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let mut drag = DragDrop::default().on_drop(move |from: u8, to: u8| {
        assert_eq!((from, to), (1, 2));
        called.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    drag.start(7);
    let board = Mounted::new(drag);
    board.act("drop", (1, 2))?;
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    board.check(Describe::new().attr("drag", 7), &["drop"])
}
