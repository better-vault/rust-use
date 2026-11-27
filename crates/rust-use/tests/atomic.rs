//! Atomic actions through the facade and compact protocol.
#![cfg(all(feature = "agent", feature = "macros"))]

use rust_use::prelude::*;
use rust_use::{ActError, ChangeKind, InvokeError};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Clone, Primitive)]
#[primitive(checkpoint)]
struct Meter {
    #[agent(value)]
    level: u8,
}

#[actions]
impl Meter {
    /// Set the level.
    fn set(&mut self, level: u8) -> u8 {
        self.level = level;
        level
    }

    /// Change the level and then fail.
    fn fail(&mut self) -> Result<(), ActionError> {
        self.level = 99;
        Err(ActionError::Rejected("rejected".into()))
    }
}

#[derive(Primitive)]
#[primitive(no_actions)]
struct Slider {
    #[behavior]
    range: Range,
}

#[test]
fn failed_atomic_batch_restores_both_primitives_in_reverse_order() {
    let ui = Ui::new();
    let first = ui.mount(Meter { level: 1 });
    let second = ui.mount(Meter { level: 2 });
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    let _subscription = ui.subscribe(move |event| {
        captured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event.clone());
    });
    let agent = ui.agent();
    assert_eq!(
        agent.handle("act atomic\n@1 set 5; @2 fail"),
        "err 2 @2 fail: rejected\nrolled back\nrev 5\n@1 meter =1 !set(level:int) !fail\n@2 meter =2 !set(level:int) !fail"
    );
    assert_eq!(first.read(|m| m.level), Some(1));
    assert_eq!(second.read(|m| m.level), Some(2));
    assert_eq!(agent.diff().to_string(), "rev 5");
    let events = events.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(
        events
            .iter()
            .map(|e| (e.id, &e.kind, e.revision, e.source))
            .collect::<Vec<_>>(),
        [
            (
                first.id(),
                &ChangeKind::Action("set".into()),
                3,
                ChangeSource::Agent
            ),
            (second.id(), &ChangeKind::Restored, 4, ChangeSource::Agent),
            (first.id(), &ChangeKind::Restored, 5, ChangeSource::Agent),
        ]
    );
}

#[test]
fn preflight_rejects_uncheckpointed_and_missing_targets() -> Result<(), Box<dyn std::error::Error>>
{
    let ui = Ui::new();
    let meter = ui.mount(Meter { level: 1 });
    let slider = ui.mount(Slider {
        range: Range::new(0.0, 100.0),
    });
    let agent = ui.agent();
    let before = ui.revision();
    let script = Script::from([
        step(meter.id(), "set", (5,)),
        step(slider.id(), "set", (50,)),
    ])
    .atomic();
    let err = agent.act(script).err().ok_or("atomic batch succeeded")?;
    assert!(matches!(
        err,
        ActError::NotAtomic {
            index: 2,
            component: "slider",
            ..
        }
    ));
    assert_eq!(
        err.to_reply(),
        "err 2 @2 set 50: @2 slider can't be rolled back (add #[primitive(checkpoint)])"
    );
    assert!(agent.act("atomic; @1 set 5; missing set 50").is_err());
    assert_eq!(ui.revision(), before);
    assert_eq!(meter.read(|m| m.level), Some(1));
    agent.act(Script::from(step(meter.id(), "set", (5,))).atomic())?;
    assert_eq!(meter.read(|m| m.level), Some(5));
    Ok(())
}

#[test]
fn interceptor_can_read_and_reject_a_reserved_action() {
    let ui = Ui::new();
    let meter = ui.mount(Meter { level: 1 });
    let from_hook = meter.clone();
    let agent = ui.agent().allow(move |req| {
        assert_eq!(from_hook.read(|m| m.level), Some(1));
        assert_eq!(
            from_hook.invoke("set", (9,), ChangeSource::User),
            Err(InvokeError::TransactionActive(req.id))
        );
        false
    });
    assert_eq!(
        agent.handle("act atomic; meter set 5"),
        "err 1 meter set 5: denied: denied\nrolled back\nrev 1"
    );
    assert_eq!(meter.update(ChangeSource::User, |m| m.level = 8), Some(()));
}
