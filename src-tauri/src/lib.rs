pub mod app;
pub mod catalog;
pub mod commands;
pub mod config;
pub mod frecency;
pub mod icons;
pub mod indexer;
pub mod inline_tools;
pub mod logging;
pub mod model;
pub mod path_nav;
pub mod search;
pub mod storage;
pub mod tray;
pub mod win32;

/// Starts the Launchify Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(app::setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::query,
            commands::execute,
            commands::autocomplete,
            commands::hide_launcher,
            commands::resize_launcher,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
