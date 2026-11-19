# CONTEXT (keep <=300 lines, signatures not implementations)

## Stack & Versions
- **Core & Desktop Shell:** Rust (2021 edition, `rustc 1.99.0`) + Tauri v2 (`tauri = "2"`, `tauri-build = "2"`)
- **Win32 / OS Layer:** `windows = "0.58"` (`#[cfg(windows)]` trait boundary in `win32/` and `indexer/`)
- **Window Material:** `window-vibrancy = "0.5"` (Mica/Acrylic)
- **Search & Concurrency:** `nucleo-matcher = "0.3"`, `arc-swap = "1"`, `crossbeam-channel = "0.5"`
- **Persistence:** `bincode = "1"` (pinned v1 for `index.bin`), `serde = "1"`, `serde_json = "1"`
- **Logging:** `tracing = "0.1"`, `tracing-subscriber = "0.3"`
- **Frontend:** Vanilla TypeScript (`typescript ^5.7.2`, strict) + Vite (`^6.0.3`) + `@tauri-apps/api ^2.2.0`

## Commands
- install: `npm.cmd install` (use `npm.cmd` in PowerShell if `npm.ps1` is blocked)
- dev: `npm.cmd run dev` | tauri dev: `npm.cmd run tauri dev`
- build: `npm.cmd run build`
- typecheck: `npm.cmd run typecheck`
- check: `cargo check --manifest-path src-tauri/Cargo.toml`
- lint: `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- test: `cargo test --manifest-path src-tauri/Cargo.toml`

## Folder Map
- `src/` — Vanilla TS frontend (`main.ts`, `types.ts`, `ipc.ts`, `state.ts`, `renderer.ts`, `keyboard.ts`, `styles.css`)
- `src-tauri/src/` — Rust backend (`lib.rs`, `model.rs`, `config.rs`, `storage.rs`, `logging.rs`, `inline_tools/`, `path_nav.rs`, `search/`, `frecency.rs`, `catalog.rs`, `indexer/`, `icons.rs`, `win32/`, `app.rs`, `commands.rs`, `tray.rs`)
- `docs/agents/` — `SPEC.md`, `TODO.md`, `DECISIONS.md`, `CONTEXT.md`

## Conventions (naming, errors, logging, validation)
- **Errors:** Core Rust modules return `Result<T, String>` or typed errors converted at IPC boundary; Tauri `#[tauri::command]` handlers return `Result<T, String>`.
- **Logging:** `tracing::{info, warn, error, debug}` initialized in `logging.rs`; never log user keystrokes or sensitive paths at `info` in production.
- **Validation:** Config clamped/validated on load (`max_results` `1..=8`, `prefix_memory_threshold >= 1`); atomic file writes (`.tmp` + rename).
- **Testability:** Win32 APIs sit behind traits/functions gated with `#[cfg(windows)]` so core logic is unit-tested deterministically.
- **Security:** `assetProtocol` scoped strictly to `$APPLOCALDATA/Launchify/icons/*`. Zero network calls.

## Env Vars
- `%LOCALAPPDATA%\Launchify\` — runtime data (`config.json`, `index.bin`, `frecency.json`, `icons\`, `logs\`)

## Shared Types / DB Schema
- Defined in `src-tauri/src/model.rs` (WP-01) and mirrored in `src/types.ts` (WP-06/WP-07).

## Module: `model`, `config`, `storage`, `logging` (WP-01)
- `model.rs`: `INDEX_SNAPSHOT_VERSION: u32 = 1`, `ItemKind`, `IndexItem`, `IndexSnapshot`, `FrecencyEntry`, `FrecencyStore`, `ActionKind`, `PrefixMode`, `InlinePreview`, `SearchResult`, `QueryResponse`, `ExecuteOutcome` (`Launched | CopiedToClipboard { value: String } | WindowFocused`, tagged `type`/`value`).
- `config.rs`: `LaunchifyConfig` (`curated_folders`, `custom_workflows`, `web_shortcuts`, `currency_rates`, `extra_exclusions`, `prefix_memory_threshold`, `max_results`), `validate(cfg) -> LaunchifyConfig`, `load_or_create(app_dir) -> Result<LaunchifyConfig, String>`, `save_config_atomic(app_dir, cfg) -> Result<(), String>`, `ExclusionFilter::new(extra)` (`is_excluded_dir`, `is_excluded_file`, `is_excluded`).
- `storage.rs`: `write_atomic(path, bytes) -> Result<(), String>`, `load_index(path) -> Result<IndexSnapshot, String>` (empty fallback on missing/corrupt/version-mismatch), `save_index_atomic(path, snap) -> Result<(), String>`, `load_frecency(path) -> FrecencyStore`, `save_frecency_atomic(path, store) -> Result<(), String>`.
- `logging.rs`: `init_logging(log_dir: Option<&Path>) -> Result<(), String>` (idempotent `tracing_subscriber::fmt` with `EnvFilter`, defaults to `info`).

## Module: `win32` (WP-05)
- `win32/hook.rs`: `VK_LWIN_CODE`, `HookDecision` (`PassThrough | TriggerSoloWin`), `WinKeyStateMachine::on_key_event(vk, is_key_down, is_injected)`, `WinKeyHookHandle::start(trigger_tx)` (`WH_KEYBOARD_LL` on dedicated thread, injects `0xFF` mask key on solo `VK_LWIN` release, `UnhookWindowsHookEx` on `Drop`).
- `win32/window.rs`: `MonitorRect`, `compute_launcher_position(work_area, logical_width, dpi_scale) -> (i32, i32)` (centered horizontally, 28% vertical anchor), `apply_toolwindow_style(hwnd)`, `save_foreground_window()`, `restore_foreground_window(hwnd)`, `show_and_focus_launcher(hwnd, logical_width)`, `hide_launcher_window(hwnd)` (`SW_HIDE`).
- `win32/switcher.rs`: `WindowInfo { hwnd, title, process_name, exe_path }`, `enumerate_visible_windows() -> Vec<WindowInfo>`, `focus_window(hwnd)`.
- `win32/executor.rs`: `LaunchMode` (`Normal | Elevated | RevealInExplorer | ShellHidden | ShellTerminal`), `execute_target(target, mode)`, `copy_text_to_clipboard(text)`, `system_power_action(action)`.

## Module: `catalog`, `indexer`, `icons` (WP-04)
- `catalog.rs`: `builtin_catalog() -> Vec<IndexItem>` (`ms-settings:*` deep-links + `power:*` actions), `synonym_keywords(title) -> Vec<String>`, `config_items(cfg) -> Vec<IndexItem>` (`Workflow` + `WebShortcut`).
- `icons.rs`: `IconCache::new(cache_dir)` (`icon_path(key)`, `has_cached(key)`, `write_cached_png(key, bytes)`, `get_or_extract(key, target)` using `SHGetFileInfoW` + RGBA->PNG on Windows).
- `indexer/mod.rs`: `IndexManager::new(snapshot, config, app_dir)` (`snapshot()`, `rebuild_now() -> Result<IndexSnapshot, String>` updating `ArcSwap<IndexSnapshot>` + `index.bin`).
- `indexer/scanner.rs`: `scan_curated_folders(folders, filter)`, `scan_start_menu(filter)`, `scan_apps_folder()`.
- `indexer/watcher.rs`: `DEBOUNCE_MS = 1500`, `Debouncer` (`record_event`, `should_flush`, `mark_flushed`), `WatcherHandle`, `spawn_watcher(event_rx, on_flush)` (EcoQoS thread on Windows).

## Module: `inline_tools`, `path_nav` (WP-02)
- `inline_tools/mod.rs`: `evaluate(query: &str, force_calc: bool, rates: &HashMap<String, f64>) -> Option<InlinePreview>` (dispatches `utils` -> `units` -> `math`; `=` prefix forces evaluation, otherwise auto-detects unambiguous expressions).
- `inline_tools/math.rs`: `eval_math(input: &str) -> Result<f64, String>`, `format_number(val: f64) -> String`, `looks_like_math_expr(input: &str) -> bool`.
- `inline_tools/units.rs`: `try_convert(input: &str, rates: &HashMap<String, f64>) -> Option<(String, String, String)>` (offline length/mass/temp/data/time + static USD-base currency rates).
- `inline_tools/utils.rs`: `try_eval_util(input: &str) -> Option<(String, String, String)>` (`#RRGGBB`/`#RGB` <-> `rgb(r,g,b)`, `uuid`/`uuid4`, `now`/`unix`/10-or-13-digit Unix/ISO-8601 UTC).
- `path_nav.rs`: `browse_path(raw: &str, exclusions: &ExclusionFilter, limit: usize) -> Vec<SearchResult>`, `complete_path(raw: &str, exclusions: &ExclusionFilter) -> Option<String>`.

## Module: `search` & `frecency` (WP-03)
- `frecency.rs`: `normalize_prefix(query)`, `record_selection(store, query_prefix, item_id, now_unix)`, `frecency_score(entry, now_unix) -> f64` (7-day half-life decay), `prefix_memory_boost(store, query_prefix, item_id, threshold) -> f64` (strong boost at `count >= threshold`).
- `search/ranking.rs`: `title_match_bonus(title, query) -> f64` (exact > prefix > word-boundary > substring), `secondary_hint_for_kind(kind) -> Option<String>`.
- `search/engine.rs`: `SearchEngine` wrapping `Arc<ArcSwap<IndexSnapshot>>` with `new`, `from_arc`, `update_index`, `snapshot`, `search(query, frecency, max_results, prefix_threshold, now_unix) -> Vec<SearchResult>` using `nucleo-matcher`.

## Module: `frontend` (WP-07)
- `src/types.ts`: TS mirrors of `model.rs` (`ItemKind`, `PrefixMode`, `InlinePreview`, `SearchResult`, `QueryResponse`, `ExecuteOutcome`, `LauncherState`).
- `src/ipc.ts`: Typed Tauri `invoke` wrappers (`queryBackend`, `executeItem`, `autocompleteQuery`, `hideLauncher`, `resizeLauncher`), event listeners (`onLauncherShown`, `onLauncherHidden`), and `resolveIconUrl` (`convertFileSrc`).
- `src/state.ts`: `createInitialState`, `clampResults` (max 8), `moveSelection` (wrap-around), `computeTargetHeight` (56px idle, 48px/row + 9px padding), `resetQueryState`.
- `src/renderer.ts`: `getDomRefs`, `renderVisibility`, `renderState`, `updateSelectionOnly` (with `2px` left accent bar on `.is-selected`, async `loading="lazy"` icons + fallback glyph, and `resizeLauncher` on height change).
- `src/keyboard.ts`: `attachKeyboardFlow` (`ArrowUp`/`ArrowDown`/`Ctrl+J`/`Ctrl+K` wrap-around, `Enter`/`Shift+Enter`/`Ctrl+Enter`, `Tab` autocomplete, 2-stage `Escape`, `blur` hide).
- `src/styles.css`: 680px × 56px idle pill, `border-radius: 14px`, `1px` inner border, `80ms` scale-fade (`0.98 -> 1.0`), zero idle keyframes/timers.
