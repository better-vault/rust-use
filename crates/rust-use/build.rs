//! Forward the plugin ACL to apps that depend on the facade alone.
fn main() {
    if let Ok(path) = std::env::var("DEP_TAURI_PLUGIN_RUST_USE_PERMISSION_FILES_PATH") {
        println!("cargo:PERMISSION_FILES_PATH={path}");
    }
}
