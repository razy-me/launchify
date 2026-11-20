use tauri::image::Image;
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

    let builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Launchify")
        .icon(build_tray_icon())
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
                    let _ = state.watch_tx.try_send(());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        });

    builder
        .build(app)
        .map_err(|e| format!("failed to build tray icon: {e}"))?;

    Ok(())
}

/// Generates a crisp 32x32 RGBA tray icon (rounded dark badge with a white launch/bolt glyph).
fn build_tray_icon() -> Image<'static> {
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE as f32 - 1.0) * 0.5;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = (x as f32 - center).abs();
            let dy = (y as f32 - center).abs();
            // Rounded square SDF (radius 13.5, corner radius 6.0)
            let qx = (dx - 7.5).max(0.0);
            let qy = (dy - 7.5).max(0.0);
            let dist = (qx * qx + qy * qy).sqrt() - 6.0;

            let idx = (y * SIZE + x) * 4;
            if dist <= 0.5 {
                let alpha = ((0.5 - dist).clamp(0.0, 1.0) * 255.0) as u8;
                // Subtle light border on outer 1.2px
                if dist > -1.2 {
                    rgba[idx] = 110;
                    rgba[idx + 1] = 110;
                    rgba[idx + 2] = 125;
                    rgba[idx + 3] = alpha;
                } else {
                    rgba[idx] = 24;
                    rgba[idx + 1] = 24;
                    rgba[idx + 2] = 28;
                    rgba[idx + 3] = alpha;
                }
            }
        }
    }

    // Draw a crisp white lightning/launch glyph inside the badge
    let set_px = |buf: &mut [u8], px: i32, py: i32, r: u8, g: u8, b: u8| {
        if px >= 0 && px < SIZE as i32 && py >= 0 && py < SIZE as i32 {
            let i = (py as usize * SIZE + px as usize) * 4;
            buf[i] = r;
            buf[i + 1] = g;
            buf[i + 2] = b;
            buf[i + 3] = 255;
        }
    };

    // Upper-right to center-left bolt
    let glyph_rows: &[(i32, i32, i32)] = &[
        (7, 17, 19),
        (8, 16, 19),
        (9, 15, 18),
        (10, 14, 18),
        (11, 13, 17),
        (12, 12, 17),
        (13, 11, 21),
        (14, 10, 21),
        (15, 10, 20),
        (16, 14, 20),
        (17, 14, 19),
        (18, 13, 18),
        (19, 13, 17),
        (20, 12, 16),
        (21, 12, 15),
        (22, 11, 14),
        (23, 11, 13),
        (24, 11, 12),
    ];
    for &(y, x0, x1) in glyph_rows {
        for x in x0..=x1 {
            set_px(&mut rgba, x, y, 244, 244, 248);
        }
    }

    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}
