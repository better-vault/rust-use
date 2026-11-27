//! Exercise generated primitives through the same registry used by agents.
#![cfg(feature = "macros")]

use rust_use::InvokeError;
use rust_use::prelude::*;

#[derive(Default)]
struct Selection(bool);

impl Behavior for Selection {
    fn describe(&self, out: &mut Describe) {
        out.attr("selected", self.0);
    }
    fn actions(&self) -> Vec<ActionSpec> {
        vec![ActionSpec {
            name: "select",
            doc: "Select.",
            params: &[],
        }]
    }
    fn is_enabled(&self, action: &str) -> bool {
        action == "select" && !self.0
    }
    fn invoke(&mut self, action: &str, _: &Args) -> Option<Result<Value, ActionError>> {
        (action == "select").then(|| {
            self.0 = true;
            Ok(Value::Null)
        })
    }
    fn queryable(&self) -> bool {
        true
    }
    fn query(&self, q: &Query) -> Option<Page> {
        Some(Page::from_iter([Value::from(self.0)], q))
    }
}

/// A button that mutes audio.
#[derive(Primitive, Default)]
#[primitive(name = "mute-button")]
struct MuteButton {
    #[agent(value)]
    muted: bool,
    volume: u8,
    #[agent(skip)]
    cache: Vec<u8>,
    #[agent(rename = "lvl")]
    level: u8,
    #[agent(flag)]
    busy: bool,
    #[behavior]
    selection: Selection,
}

#[actions]
impl MuteButton {
    /// Mute or unmute.
    fn toggle(&mut self) {
        self.muted = !self.muted;
    }

    /// Set the volume
    /// (0–100).
    ///
    /// Saves the previous volume.
    fn set_volume(&mut self, volume: u8) -> Result<(), ActionError> {
        if volume > 100 {
            return Err(ActionError::Rejected("volume".into()));
        }
        self.cache.push(self.volume);
        self.volume = volume;
        Ok(())
    }

    /// Undo the last change.
    #[action(enabled = Self::can_undo)]
    fn undo(&mut self) {
        if let Some(v) = self.cache.pop() {
            self.volume = v;
        }
    }

    /// Rename.
    #[action(name = "rename_to")]
    fn rename(&mut self, name: String, note: Option<String>) -> String {
        self.busy = true;
        note.map_or(name.clone(), |n| name + &n)
    }

    /// Report a failure.
    fn reject(&mut self) -> Result<(), String> {
        if self.busy {
            Err("busy".into())
        } else {
            Ok(())
        }
    }

    #[action(skip)]
    fn helper(&mut self) {
        self.cache.clear();
    }
    fn can_undo(&self) -> bool {
        !self.cache.is_empty()
    }
}

#[test]
fn describe_and_specs() -> Result<(), InvokeError> {
    let mut button = MuteButton::default();
    button.helper();
    assert!(button.owns("undo"));
    assert!(button.owns("select"));
    assert!(!button.owns("missing"));
    let specs = button.actions();
    assert_eq!(
        specs.iter().map(|s| s.name).collect::<Vec<_>>(),
        [
            "toggle",
            "set_volume",
            "undo",
            "rename_to",
            "reject",
            "select"
        ]
    );
    assert_eq!(specs[1].doc, "Set the volume (0–100).");
    assert_eq!(
        specs[1].params,
        [ParamSpec {
            name: "volume",
            ty: "int",
            optional: false
        }]
    );
    assert_eq!(
        specs[3].params,
        [
            ParamSpec {
                name: "name",
                ty: "str",
                optional: false
            },
            ParamSpec {
                name: "note",
                ty: "str",
                optional: true
            }
        ]
    );
    let ui = Ui::new();
    let button = ui.mount(button);
    let id = button.id();
    let initial = ui.component(id)?;
    assert_eq!(initial.name, "mute-button");
    assert!(initial.queryable);
    let mut expected = Describe::new();
    expected
        .value(false)
        .attr("volume", 0)
        .attr("lvl", 0)
        .attr("selected", false);
    assert_eq!(initial.describe, expected);
    Ok(())
}

#[test]
fn dispatch() -> Result<(), InvokeError> {
    let ui = Ui::new();
    let button = ui.mount(MuteButton::default());
    let id = button.id();
    let act = |name: &str, args: Args| ui.invoke(id, name, args, ChangeSource::Agent);
    assert_eq!(
        act("undo", ().into()),
        Err(ActionError::Disabled("undo".into()).into())
    );
    assert_eq!(act("toggle", ().into())?, Value::Null);
    assert!(matches!(
        act("set_volume", ("loud",).into()),
        Err(InvokeError::Action(ActionError::InvalidArg {
            name: "volume",
            ..
        }))
    ));
    assert_eq!(
        act("set_volume", ().into()),
        Err(ActionError::MissingArg("volume").into())
    );
    assert_eq!(
        act("set_volume", (40, 2).into()),
        Err(ActionError::InvalidArg {
            name: "args",
            reason: "expected 1, got 2".into()
        }
        .into())
    );
    assert_eq!(act("set_volume", (40,).into())?, Value::Null);
    assert_eq!(button.read(|b| b.volume), Some(40));
    act("undo", ().into())?;
    assert_eq!(act("rename_to", ("a",).into())?, Value::from("a"));
    assert_eq!(act("rename_to", ("a", "b").into())?, Value::from("ab"));
    assert_eq!(
        act("reject", ().into()),
        Err(ActionError::Rejected("busy".into()).into())
    );
    act("select", ().into())?;
    assert_eq!(
        act("select", ().into()),
        Err(ActionError::Disabled("select".into()).into())
    );
    let mut expected = Describe::new();
    expected
        .value(true)
        .attr("volume", 0)
        .attr("lvl", 0)
        .flag("busy", true)
        .attr("selected", true);
    assert_eq!(ui.component(id)?.describe, expected);
    assert_eq!(
        ui.query(id, &Query::default())?.map(|p| p.items),
        Some(vec![true.into()])
    );
    Ok(())
}
