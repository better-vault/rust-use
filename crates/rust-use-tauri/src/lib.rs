#![doc = include_str!("../README.md")]
// Tauri injects State by value into command handlers.
#![allow(clippy::needless_pass_by_value)]

use rust_use_agent::Agent;
use rust_use_core::{ActionRequest, ChangeSource, Subscription, Ui};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{Mutex, mpsc},
    thread,
};
use tauri::{Emitter, Manager, Runtime, State, plugin::TauriPlugin};

/// A Tauri plugin that serves a rust-use registry to the webview and to agents.
pub fn init<R: Runtime>(ui: Ui) -> TauriPlugin<R> {
    builder(ui).build()
}

/// Configure how the plugin serves the registry.
pub fn builder(ui: Ui) -> PluginBuilder {
    PluginBuilder {
        agent: Agent::new(&ui),
        ui,
        #[cfg(feature = "ipc")]
        endpoint: None,
    }
}

/// Options for the `rust-use` plugin.
#[must_use]
pub struct PluginBuilder {
    ui: Ui,
    agent: Agent,
    #[cfg(feature = "ipc")]
    endpoint: Option<String>,
}

impl PluginBuilder {
    /// Also serve the registry to local IPC clients under this app name.
    #[cfg(feature = "ipc")]
    pub fn serve_ipc(mut self, name: &str) -> Self {
        self.endpoint = Some(name.to_owned());
        self
    }

    /// Require the predicate for every agent action on this registry.
    pub fn allow(mut self, f: impl Fn(&ActionRequest<'_>) -> bool + Send + Sync + 'static) -> Self {
        self.agent = self.agent.allow(f);
        self
    }

    /// Build the plugin for the application's runtime.
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        tauri::plugin::Builder::<R>::new("rust-use")
            .invoke_handler(tauri::generate_handler![#![plugin(rust_use)] snapshot, snapshot_json, act, invoke, query])
            .setup(move |app, _| self.setup(app))
            .on_event(|app, event| {
                if matches!(event, tauri::RunEvent::Exit) {
                    stop_services(app);
                }
            })
            .on_drop(|app| stop_services(&app))
            .build()
    }

    fn setup<R: Runtime>(
        self,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "ipc")]
        let server = self
            .endpoint
            .map(|name| {
                use rust_use_agent::ipc::{Endpoint, Server};
                Server::new(&self.ui)
                    .endpoint(Endpoint::new(&name)?)
                    .spawn()
            })
            .transpose()?;
        let subscription = watch(app, &self.ui)?;
        if !app.manage(self.ui) {
            return Err("rust-use requires its Ui to be the managed Ui".into());
        }
        app.manage(Bridge {
            agent: self.agent,
            services: Mutex::new(Some(Services {
                _subscription: subscription,
                #[cfg(feature = "ipc")]
                _server: server,
            })),
        });
        Ok(())
    }
}

struct Bridge {
    agent: Agent,
    services: Mutex<Option<Services>>,
}

struct Services {
    _subscription: Subscription,
    #[cfg(feature = "ipc")]
    _server: Option<rust_use_agent::ipc::ServerGuard>,
}

// Disconnect the worker even while its AppHandle keeps managed state alive.
fn stop_services<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(bridge) = app.try_state::<Bridge>() {
        bridge
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

fn watch<R: Runtime>(app: &tauri::AppHandle<R>, ui: &Ui) -> std::io::Result<Subscription> {
    let (tx, rx) = mpsc::channel();
    let subscription = ui.subscribe(move |e| {
        let _ = tx.send((e.revision, e.id.get()));
    });
    let app = app.clone();
    thread::Builder::new()
        .name("rust-use-changed".into())
        .spawn(move || {
            let mut revision = 0;
            while let Ok(first) = rx.recv() {
                let mut ids = BTreeSet::new();
                for (rev, id) in std::iter::once(first).chain(rx.try_iter()) {
                    revision = revision.max(rev);
                    ids.insert(id);
                }
                let _ = app.emit(
                    "rust-use://changed",
                    json!({ "revision": revision, "ids": ids }),
                );
            }
        })?;
    Ok(subscription)
}

#[tauri::command]
fn snapshot(bridge: State<'_, Bridge>) -> String {
    bridge.agent.snapshot().to_string()
}

#[tauri::command]
fn snapshot_json(bridge: State<'_, Bridge>) -> Value {
    bridge.agent.snapshot().to_json()
}

#[tauri::command]
async fn act<R: Runtime>(app: tauri::AppHandle<R>, script: String) -> Result<String, String> {
    // Approval may block while a user responds on the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Bridge>()
            .agent
            .act(script)
            .map_or_else(|e| e.to_reply(), |out| out.to_string())
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn invoke(
    ui: State<'_, Ui>,
    target: String,
    action: &str,
    args: Vec<Value>,
) -> Result<Value, String> {
    ui.invoke(target, action, args, ChangeSource::User)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn query(
    bridge: State<'_, Bridge>,
    target: String,
    text: Option<&str>,
    from: Option<usize>,
) -> String {
    bridge
        .agent
        .query(target, text, from.unwrap_or_default())
        .map_or_else(|e| format!("err {e}"), |out| out.to_string())
}
