use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_swap::ArcSwap;
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::{self, ClipboardStore};
use crate::config::{self, LaunchifyConfig};
use crate::indexer::watcher::WatcherHandle;
use crate::indexer::IndexManager;
use crate::logging;
use crate::model::{FrecencyStore, IndexSnapshot};
use crate::search::engine::SearchEngine;
use crate::storage;
use crate::tray;
use crate::win32;
use crate::win32::hook::WinKeyHookHandle;

/// Logical width of the launcher window in DIPs.
pub const LAUNCHER_WIDTH: u32 = 680;
/// Minimum (idle) logical height of the launcher window in DIPs.
pub const LAUNCHER_MIN_HEIGHT: u32 = 56;
/// Maximum logical height of the launcher window in DIPs.
pub const LAUNCHER_MAX_HEIGHT: u32 = 520;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub app_dir: PathBuf,
    pub config: Arc<ArcSwap<LaunchifyConfig>>,
    pub search_engine: Arc<SearchEngine>,
    pub index_manager: Arc<IndexManager>,
    pub frecency: Mutex<FrecencyStore>,
    pub prev_foreground_hwnd: Mutex<Option<isize>>,
    pub hook_handle: Mutex<Option<WinKeyHookHandle>>,
    pub watcher_handle: Mutex<Option<WatcherHandle>>,
    pub watch_tx: crossbeam_channel::Sender<()>,
    pub last_query: Mutex<String>,
    pub clipboard_store: Arc<ClipboardStore>,
}

/// Returns the current Unix timestamp in seconds.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Resolves the Launchify data directory (`%LOCALAPPDATA%\Launchify` on Windows).
pub fn resolve_app_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            return PathBuf::from(local_app_data).join("Launchify");
        }
    }
    std::env::temp_dir().join("Launchify")
}

/// Clamps the requested launcher height to the allowed `[56, 520]` range.
pub fn clamp_launcher_height(height: u32) -> u32 {
    height.clamp(LAUNCHER_MIN_HEIGHT, LAUNCHER_MAX_HEIGHT)
}

/// Shows and focuses the launcher window, saving the previous foreground window.
pub fn show_launcher(app: &AppHandle, state: &AppState) {
    let prev = win32::window::save_foreground_window();
    if let Ok(mut guard) = state.prev_foreground_hwnd.lock() {
        *guard = prev;
    }

    if let Some(window) = app.get_webview_window("main") {
        #[cfg(windows)]
        if let Ok(hwnd) = window.hwnd() {
            let _ = win32::window::show_and_focus_launcher(hwnd.0 as isize, LAUNCHER_WIDTH as i32);
        }
        #[cfg(not(windows))]
        {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    let _ = app.emit("launcher://shown", ());
}

/// Hides the launcher window and restores focus to the previous foreground window.
pub fn hide_launcher(app: &AppHandle, state: &AppState) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(windows)]
        if let Ok(hwnd) = window.hwnd() {
            win32::window::hide_launcher_window(hwnd.0 as isize);
        }
        #[cfg(not(windows))]
        {
            let _ = window.hide();
        }
    }

    let prev = state
        .prev_foreground_hwnd
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    if let Some(hwnd) = prev {
        let _ = win32::window::restore_foreground_window(hwnd);
    }

    let _ = app.emit("launcher://hidden", ());
}

/// Initializes logging, config, index snapshot, frecency, hook, tray, and vibrancy.
pub fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = resolve_app_dir();
    let _ = logging::init_logging(Some(&app_dir.join("logs")));

    let mut cfg = config::load_or_create(&app_dir).unwrap_or_default();
    let currency_cache_path = app_dir.join("currency_rates.json");
    let cached_currency = storage::load_currency_cache(&currency_cache_path);
    if let Some(ref cache) = cached_currency {
        for (k, v) in &cache.rates {
            cfg.currency_rates.insert(k.clone(), *v);
        }
    }

    let index_path = app_dir.join("index.bin");
    let snapshot: IndexSnapshot = storage::load_index(&index_path).unwrap_or_default();
    let is_empty_index = snapshot.items.is_empty();
    let frecency = storage::load_frecency(&app_dir.join("frecency.json"));

    let snapshot_swap = Arc::new(ArcSwap::from_pointee(snapshot));
    let config_swap = Arc::new(ArcSwap::from_pointee(cfg));
    let search_engine = Arc::new(SearchEngine::from_arc(Arc::clone(&snapshot_swap)));
    let index_manager = Arc::new(IndexManager::new(
        snapshot_swap,
        Arc::clone(&config_swap),
        app_dir.clone(),
    ));

    let (trigger_tx, trigger_rx) = crossbeam_channel::bounded::<()>(4);
    let hook_handle = match WinKeyHookHandle::start(trigger_tx) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!("Failed to start WinKey hook: {e}");
            None
        }
    };

    let (watch_tx, watch_rx) = crossbeam_channel::bounded::<()>(16);
    let idx_for_watch = Arc::clone(&index_manager);
    let eng_for_watch = Arc::clone(&search_engine);
    let watcher_handle = crate::indexer::watcher::spawn_watcher(watch_rx, move || {
        match idx_for_watch.rebuild_now() {
            Ok(snap) => eng_for_watch.update_index(snap),
            Err(e) => tracing::warn!("Watcher index rebuild failed: {e}"),
        }
    });

    let clipboard_store = Arc::new(ClipboardStore::load_or_new(app_dir.join("clipboard.json")));

    let state = AppState {
        app_dir: app_dir.clone(),
        config: Arc::clone(&config_swap),
        search_engine: Arc::clone(&search_engine),
        index_manager: Arc::clone(&index_manager),
        frecency: Mutex::new(frecency),
        prev_foreground_hwnd: Mutex::new(None),
        hook_handle: Mutex::new(hook_handle),
        watcher_handle: Mutex::new(Some(watcher_handle)),
        watch_tx,
        last_query: Mutex::new(String::new()),
        clipboard_store: Arc::clone(&clipboard_store),
    };
    app.manage(state);

    clipboard::spawn_clipboard_watcher(Arc::clone(&clipboard_store));
    spawn_currency_refresh(currency_cache_path, cached_currency, Arc::clone(&config_swap));

    let handle_for_hook = app.handle().clone();
    std::thread::Builder::new()
        .name("launchify-hook-listener".into())
        .spawn(move || {
            while trigger_rx.recv().is_ok() {
                let handle = handle_for_hook.clone();
                if let Err(e) = handle_for_hook.run_on_main_thread(move || {
                    if let Some(st) = handle.try_state::<AppState>() {
                        let is_visible = handle
                            .get_webview_window("main")
                            .and_then(|w| w.is_visible().ok())
                            .unwrap_or(false);
                        if is_visible {
                            hide_launcher(&handle, &st);
                        } else {
                            show_launcher(&handle, &st);
                        }
                    }
                }) {
                    tracing::warn!("Failed to dispatch show_launcher to main thread: {e}");
                }
            }
        })?;

    if is_empty_index {
        let idx_mgr = Arc::clone(&index_manager);
        let engine = Arc::clone(&search_engine);
        std::thread::Builder::new()
            .name("launchify-initial-index".into())
            .spawn(move || match idx_mgr.rebuild_now() {
                Ok(snap) => engine.update_index(snap),
                Err(e) => tracing::warn!("Initial index build failed: {e}"),
            })?;
    }

    if let Err(e) = tray::setup_tray(app.handle()) {
        tracing::warn!("System tray setup failed: {e}");
    }

    if let Some(window) = app.get_webview_window("main") {
        #[cfg(windows)]
        {
            if let Ok(hwnd) = window.hwnd() {
                let _ = win32::window::apply_toolwindow_style(hwnd.0 as isize);
            }
        }
    }

    Ok(())
}

const CURRENCY_CACHE_TTL_SECS: u64 = 3600;
const CURRENCY_API_URL: &str = "https://open.er-api.com/v6/latest/USD";

#[derive(serde::Deserialize)]
struct ExchangeRateApiResponse {
    rates: std::collections::HashMap<String, f64>,
}

/// Spawns a background thread to refresh USD-base currency rates if missing or stale (>1h).
fn spawn_currency_refresh(
    cache_path: PathBuf,
    cached: Option<storage::CurrencyCache>,
    config_swap: Arc<ArcSwap<LaunchifyConfig>>,
) {
    let now = now_unix().max(0) as u64;
    let is_fresh = cached
        .as_ref()
        .is_some_and(|c| now.saturating_sub(c.updated_at_unix) < CURRENCY_CACHE_TTL_SECS);
    if is_fresh {
        return;
    }

    let _ = std::thread::Builder::new()
        .name("launchify-currency-refresh".into())
        .spawn(move || {
            let resp = match ureq::get(CURRENCY_API_URL)
                .timeout(std::time::Duration::from_secs(5))
                .call()
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Failed to fetch live currency rates: {e}");
                    return;
                }
            };

            let parsed: ExchangeRateApiResponse = match resp.into_json() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Failed to parse currency rates JSON: {e}");
                    return;
                }
            };

            if parsed.rates.is_empty() {
                tracing::warn!("Currency rates response was empty; keeping cached/static rates");
                return;
            }

            let cache = storage::CurrencyCache {
                updated_at_unix: now_unix().max(0) as u64,
                rates: parsed.rates,
            };

            if let Err(e) = storage::save_currency_cache_atomic(&cache_path, &cache) {
                tracing::warn!("Failed to save currency cache: {e}");
            }

            let mut updated = (**config_swap.load()).clone();
            for (k, v) in &cache.rates {
                updated.currency_rates.insert(k.clone(), *v);
            }
            config_swap.store(Arc::new(updated));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_height_bounds() {
        assert_eq!(clamp_launcher_height(0), LAUNCHER_MIN_HEIGHT);
        assert_eq!(clamp_launcher_height(56), 56);
        assert_eq!(clamp_launcher_height(240), 240);
        assert_eq!(clamp_launcher_height(999), LAUNCHER_MAX_HEIGHT);
    }

    #[test]
    fn resolve_app_dir_ends_with_launchify() {
        let dir = resolve_app_dir();
        assert_eq!(
            dir.file_name().and_then(|s| s.to_str()),
            Some("Launchify")
        );
    }
}
