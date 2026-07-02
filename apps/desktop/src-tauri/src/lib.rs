//! Tauri v2 entry point. Wraps the SvelteKit static build (`../build`) in a
//! native window. The same static assets are also embedded into `wipe-daemon`
//! and served by `wipe serve`.

/// Run the desktop application.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running wipe desktop application");
}
