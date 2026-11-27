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
