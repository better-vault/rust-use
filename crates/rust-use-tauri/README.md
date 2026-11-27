# rust-use for Tauri

Enable the `tauri` feature on `rust-use` to expose a mounted `Ui` to webview controls and agents. The adapter accepts the application's Tauri runtime. Its tests use Tauri's mock runtime.

In `main.rs`, pass your registry to the plugin. This example assumes your app defines a `MuteButton` primitive with a boolean primary value and a `toggle` action.

```rust,ignore
fn main() -> tauri::Result<()> {
    let ui = rust_use::Ui::new();
    ui.mount(MuteButton::default()).label("Mute");
    tauri::Builder::default().plugin(rust_use::tauri::init(ui)).run(tauri::generate_context!())
}
```

Include the plugin and event permissions in the local window's capability:

```json
"permissions": ["core:event:default", "rust-use:default"]
```

Set `app.withGlobalTauri` to `true` in `tauri.conf.json`. Place this module in the page with the toggle button:

```html
<button id="mute" type="button" disabled>Mute</button>
<script type="module">
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const call = (name, args) => invoke(`plugin:rust-use|${name}`, args);
  const button = document.querySelector("#mute");
  let target;
  let revision = -1;

  async function render() {
    const screen = await call("snapshot_json");
    if (screen.revision < revision) return;
    revision = screen.revision;
    const mute = screen.components.find(c => c.label === "Mute");
    target = mute && `@${mute.id}`;
    button.disabled = !mute?.actions.some(a => a.name === "toggle");
    button.setAttribute("aria-pressed", String(mute?.value === true));
    button.textContent = mute?.value ? "Unmute" : "Mute";
  }

  button.addEventListener("click", async () => {
    await call("invoke", { target, action: "toggle", args: [] });
  });
  await listen("rust-use://changed", render);
  await render();
</script>
```

`snapshot` returns compact text and `snapshot_json` returns structured data. `invoke` dispatches a user action and returns its JSON value. `act({ script })` dispatches agent actions and returns the compact success or error reply. `query({ target, text, from })` returns the compact options page. Both `text` and `from` are optional. Changes from any source trigger `rust-use://changed` with the latest revision in the batch and its distinct component IDs.

Configure agent permissions with `rust_use::tauri::builder(ui).allow(predicate).build()`. Both action paths run registry interceptors, and the builder's predicate applies to agent actions. With the facade's `ipc` feature enabled, add `.serve_ipc("settings")` before `.build()` to also serve the registry to `rust-use --app settings`.
