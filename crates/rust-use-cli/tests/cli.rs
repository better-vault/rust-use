//! Drive a live app through independent CLI processes.
use rust_use_agent::ipc::{Endpoint, Server};
use rust_use_core::{ActionError, ActionSpec, Args, Behavior, Describe, Primitive, Ui, Value};
use std::process::Command;

struct Button(bool);
impl Primitive for Button {
    const NAME: &'static str = "button";
}
impl Behavior for Button {
    fn describe(&self, d: &mut Describe) {
        d.value(self.0);
    }
    fn actions(&self) -> Vec<ActionSpec> {
        vec![ActionSpec {
            name: "press",
            params: &[],
            doc: "Press.",
        }]
    }
    fn invoke(&mut self, name: &str, _: &Args) -> Option<Result<Value, ActionError>> {
        (name == "press").then(|| {
            self.0 = true;
            Ok(Value::Null)
        })
    }
}

#[test]
fn cli_session() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = Endpoint::new(&format!("cli-{}", std::process::id()))?;
    let ui = Ui::new();
    ui.mount(Button(false));
    let guard = Server::new(&ui)
        .endpoint(endpoint.clone())
        .command("echo", str::to_owned)
        .spawn()?;
    let run = |args: &[&str], code| -> Result<String, Box<dyn std::error::Error>> {
        let out = Command::new(env!("CARGO_BIN_EXE_rust-use"))
            .args(["--app", endpoint.name()])
            .args(args)
            .output()?;
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        if code == 2 {
            assert!(out.stdout.is_empty());
            return Ok(String::from_utf8(out.stderr)?);
        }
        assert!(out.stderr.is_empty());
        Ok(String::from_utf8(out.stdout)?)
    };
    assert_eq!(run(&["snapshot"], 0)?, "rev 1\n@1 button =false !press");
    assert_eq!(
        run(&["act", "@1", "press"], 0)?,
        "ok\nrev 2\n@1 button =true !press"
    );
    ui.mount(Button(false));
    assert_eq!(run(&["diff"], 0)?, "rev 3\n@2 button =false !press");
    assert_eq!(run(&["diff"], 0)?, "rev 3");
    assert_eq!(run(&["query", "@1"], 0)?, "not queryable");
    assert_eq!(run(&["raw", "echo", "two", "words"], 0)?, "two words");
    assert!(run(&["act", "@1", "missing"], 1)?.starts_with("err 1"));
    drop(guard);
    let diagnostic = run(&["snapshot"], 2)?;
    assert_eq!(diagnostic.lines().count(), 1);
    assert!(diagnostic.contains("ipc"));
    std::fs::remove_file(endpoint.revision_path())?;
    Ok(())
}
