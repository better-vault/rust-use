//! Exercise the plugin through a mock Tauri window.
use rust_use_core::{ChangeSource, Ui};
use rust_use_macros::{Primitive, actions};
use serde_json::{Value, json};
use std::{error::Error, sync::mpsc, thread, time::Duration};
use tauri::{
    Listener, WebviewWindow, WebviewWindowBuilder,
    ipc::{CallbackFn, InvokeBody},
    test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets},
    webview::InvokeRequest,
};

#[derive(Default, Primitive)]
struct Switch {
    #[agent(value)]
    on: bool,
}

#[actions]
impl Switch {
    /// Set the switch and return its new value.
    fn set(&mut self, on: bool) -> bool {
        self.on = on;
        on
    }
}

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn app(plugin: tauri::plugin::TauriPlugin<MockRuntime>) -> TestResult<tauri::App<MockRuntime>> {
    let mut context = mock_context(noop_assets());
    for command in ["snapshot", "snapshot_json", "act", "invoke", "query"] {
        context.runtime_authority_mut().__allow_command(
            format!("plugin:rust-use|{command}"),
            tauri::utils::acl::ExecutionContext::Local,
        );
    }
    Ok(mock_builder().plugin(plugin).build(context)?)
}

fn call(window: &WebviewWindow<MockRuntime>, command: &str, body: Value) -> TestResult<Value> {
    Ok(get_ipc_response(
        window,
        InvokeRequest {
            cmd: format!("plugin:rust-use|{command}"),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()?,
            body: InvokeBody::Json(body),
            headers: tauri::http::HeaderMap::new(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map_err(|e| e.to_string())?
    .deserialize()?)
}

#[test]
fn commands_share_state_and_preserve_action_sources() -> TestResult {
    let ui = Ui::new();
    let switch = ui.mount(Switch::default());
    let (tx, rx) = mpsc::channel();
    let _subscription = ui.subscribe(move |event| {
        let _ = tx.send(event.source);
    });
    let app = app(rust_use_tauri::init(ui.clone()))?;
    let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default()).build()?;
    assert_eq!(
        call(&window, "snapshot", json!({}))?,
        "rev 1\n@1 switch =false !set(on:bool)"
    );
    assert_eq!(
        call(
            &window,
            "invoke",
            json!({"target":switch.id().to_string(),"action":"set","args":[true]})
        )?,
        true
    );
    assert_eq!(rx.try_recv()?, ChangeSource::User);
    assert_eq!(
        call(&window, "snapshot_json", json!({}))?["components"][0]["value"],
        true
    );
    assert!(
        call(&window, "act", json!({"script":"@1 set false"}))?
            .as_str()
            .is_some_and(|s| s.starts_with("ok"))
    );
    assert_eq!(rx.try_recv()?, ChangeSource::Agent);
    assert_eq!(switch.read(|s| s.on), Some(false));
    assert!(call(&window, "query", json!({"target":"@1"}))?.is_string());
    Ok(())
}

#[test]
fn allow_denies_agent_script() -> TestResult {
    let ui = Ui::new();
    let switch = ui.mount(Switch::default());
    let app = app(rust_use_tauri::builder(ui).allow(|_| false).build())?;
    let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default()).build()?;
    let reply = call(&window, "act", json!({"script":"@1 set true"}))?;
    assert!(
        reply
            .as_str()
            .is_some_and(|s| s.starts_with("err") && s.contains("denied"))
    );
    assert_eq!(switch.read(|s| s.on), Some(false));
    Ok(())
}

#[test]
fn emits_cross_thread_changes() -> TestResult {
    let ui = Ui::new();
    let switch = ui.mount(Switch::default());
    let id = switch.id().get();
    let app = app(rust_use_tauri::init(ui.clone()))?;
    let (tx, rx) = mpsc::channel();
    app.listen("rust-use://changed", move |event| {
        let _ = tx.send(event.payload().to_owned());
    });
    thread::spawn(move || switch.invoke("set", vec![json!(true)], ChangeSource::Program))
        .join()
        .map_err(|_| "switch thread panicked")??;
    let payload: Value = serde_json::from_str(&rx.recv_timeout(Duration::from_secs(3))?)?;
    assert_eq!(payload, json!({"revision":ui.revision(), "ids":[id]}));
    assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
    Ok(())
}
