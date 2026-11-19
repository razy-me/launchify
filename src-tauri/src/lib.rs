pub mod catalog;
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
pub mod win32;

/// Starts the Launchify Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
