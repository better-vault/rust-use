//! Generate command permissions for the Tauri capability system.
const COMMANDS: &[&str] = &["snapshot", "snapshot_json", "act", "invoke", "query"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
