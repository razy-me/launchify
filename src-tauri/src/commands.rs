use tauri::{AppHandle, LogicalSize, Manager, State};

use crate::app::{self, clamp_launcher_height, now_unix, AppState, LAUNCHER_WIDTH};
use crate::config::ExclusionFilter;
use crate::frecency::{normalize_prefix, record_selection};
use crate::inline_tools;
use crate::model::{
    ActionKind, ExecuteOutcome, ItemKind, PrefixMode, QueryResponse, SearchResult,
};
use crate::path_nav;
use crate::storage;
use crate::win32;
use crate::win32::executor::LaunchMode;

/// Splits a raw query string into its `PrefixMode` and the remaining query text.
pub fn parse_prefix_mode(raw: &str) -> (PrefixMode, &str) {
    let trimmed = raw.trim_start();
    if let Some(rest) = trimmed.strip_prefix('>') {
        (PrefixMode::Shell, rest.trim_start())
    } else if let Some(rest) = trimmed.strip_prefix('=') {
        (PrefixMode::Calc, rest.trim_start())
    } else if let Some(rest) = trimmed.strip_prefix('/') {
        (PrefixMode::Path, rest)
    } else if let Some(rest) = trimmed.strip_prefix('@') {
        (PrefixMode::Window, rest.trim_start())
    } else {
        (PrefixMode::Global, trimmed)
    }
}

/// Evaluates a launcher query across prefix modes, inline tools, and fuzzy search.
#[tauri::command]
pub fn query(raw: String, state: State<'_, AppState>) -> Result<QueryResponse, String> {
    if let Ok(mut last) = state.last_query.lock() {
        *last = raw.clone();
    }

    let cfg = state.config.load();
    let (mode, rest) = parse_prefix_mode(&raw);

    match mode {
        PrefixMode::Shell => {
            let results = if rest.is_empty() {
                Vec::new()
            } else {
                vec![SearchResult {
                    id: format!("shell:{rest}"),
                    title: rest.to_string(),
                    subtitle: "Run shell command (Shift/Ctrl+Enter for Terminal)".to_string(),
                    kind: ItemKind::Workflow,
                    icon_path: None,
                    secondary_hint: Some("Open in Terminal".to_string()),
                    score: 1.0,
                    autocomplete_text: Some(format!("> {rest}")),
                }]
            };
            Ok(QueryResponse {
                mode,
                results,
                inline_preview: None,
            })
        }
        PrefixMode::Calc => {
            let inline_preview = inline_tools::evaluate(rest, true, &cfg.currency_rates);
            Ok(QueryResponse {
                mode,
                results: Vec::new(),
                inline_preview,
            })
        }
        PrefixMode::Path => {
            let exclusions = ExclusionFilter::new(&cfg.extra_exclusions);
            let results = path_nav::browse_path(rest, &exclusions, cfg.max_results);
            Ok(QueryResponse {
                mode,
                results,
                inline_preview: None,
            })
        }
        PrefixMode::Window => {
            let needle = rest.to_lowercase();
            let windows = win32::switcher::enumerate_visible_windows();
            let results = windows
                .into_iter()
                .filter(|w| {
                    needle.is_empty()
                        || w.title.to_lowercase().contains(&needle)
                        || w.process_name.to_lowercase().contains(&needle)
                })
                .take(cfg.max_results)
                .map(|w| SearchResult {
                    id: format!("win:{}", w.hwnd),
                    title: w.title,
                    subtitle: w.process_name,
                    kind: ItemKind::Window,
                    icon_path: None,
                    secondary_hint: None,
                    score: 1.0,
                    autocomplete_text: None,
                })
                .collect();
            Ok(QueryResponse {
                mode,
                results,
                inline_preview: None,
            })
        }
        PrefixMode::Global => {
            if rest.is_empty() {
                return Ok(QueryResponse {
                    mode,
                    results: Vec::new(),
                    inline_preview: None,
                });
            }
            let inline_preview = inline_tools::evaluate(rest, false, &cfg.currency_rates);
            let frecency = state
                .frecency
                .lock()
                .map_err(|_| "frecency lock poisoned".to_string())?;
            let results = state.search_engine.search(
                rest,
                &frecency,
                cfg.max_results,
                cfg.prefix_memory_threshold,
                now_unix(),
            );
            Ok(QueryResponse {
                mode,
                results,
                inline_preview,
            })
        }
    }
}

/// Executes the selected item's primary or secondary action and hides the launcher.
#[tauri::command]
pub fn execute(
    item_id: String,
    action: ActionKind,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExecuteOutcome, String> {
    let secondary = action == ActionKind::Secondary;

    if !item_id.is_empty() && !item_id.starts_with("win:") && !item_id.starts_with("shell:") {
        let prefix = state
            .last_query
            .lock()
            .map(|q| normalize_prefix(&q))
            .unwrap_or_default();
        if let Ok(mut frecency) = state.frecency.lock() {
            record_selection(&mut frecency, &prefix, &item_id, now_unix());
            let _ = storage::save_frecency_atomic(&state.app_dir.join("frecency.json"), &frecency);
        }
    }

    let outcome = if let Some(cmd) = item_id.strip_prefix("shell:") {
        let mode = if secondary {
            LaunchMode::ShellTerminal
        } else {
            LaunchMode::ShellHidden
        };
        win32::executor::execute_target(cmd, mode)?;
        ExecuteOutcome::Launched
    } else if let Some(hwnd_str) = item_id.strip_prefix("win:") {
        let hwnd = hwnd_str
            .parse::<isize>()
            .map_err(|_| format!("invalid window handle: {hwnd_str}"))?;
        win32::switcher::focus_window(hwnd)?;
        ExecuteOutcome::WindowFocused
    } else if let Some(copy_val) = item_id.strip_prefix("copy:") {
        win32::executor::copy_text_to_clipboard(copy_val)?;
        ExecuteOutcome::CopiedToClipboard {
            value: copy_val.to_string(),
        }
    } else {
        let snapshot = state.search_engine.snapshot();
        if let Some(item) = snapshot.items.iter().find(|i| i.id == item_id) {
            if let Some(power_cmd) = item.target.strip_prefix("power:") {
                win32::executor::system_power_action(power_cmd)?;
                ExecuteOutcome::Launched
            } else {
                match item.kind {
                    ItemKind::Folder => {
                        let mode = if secondary {
                            LaunchMode::RevealInExplorer
                        } else {
                            LaunchMode::Normal
                        };
                        win32::executor::execute_target(&item.target, mode)?;
                        ExecuteOutcome::Launched
                    }
                    ItemKind::File => {
                        if secondary {
                            win32::executor::copy_text_to_clipboard(&item.target)?;
                            ExecuteOutcome::CopiedToClipboard {
                                value: item.target.clone(),
                            }
                        } else {
                            win32::executor::execute_target(&item.target, LaunchMode::Normal)?;
                            ExecuteOutcome::Launched
                        }
                    }
                    _ => {
                        let mode = if secondary {
                            LaunchMode::Elevated
                        } else {
                            LaunchMode::Normal
                        };
                        win32::executor::execute_target(&item.target, mode)?;
                        ExecuteOutcome::Launched
                    }
                }
            }
        } else if secondary {
            win32::executor::copy_text_to_clipboard(&item_id)?;
            ExecuteOutcome::CopiedToClipboard { value: item_id }
        } else {
            win32::executor::execute_target(&item_id, LaunchMode::Normal)?;
            ExecuteOutcome::Launched
        }
    };

    app::hide_launcher(&app, &state);
    Ok(outcome)
}

/// Autocompletes the current query using filesystem path completion or the top result title.
#[tauri::command]
pub fn autocomplete(
    raw: String,
    top_item_title: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let (mode, rest) = parse_prefix_mode(&raw);
    if mode == PrefixMode::Path {
        let cfg = state.config.load();
        let exclusions = ExclusionFilter::new(&cfg.extra_exclusions);
        return Ok(path_nav::complete_path(rest, &exclusions).map(|p| format!("/{p}")));
    }
    Ok(top_item_title)
}

/// Hides the launcher window and restores focus to the previous foreground window.
#[tauri::command]
pub fn hide_launcher(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    app::hide_launcher(&app, &state);
    Ok(())
}

/// Resizes the launcher window height, clamped to `[56, 520]` logical pixels.
#[tauri::command]
pub fn resize_launcher(height: u32, app: AppHandle) -> Result<(), String> {
    let clamped = clamp_launcher_height(height);
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_size(LogicalSize::new(LAUNCHER_WIDTH as f64, clamped as f64))
            .map_err(|e| format!("failed to resize launcher: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prefix_modes() {
        assert_eq!(parse_prefix_mode("> ipconfig"), (PrefixMode::Shell, "ipconfig"));
        assert_eq!(parse_prefix_mode("= 1920*0.75"), (PrefixMode::Calc, "1920*0.75"));
        assert_eq!(parse_prefix_mode("/C:\\Tools"), (PrefixMode::Path, "C:\\Tools"));
        assert_eq!(parse_prefix_mode("@ code"), (PrefixMode::Window, "code"));
        assert_eq!(parse_prefix_mode("spotify"), (PrefixMode::Global, "spotify"));
    }
}
