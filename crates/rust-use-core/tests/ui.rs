//! Public registry contract and callback reentrancy tests.
#![allow(clippy::unwrap_used)]

use rust_use_core::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Counter(Cell<u32>);
impl Behavior for Counter {
    fn describe(&self, out: &mut Describe) {
        out.value(self.0.get());
    }
    fn actions(&self) -> Vec<ActionSpec> {
        ["add", "reset", "fallthrough"]
            .map(|name| {
                ActionSpec::new(
                    name,
                    "",
                    if name == "add" {
                        const { &[ParamSpec::new("amount", "int")] }
                    } else {
                        &[]
                    },
                )
            })
            .to_vec()
    }
    fn is_enabled(&self, action: &str) -> bool {
        action != "reset" || self.0.get() > 0
    }
    fn invoke(&mut self, action: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        match action {
            "add" => Some(args.get::<u32>(0, "amount").map(|n| {
                self.0.set(self.0.get() + n);
                self.0.get().into()
            })),
            "reset" => {
                self.0.set(0);
                Some(Ok(Value::Null))
            }
            _ => None,
        }
    }
    fn queryable(&self) -> bool {
        true
    }
    fn query(&self, q: &Query) -> Option<Page> {
        Some(Page::from_iter((0..self.0.get()).map(Value::from), q))
    }
}
impl Primitive for Counter {
    const NAME: &'static str = "counter";
}

#[test]
fn lifecycle_and_revision_history() {
    let ui = Ui::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let log = events.clone();
    ui.subscribe(move |e| log.lock().unwrap().push(e.clone()))
        .detach();
    assert_eq!(ui.revision(), 0);
    let h = ui.mount(Counter::default());
    let id = h.id();
    assert_eq!(id.get(), 1);
    assert_eq!(ui.component(id).unwrap().revision, 1);
    let h = h.label("Score");
    assert_eq!(ui.component("Score").unwrap().revision, 2);
    assert_eq!(h.update(ChangeSource::User, |c| c.0.set(3)), Some(()));
    assert_eq!(ui.component(id).unwrap().revision, 3);
    assert_eq!(
        h.invoke("add", (2,), ChangeSource::Agent).unwrap(),
        Value::from(5)
    );
    let info = ui.component(id).unwrap();
    assert_eq!(info.revision, 4);
    assert_eq!(info.describe.primary(), Some(&5.into()));
    assert_eq!(info.actions[0].params[0].name, "amount");
    assert_eq!(ui.changes_since(1).changed, [id]);
    drop(h.clone());
    assert!(h.is_mounted());
    h.clone().unmount();
    assert!(!h.is_mounted());
    assert!(h.read(|_| ()).is_none());
    assert!(h.update(ChangeSource::User, |_| ()).is_none());
    h.set_label("gone");
    h.unmount();
    let other = ui.mount(Counter::default()).id();
    assert_eq!(other.get(), 2);
    assert_eq!(
        ui.changes_since(0),
        ChangeSet {
            revision: 6,
            changed: vec![other],
            removed: vec![id]
        }
    );
    assert!(ui.changes_since(5).removed.is_empty());
    assert_eq!(
        ui.components().iter().map(|i| i.id).collect::<Vec<_>>(),
        [other]
    );
    let events = events.lock().unwrap();
    let expected = [
        (ChangeKind::Mounted, ChangeSource::Program),
        (ChangeKind::Relabeled, ChangeSource::Program),
        (ChangeKind::Updated, ChangeSource::User),
        (ChangeKind::Action("add".into()), ChangeSource::Agent),
        (ChangeKind::Unmounted, ChangeSource::Program),
        (ChangeKind::Mounted, ChangeSource::Program),
    ];
    for (i, (kind, source)) in expected.into_iter().enumerate() {
        assert_eq!(
            events[i],
            ChangeEvent {
                id: if i == 5 { other } else { id },
                revision: i as u64 + 1,
                kind,
                source
            }
        );
    }
    assert_eq!(events.len(), 6);
}

#[test]
fn dispatch_errors_and_veto_order() {
    let ui = Ui::new();
    let h = ui.mount(Counter::default()).label("Score");
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let read = h.clone();
    let first = ui.intercept(move |r| {
        assert_eq!(
            (r.id, r.component, r.label, r.action, r.source),
            (
                read.id(),
                "counter",
                Some("Score"),
                "add",
                ChangeSource::Agent
            )
        );
        assert_eq!(r.args.as_slice(), &[Value::from(3)]);
        assert_eq!(read.read(|c| c.0.get()), Some(0));
        assert_eq!(read.ui().component(r.id).unwrap().revision, 2);
        count.fetch_add(1, Ordering::SeqCst);
        Decision::Deny("locked".into())
    });
    let second = ui.intercept(|_| panic!("called after denial"));
    let invoke = |a| h.invoke(a, (3,), ChangeSource::Agent);
    let unknown = |a: &str| InvokeError::UnknownAction {
        component: "counter".into(),
        action: a.into(),
        available: "add,fallthrough".into(),
    };
    assert!(matches!(
        ui.invoke("absent", "add", (), ChangeSource::Agent),
        Err(InvokeError::Resolve(_))
    ));
    assert_eq!(invoke("typo"), Err(unknown("typo")));
    assert_eq!(
        invoke("reset"),
        Err(ActionError::Disabled("reset".into()).into())
    );
    assert_eq!(invoke("add"), Err(InvokeError::Denied("locked".into())));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop((first, second));
    assert_eq!(invoke("fallthrough"), Err(unknown("fallthrough")));
    assert_eq!(
        h.invoke("add", (), ChangeSource::User),
        Err(ActionError::MissingArg("amount").into())
    );
    assert_eq!(ui.revision(), 2);
    assert_eq!(Decision::from(false), Decision::Deny("denied".into()));
    let guard = ui.intercept(|_| false.into());
    for source in [
        ChangeSource::Program,
        ChangeSource::User,
        ChangeSource::Agent,
    ] {
        assert_eq!(
            h.invoke("add", (1,), source),
            Err(InvokeError::Denied("denied".into()))
        );
    }
    drop(guard);
    assert_eq!(ui.revision(), 2);
}
#[test]
fn callbacks_observe_committed_state() {
    let ui = Ui::new();
    let h = ui.mount(Counter::default());
    let read = h.clone();
    let sub = ui.subscribe(move |e| {
        assert_eq!(read.read(|c| c.0.get()), Some(3));
        assert_eq!(read.ui().component(e.id).unwrap().revision, e.revision);
    });
    h.invoke("add", (3,), ChangeSource::User).unwrap();
    drop(sub);
    let changed = h.clone();
    let hook = ui.intercept(move |_| {
        changed.update(ChangeSource::Program, |c| c.0.set(0));
        Decision::Allow
    });
    assert_eq!(
        h.invoke("reset", (), ChangeSource::User),
        Err(ActionError::Disabled("reset".into()).into())
    );
    drop(hook);
    let removed = h.clone();
    let _hook = ui.intercept(move |_| {
        removed.clone().unmount();
        Decision::Allow
    });
    assert!(matches!(
        h.invoke("add", (1,), ChangeSource::Program),
        Err(InvokeError::Resolve(_))
    ));
    assert_eq!(ui.revision(), 4);
}
#[test]
fn ref_resolution_priorities() {
    let ui = Ui::new();
    let a = ui.mount(Counter::default()).label("Score");
    assert_eq!(ui.resolve("counter"), Ok(a.id()));
    let b = ui.mount(Counter::default()).label("score");
    let ambiguous = |q: &str| {
        Err(ResolveError::Ambiguous {
            query: q.to_owned(),
            candidates: vec![a.id(), b.id()],
        })
    };
    assert_eq!(ui.resolve("Score"), Ok(a.id()));
    assert_eq!(ui.resolve("score"), Ok(b.id()));
    assert_eq!(ui.resolve("SCORE"), ambiguous("SCORE"));
    assert_eq!(ui.resolve("counter"), ambiguous("counter"));
    b.set_label("Score");
    assert_eq!(ui.resolve("Score"), ambiguous("Score"));
    b.set_label("COUNTER");
    assert_eq!(ui.resolve("counter"), Ok(b.id()));
    assert_eq!(a.id().to_string(), "@1");
    for s in ["1", "@1"] {
        assert_eq!(s.parse(), Ok(a.id()));
    }
    assert_eq!(ui.resolve(String::from("@1")), Ok(a.id()));
    assert_eq!(ui.resolve("1"), Err(ResolveError::NotFound("1".into())));
    a.unmount();
    assert_eq!(ui.resolve("@1"), Err(ResolveError::NotFound("@1".into())));
}
#[test]
fn bounded_removals() {
    let ui = Ui::new();
    for _ in 0..1025 {
        ui.mount(Counter::default()).unmount();
    }
    let changes = ui.changes_since(0);
    assert_eq!(changes.removed.len(), 1024);
    assert_eq!(changes.removed[0].get(), 2);
    assert!(changes.changed.is_empty());
}
#[test]
fn poisoned_component_recovers() {
    let ui = Ui::new();
    let h = ui.mount(Counter::default());
    assert!(
        catch_unwind(AssertUnwindSafe(
            || h.update(ChangeSource::User, |_| panic!("poison"))
        ))
        .is_err()
    );
    assert_eq!(h.read(|c| c.0.get()), Some(0));
    assert_eq!(
        h.invoke("add", (1,), ChangeSource::User).unwrap(),
        Value::from(1)
    );
}
#[test]
fn thread_safe_and_concurrent() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Ui>();
    assert_send_sync::<Handle<Counter>>();
    let ui = Ui::new();
    let h = ui.mount(Counter::default());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..100 {
                    h.invoke("add", (1,), ChangeSource::Agent).unwrap();
                }
            });
        }
    });
    assert_eq!(h.read(|c| c.0.get()), Some(400));
    assert_eq!(ui.component(h.id()).unwrap().revision, 401);
}
#[test]
fn query_window_and_argument_decoding() {
    let ui = Ui::new();
    let h = ui.mount(Counter(Cell::new(55)));
    let page = |from, limit| {
        ui.query(
            h.id(),
            &Query {
                text: None,
                from,
                limit,
            },
        )
        .unwrap()
        .unwrap()
    };
    assert_eq!(page(0, 0).items.len(), 50);
    assert_eq!(page(0, 100).next, Some(50));
    assert_eq!(
        page(50, 10),
        Page {
            items: (50..55).map(Value::from).collect(),
            total: 55,
            next: None
        }
    );
    assert!(page(usize::MAX, 10).items.is_empty());
    let args = Args::from((1, "two", true, 4.0));
    assert_eq!(args.len(), 4);
    assert_eq!(args.get_opt::<u32>(4, "x"), Ok(None));
    assert!(matches!(
        args.get_opt::<u32>(1, "x"),
        Err(ActionError::InvalidArg { name: "x", .. })
    ));
}
