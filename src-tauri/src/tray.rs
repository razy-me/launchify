use std::sync::Arc;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::app::AppState;
use crate::win32;
use crate::win32::executor::LaunchMode;

/// Sets up the Launchify system tray icon and menu (Open Config, Re-index, Quit).
pub fn setup_tray(app: &AppHandle) -> Result<(), String> {
    let open_config = MenuItemBuilder::with_id("open_config", "Open Config")
        .build(app)
        .map_err(|e| format!("failed to build open_config menu item: {e}"))?;
    let reindex = MenuItemBuilder::with_id("reindex", "Re-index")
        .build(app)
        .map_err(|e| format!("failed to build reindex menu item: {e}"))?;
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .build(app)
        .map_err(|e| format!("failed to build quit menu item: {e}"))?;

    let menu = MenuBuilder::new(app)
        .items(&[&open_config, &reindex, &quit])
        .build()
        .map_err(|e| format!("failed to build tray menu: {e}"))?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Launchify")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_config" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let config_path = state.app_dir.join("config.json");
                    let _ = win32::executor::execute_target(
                        &config_path.to_string_lossy(),
                        LaunchMode::Normal,
                    );
                }
            }
            "reindex" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let idx_mgr = Arc::clone(&state.index_manager);
                    let engine = Arc::clone(&state.search_engine);
                    std::thread::spawn(move || match idx_mgr.rebuild_now() {
                        Ok(snap) => engine.update_index(snap),
                        Err(e) => tracing::warn!("Manual re-index failed: {e}"),
                    });
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .build(app)
        .map_err(|e| format!("failed to build tray icon: {e}"))?;

    Ok(())
}
