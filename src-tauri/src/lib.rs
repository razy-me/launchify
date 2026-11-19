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

#[cfg(windows)]
fn acquire_single_instance() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    unsafe {
        let handle = CreateMutexW(None, false, w!("Local\\LaunchifySingleInstance")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            return None;
        }
        Some(handle)
    }
}

/// Starts the Launchify Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    let _instance_guard = match acquire_single_instance() {
        Some(h) => h,
        None => return,
    };

    tauri::Builder::default()
        .setup(app::setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::query,
            commands::execute,
            commands::autocomplete,
            commands::hide_launcher,
            commands::resize_launcher,
            commands::collapse_to_row,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
