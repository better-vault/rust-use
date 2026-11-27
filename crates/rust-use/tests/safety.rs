//! Agent visibility and human approval at the facade and registry entry points.
#![cfg(all(feature = "agent", feature = "macros"))]
#![allow(clippy::unwrap_used)]

use rust_use::{InvokeError, ResolveError, prelude::*};
use std::{sync::mpsc, thread, time::Duration};

#[derive(Default)]
struct Credential;

#[derive(Default, Primitive)]
#[primitive(hidden)]
struct Project {
    #[agent(value, secret)]
    _token: Credential,
    #[agent(secret, rename = "key")]
    _key: Credential,
    count: u32,
    #[agent(skip)]
    last_deleted: Option<String>,
}

#[actions]
impl Project {
    /// Delete a project.
    #[action(confirm = "Delete the project")]
    fn delete(&mut self, id: String) {
        self.count += 1;
        self.last_deleted = Some(id);
    }

    /// Record a visit.
    fn visit(&mut self) {
        self.count += 1;
    }
}

#[test]
fn hidden_components_and_redacted_fields() {
    let ui = Ui::new();
    let project = ui.mount(Project::default()).label("Shared");
    let other = ui.mount(Project::default()).label("Shared");
    let agent = ui.agent();
    assert!(ui.component(project.id()).unwrap().hidden);
    assert!(agent.snapshot().components.is_empty());
    assert!(
        agent
            .search(None, Some("shared"), SnapshotOptions::default())
            .components
            .is_empty()
    );
    assert_eq!(agent.handle("diff 0"), "rev 4");
    assert!(matches!(
        agent.query(project.id(), None, 0),
        Err(ResolveError::NotFound(_))
    ));
    assert!(matches!(
        project.invoke("visit", (), ChangeSource::Agent),
        Err(InvokeError::Resolve(ResolveError::NotFound(_)))
    ));
    project.show_to_agents();
    project.show_to_agents();
    let visible = "rev 5\n@1 project \"Shared\" =\"[redacted]\" key=\"[redacted]\" count=0 !!delete(id) !visit";
    assert_eq!(agent.diff().to_string(), visible);
    assert_eq!(agent.snapshot().to_string(), visible);
    assert!(
        agent
            .handle("snapshot --verbose")
            .contains("# Delete a project. (needs a person: Delete the project)")
    );
    for target in ["shared", "Shared", "project"] {
        assert!(ui.invoke(target, "visit", (), ChangeSource::Agent).is_ok());
    }
    agent.snapshot();
    project.hide_from_agents();
    project.hide_from_agents();
    assert_eq!(agent.diff().to_string(), "rev 9\n-@1");
    other.unmount();
    project.unmount();
    assert_eq!(agent.diff().to_string(), "rev 11");
}

#[test]
fn confirm_requires_a_person_for_agent_only() {
    let ui = Ui::new();
    let h = ui.mount(Project::default());
    h.show_to_agents();
    let call = |source| h.invoke("delete", ("x",), source);
    assert_eq!(
        call(ChangeSource::Agent),
        Err(InvokeError::Denied(
            "needs a person: Delete the project".into()
        ))
    );
    assert!(call(ChangeSource::User).is_ok());
    let approvals = Approvals::new().timeout(Duration::from_millis(50));
    let _guard = ui.require_approval(&approvals);
    assert_eq!(
        call(ChangeSource::Agent),
        Err(InvokeError::Denied(
            "no answer from a person within 50ms".into()
        ))
    );
    assert!(approvals.pending().is_empty());
    assert_eq!(h.read(|p| p.count), Some(1));
}

#[test]
fn waiting_releases_locks_and_rechecks_visibility() {
    let ui = Ui::new();
    let target = ui.mount(Project::default()).label("Work");
    let other = ui.mount(Project::default());
    target.show_to_agents();
    other.show_to_agents();
    let approvals = Approvals::new().timeout(Duration::from_secs(3));
    let (tx, rx) = mpsc::channel();
    let queue = approvals.clone();
    let _listener = approvals.on_request(move |request| {
        assert_eq!(queue.pending()[0].id, request.id);
        tx.send(request.clone()).unwrap();
    });
    let _hook = ui.intercept(|request| {
        if request.action == "delete" {
            assert_eq!(request.confirm, Some("Delete the project"));
        }
        Decision::Allow
    });
    thread::scope(|s| {
        for answer in ["yes", "no", "hide", "off", "replace"] {
            target.show_to_agents();
            let guard = ui.require_approval(&approvals);
            let worker = s.spawn(|| ui.agent().handle("act Work delete x"));
            let request = rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(request.label.as_deref(), Some("Work"));
            assert_eq!(request.component, target.id());
            assert_eq!(request.args.get::<String>(0, "id").unwrap(), "x");
            assert_eq!(ui.agent().snapshot().components.len(), 2);
            assert_eq!(target.read(|p| p.count), Some(u32::from(answer != "yes")));
            other.invoke("visit", (), ChangeSource::Agent).unwrap();
            let expected = match answer {
                "yes" => {
                    assert!(approvals.approve(request.id));
                    "ok"
                }
                "no" => {
                    assert!(approvals.deny(request.id, "keep it"));
                    "denied: keep it"
                }
                "hide" => {
                    target.hide_from_agents();
                    assert!(approvals.approve(request.id));
                    "no component matches"
                }
                "replace" => {
                    let _replacement = ui.require_approval(&approvals);
                    "denied: approvals were turned off"
                }
                _ => {
                    drop(guard);
                    "denied: approvals were turned off"
                }
            };
            let reply = worker.join().unwrap();
            assert!(reply.contains(expected), "{reply}");
            assert!(!approvals.approve(request.id));
        }
    });
    assert_eq!(target.read(|p| p.count), Some(1));
}
