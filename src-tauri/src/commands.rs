use base64::Engine as _;
use tauri::{AppHandle, LogicalSize, Manager, State};

use crate::app::{self, clamp_launcher_height, now_unix, AppState, LAUNCHER_WIDTH};
use crate::config::ExclusionFilter;
use crate::find_files;
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
    } else if trimmed.eq_ignore_ascii_case("convert") {
        (PrefixMode::Convert, "")
    } else if trimmed
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("convert "))
    {
        (PrefixMode::Convert, trimmed[8..].trim_start())
    } else if trimmed.eq_ignore_ascii_case("kill") {
        (PrefixMode::Kill, "")
    } else if trimmed
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("kill "))
    {
        (PrefixMode::Kill, trimmed[5..].trim_start())
    } else if trimmed.eq_ignore_ascii_case("cb") || trimmed.eq_ignore_ascii_case("clip") {
        (PrefixMode::Clipboard, "")
    } else if trimmed
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case("cb "))
    {
        (PrefixMode::Clipboard, trimmed[3..].trim_start())
    } else if trimmed
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("clip "))
    {
        (PrefixMode::Clipboard, trimmed[5..].trim_start())
    } else {
        (PrefixMode::Global, trimmed)
    }
}

fn match_web_shortcut(
    raw: &str,
    shortcuts: &[crate::config::WebShortcutConfig],
) -> Option<SearchResult> {
    let trimmed = raw.trim_start();
    let (bang, rest) = trimmed.split_once(char::is_whitespace)?;
    let q = rest.trim();
    if q.is_empty() {
        return None;
    }
    let shortcut = shortcuts
        .iter()
        .find(|s| s.keyword.eq_ignore_ascii_case(bang))?;
    let url = shortcut
        .url_template
        .replace("{query}", &percent_encode_query(q));
    Some(SearchResult {
        id: format!("web:{url}"),
        title: format!("Search {} for '{}'", shortcut.name, q),
        subtitle: url,
        kind: ItemKind::WebShortcut,
        icon_path: None,
        secondary_hint: None,
        score: 1000.0,
        autocomplete_text: None,
    })
}

fn extract_find_rest(trimmed_raw: &str) -> Option<&str> {
    if trimmed_raw.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("find:")) {
        Some(trimmed_raw[5..].trim_start())
    } else if trimmed_raw.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("f:")) {
        Some(trimmed_raw[2..].trim_start())
    } else {
        None
    }
}

/// Evaluates a launcher query across prefix modes, inline tools, and fuzzy search.
fn load_icon_data_url(icons_dir: &std::path::Path, key: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    const MAX_ICON_CACHE_ENTRIES: usize = 512;
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(key) {
            return Some(cached.clone());
        }
    }

    let p = icons_dir.join(format!("{key}.png"));
    let bytes = std::fs::read(&p).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:image/png;base64,{b64}");

    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= MAX_ICON_CACHE_ENTRIES {
            guard.clear();
        }
        guard.insert(key.to_string(), data_url.clone());
    }
    Some(data_url)
}

fn first_line_truncated(text: &str, max_chars: usize) -> String {
    let first_line = text.lines().next().unwrap_or("").trim();
    let mut chars = first_line.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[tauri::command]
pub fn query(raw: String, state: State<'_, AppState>) -> Result<QueryResponse, String> {
    if let Ok(mut last) = state.last_query.lock() {
        *last = raw.clone();
    }

    let cfg = state.config.load();
    let trimmed_raw = raw.trim_start();
    let find_rest = extract_find_rest(trimmed_raw);

    if let Some(fq) = find_rest {
        let roots = find_files::default_find_roots(&cfg.curated_folders);
        let results = find_files::search_recent_files(fq, &roots, cfg.max_results);
        return Ok(QueryResponse {
            mode: PrefixMode::Path,
            results,
            inline_preview: None,
        });
    }

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
        PrefixMode::Convert => Ok(QueryResponse {
            mode,
            results: Vec::new(),
            inline_preview: None,
        }),
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
        PrefixMode::Kill => {
            let needle = rest.to_lowercase();
            let processes = win32::process::list_processes()?;
            let results = processes
                .into_iter()
                .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
                .take(cfg.max_results)
                .map(|p| SearchResult {
                    id: format!("kill:{}", p.pid),
                    title: format!("{} (PID {})", p.name, p.pid),
                    subtitle: if p.memory_kb > 0 {
                        format!("{} MB", p.memory_kb / 1024)
                    } else {
                        "Kill process".to_string()
                    },
                    kind: ItemKind::ShellCommand,
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
        PrefixMode::Clipboard => {
            let entries = state.clipboard_store.search(rest, cfg.max_results);
            let results = entries
                .into_iter()
                .enumerate()
                .map(|(idx, entry)| SearchResult {
                    id: format!("cb:{}", entry.id),
                    title: first_line_truncated(&entry.text, 80),
                    subtitle: "Clipboard History".to_string(),
                    kind: ItemKind::InlineResult,
                    icon_path: None,
                    secondary_hint: None,
                    score: 100.0 - idx as f64,
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
            let inline_preview = if rest.is_empty() {
                None
            } else {
                inline_tools::evaluate(rest, false, &cfg.currency_rates)
            };
            let mut results = {
                let frecency = state
                    .frecency
                    .lock()
                    .map_err(|_| "frecency lock poisoned".to_string())?;
                state.search_engine.search(
                    rest,
                    &frecency,
                    cfg.max_results,
                    cfg.prefix_memory_threshold,
                    now_unix(),
                )
            };
            let icons_dir = state.app_dir.join("icons");
            for r in &mut results {
                if let Some(key) = &r.icon_path {
                    r.icon_path = load_icon_data_url(&icons_dir, key);
                }
            }
            if let Some(web_result) = match_web_shortcut(rest, &cfg.web_shortcuts) {
                results.insert(0, web_result);
                results.truncate(cfg.max_results);
            }
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
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExecuteOutcome, String> {
    let secondary = action == ActionKind::Secondary;

    let outcome = if let Some(url) = item_id.strip_prefix("web:") {
        win32::executor::execute_target(url, LaunchMode::Normal)?;
        ExecuteOutcome::Launched
    } else if let Some(cmd) = item_id.strip_prefix("shell:") {
        let mode = match action {
            ActionKind::RunAsAdmin => LaunchMode::Elevated,
            ActionKind::Secondary => LaunchMode::ShellTerminal,
            _ => LaunchMode::ShellHidden,
        };
        win32::executor::execute_target(cmd, mode)?;
        ExecuteOutcome::Launched
    } else if let Some(cb_id) = item_id.strip_prefix("cb:") {
        let entry = state
            .clipboard_store
            .get_by_id(cb_id)
            .ok_or_else(|| format!("clipboard entry not found: {cb_id}"))?;
        win32::executor::copy_text_to_clipboard(&entry.text)?;
        ExecuteOutcome::CopiedToClipboard { value: entry.text }
    } else if let Some(pid_str) = item_id.strip_prefix("kill:") {
        let pid = pid_str
            .parse::<u32>()
            .map_err(|_| format!("invalid process id: {pid_str}"))?;
        win32::process::kill_process(pid)?;
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
    } else if let Some(file_path) = item_id.strip_prefix("file:") {
        match action {
            ActionKind::Reveal | ActionKind::Secondary => {
                win32::executor::execute_target(file_path, LaunchMode::RevealInExplorer)?;
                ExecuteOutcome::Launched
            }
            ActionKind::CopyPath => {
                win32::executor::copy_text_to_clipboard(file_path)?;
                ExecuteOutcome::CopiedToClipboard {
                    value: file_path.to_string(),
                }
            }
            ActionKind::RunAsAdmin => {
                win32::executor::execute_target(file_path, LaunchMode::Elevated)?;
                ExecuteOutcome::Launched
            }
            ActionKind::Primary => {
                if win32::executor::execute_target(file_path, LaunchMode::Normal).is_err() {
                    win32::executor::execute_target(file_path, LaunchMode::RevealInExplorer)?;
                }
                ExecuteOutcome::Launched
            }
        }
    } else if item_id == "__inline__" {
        let raw = state
            .last_query
            .lock()
            .map(|q| q.clone())
            .unwrap_or_default();
        let (mode, rest) = parse_prefix_mode(&raw);
        let cfg = state.config.load();
        if let Some(preview) =
            inline_tools::evaluate(rest, mode == PrefixMode::Calc, &cfg.currency_rates)
        {
            win32::executor::copy_text_to_clipboard(&preview.copy_value)?;
            ExecuteOutcome::CopiedToClipboard {
                value: preview.copy_value,
            }
        } else {
            return Err("No inline preview available to copy".to_string());
        }
    } else {
        let snapshot = state.search_engine.snapshot();
        if let Some(item) = snapshot.items.iter().find(|i| i.id == item_id) {
            if let Some(power_cmd) = item.target.strip_prefix("power:") {
                win32::executor::system_power_action(power_cmd)?;
                ExecuteOutcome::Launched
            } else if action == ActionKind::CopyPath {
                win32::executor::copy_text_to_clipboard(&item.target)?;
                ExecuteOutcome::CopiedToClipboard {
                    value: item.target.clone(),
                }
            } else if action == ActionKind::Reveal {
                win32::executor::execute_target(&item.target, LaunchMode::RevealInExplorer)?;
                ExecuteOutcome::Launched
            } else if action == ActionKind::RunAsAdmin {
                win32::executor::execute_target(&item.target, LaunchMode::Elevated)?;
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
                    ItemKind::Workflow => {
                        let cfg = state.config.load();
                        let keyword = item.id.strip_prefix("workflow:").unwrap_or("");
                        if let Some(wf) = cfg
                            .custom_workflows
                            .iter()
                            .find(|w| w.keyword.eq_ignore_ascii_case(keyword))
                        {
                            let full_cmd = build_workflow_command(&wf.command, &wf.args);
                            let mode = if action == ActionKind::RunAsAdmin {
                                LaunchMode::Elevated
                            } else if wf.run_in_terminal || secondary {
                                LaunchMode::ShellTerminal
                            } else {
                                LaunchMode::ShellHidden
                            };
                            win32::executor::execute_target(&full_cmd, mode)?;
                        } else {
                            let mode = if secondary {
                                LaunchMode::ShellTerminal
                            } else {
                                LaunchMode::ShellHidden
                            };
                            win32::executor::execute_target(&item.target, mode)?;
                        }
                        ExecuteOutcome::Launched
                    }
                    ItemKind::WebShortcut => {
                        let keyword = item.id.strip_prefix("web:").unwrap_or("");
                        let last_q = state
                            .last_query
                            .lock()
                            .map(|q| q.clone())
                            .unwrap_or_default();
                        let url = build_web_shortcut_url(&item.target, keyword, &last_q);
                        win32::executor::execute_target(&url, LaunchMode::Normal)?;
                        ExecuteOutcome::Launched
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
            ExecuteOutcome::CopiedToClipboard { value: item_id.clone() }
        } else {
            return Err(format!("Unknown or unindexed item id: {item_id}"));
        }
    };

    if !item_id.is_empty()
        && item_id != "__inline__"
        && !item_id.starts_with("win:")
        && !item_id.starts_with("shell:")
        && !item_id.starts_with("copy:")
    {
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

    Ok(outcome)
}

fn build_workflow_command(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        return command.to_string();
    }
    let mut parts = Vec::with_capacity(1 + args.len());
    parts.push(command.to_string());
    for arg in args {
        if arg.contains(' ') || arg.contains('\t') {
            parts.push(format!("\"{}\"", arg.replace('"', "\\\"")));
        } else {
            parts.push(arg.clone());
        }
    }
    parts.join(" ")
}

fn build_web_shortcut_url(template: &str, keyword: &str, raw_query: &str) -> String {
    let trimmed = raw_query.trim();
    let arg = if !keyword.is_empty()
        && trimmed
            .get(..keyword.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(keyword))
    {
        trimmed[keyword.len()..].trim_start()
    } else {
        trimmed
    };
    let encoded = percent_encode_query(arg);
    template.replace("{query}", &encoded)
}

fn percent_encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
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

/// Resizes and repositions the launcher window to frame only the launched row in-place.
#[tauri::command]
pub fn collapse_to_row(y_offset: i32, height: u32, app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let scale = window.scale_factor().unwrap_or(1.0);
        if let Ok(pos) = window.outer_position() {
            let new_y = pos.y + (f64::from(y_offset) * scale).round() as i32;
            let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, new_y));
        }
        window
            .set_size(LogicalSize::new(LAUNCHER_WIDTH as f64, height.max(36) as f64))
            .map_err(|e| format!("failed to collapse launcher: {e}"))?;
    }
    Ok(())
}

/// Resizes the launcher window height (clamped to `[56, 520]`) and optional width.
#[tauri::command]
pub fn resize_launcher(height: u32, width: Option<u32>, app: AppHandle) -> Result<(), String> {
    let clamped = clamp_launcher_height(height);
    let target_width = width
        .unwrap_or(LAUNCHER_WIDTH)
        .clamp(LAUNCHER_WIDTH, LAUNCHER_WIDTH + 300);
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_size(LogicalSize::new(target_width as f64, clamped as f64))
            .map_err(|e| format!("failed to resize launcher: {e}"))?;
    }
    Ok(())
}

/// Returns supported target formats for a given source file path based on its extension.
#[tauri::command]
pub fn get_convert_targets(source_path: String) -> Result<Vec<String>, String> {
    let ext = std::path::Path::new(&source_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    Ok(crate::converter::supported_targets(ext)
        .into_iter()
        .map(String::from)
        .collect())
}

/// Converts a file to the specified target format on a blocking worker thread.
#[tauri::command]
pub async fn convert_file_cmd(
    source_path: String,
    target_format: String,
) -> Result<crate::converter::ConvertResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::converter::convert_file(std::path::Path::new(&source_path), &target_format)
    })
    .await
    .map_err(|e| format!("conversion task failed: {e}"))?
}

/// Reveals the given file or directory in Windows Explorer.
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    crate::win32::executor::execute_target(&path, crate::win32::executor::LaunchMode::RevealInExplorer)
}

/// Opens a native Windows file picker and returns the selected full file path.
#[tauri::command]
pub async fn pick_convert_file(window: tauri::Window) -> Result<Option<String>, String> {
    let _ = window.set_always_on_top(false);
    let res = tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        let output = {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            std::process::Command::new("powershell")
                .creation_flags(CREATE_NO_WINDOW)
                .args([
                    "-NoProfile",
                    "-Sta",
                    "-Command",
                    "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; if ($f.ShowDialog() -eq 'OK') { Write-Output $f.FileName }",
                ])
                .output()
                .map_err(|e| format!("Failed to open file dialog: {e}"))?
        };
        #[cfg(not(windows))]
        let output = std::process::Command::new("zenity")
            .arg("--file-selection")
            .output()
            .map_err(|e| format!("Failed to open file dialog: {e}"))?;

        if !output.status.success() {
            return Ok(None);
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(path))
        }
    })
    .await
    .map_err(|e| format!("file picker task failed: {e}"));
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    let _ = window.set_focus();
    res?
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
        assert_eq!(parse_prefix_mode("convert"), (PrefixMode::Convert, ""));
        assert_eq!(parse_prefix_mode("CONVERT C:\\a.png"), (PrefixMode::Convert, "C:\\a.png"));
        assert_eq!(parse_prefix_mode("convert äöü.png"), (PrefixMode::Convert, "äöü.png"));
        assert_eq!(parse_prefix_mode("kill"), (PrefixMode::Kill, ""));
        assert_eq!(parse_prefix_mode("kill chrome"), (PrefixMode::Kill, "chrome"));
        assert_eq!(parse_prefix_mode("KILL  notepad"), (PrefixMode::Kill, "notepad"));
        assert_eq!(parse_prefix_mode("überprüfen"), (PrefixMode::Global, "überprüfen"));
        assert_eq!(parse_prefix_mode("🚀launch"), (PrefixMode::Global, "🚀launch"));
        assert_eq!(parse_prefix_mode("spotify"), (PrefixMode::Global, "spotify"));
    }

    #[test]
    fn extract_find_rest_handles_non_ascii_safely() {
        assert_eq!(extract_find_rest("aä"), None);
        assert_eq!(extract_find_rest("🚀"), None);
        assert_eq!(extract_find_rest("äää"), None);
        assert_eq!(extract_find_rest("find: report"), Some("report"));
        assert_eq!(extract_find_rest("F:  notes"), Some("notes"));
    }

    #[test]
    fn workflow_command_and_web_shortcut_formatting() {
        assert_eq!(
            build_workflow_command("npm", &["run".into(), "dev".into(), "--port 3000".into()]),
            "npm run dev \"--port 3000\""
        );
        assert_eq!(
            build_web_shortcut_url("https://google.com/search?q={query}", "g", "G  rust & tauri"),
            "https://google.com/search?q=rust%20%26%20tauri"
        );
    }

    #[test]
    fn bang_matching_and_url_encoding() {
        let cfg = crate::config::LaunchifyConfig::default();
        assert_eq!(cfg.web_shortcuts.len(), 5);

        let res = match_web_shortcut("g rust async", &cfg.web_shortcuts).expect("should match g");
        assert_eq!(res.kind, ItemKind::WebShortcut);
        assert_eq!(res.title, "Search Google for 'rust async'");
        assert_eq!(res.subtitle, "https://www.google.com/search?q=rust%20async");
        assert_eq!(res.id, "web:https://www.google.com/search?q=rust%20async");

        let res_ci = match_web_shortcut("GH  tauri/v2 & more", &cfg.web_shortcuts).expect("case-insensitive");
        assert_eq!(res_ci.title, "Search GitHub for 'tauri/v2 & more'");
        assert_eq!(res_ci.subtitle, "https://github.com/search?q=tauri%2Fv2%20%26%20more");

        assert!(match_web_shortcut("g", &cfg.web_shortcuts).is_none());
        assert!(match_web_shortcut("g   ", &cfg.web_shortcuts).is_none());
        assert!(match_web_shortcut("unknown query", &cfg.web_shortcuts).is_none());
    }
}
