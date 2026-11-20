# Plan: Launchify Feature Expansion

## Goal
Extend Launchify with 5 productivity features: expanded System Power commands, Task Killer (`kill <proc>`), Web Search & Custom Bangs (`g <q>`, `yt <q>`, etc.), Live Currency Rates with persistent offline cache, and Clipboard History (`cb <q>`).

## Requirements
- **R1 (Power):** Add `empty_recycle_bin`, `hibernate`, and `signout` to `win32/executor.rs` and `catalog.rs` (`builtin_catalog()`).
- **R2 (Task Killer):** `kill <filter>` lists running processes (PID, name, RAM) and `Enter` terminates the selected PID via Win32.
- **R3 (Web & Bangs):** Default bangs (`g`, `yt`, `gh`, `ddg`, `wiki`) + user-defined `web_shortcuts` in `config.json`; `<bang> <query>` opens percent-encoded URL.
- **R4 (Live Currency):** Fetch USD-base rates from `open.er-api.com/v6/latest/USD` on startup (if stale >1h) and on-demand; persist to `%LOCALAPPDATA%\Launchify\currency_rates.json` for offline use.
- **R5 (Clipboard History):** Background watcher records text clipboard changes (max 50, deduplicated, persisted to `clipboard.json`); `cb <filter>` searches history and `Enter` copies selection back to clipboard.

## Non-Goals
- Image/file clipboard history (text-only in v1).
- Network calls for anything other than currency rate updates.
- UI settings editor for bangs (configured via `config.json`).

## Assumptions
- **A1:** `ureq` (sync, minimal deps) is used in `Cargo.toml` for currency HTTP GET so no async runtime bloat is added.
- **A2:** Clipboard watcher uses `GetClipboardSequenceNumber()` polling (500ms) on a background thread to avoid hidden-window message-loop complexity.

---

## Work Packages

### WP-01: Expanded System Power Commands
- **Action:** `EDIT` `src-tauri/src/win32/executor.rs`, `src-tauri/src/catalog.rs`
- **Contract:** Extend `system_power_action(action: &str)` with `"empty_recycle_bin"` (`SHEmptyRecycleBinW`), `"hibernate"` (`SetSuspendState(true, false, false)`), `"signout"` (`ExitWindowsEx(EWX_LOGOFF, 0)`). Add `power:recycle_bin`, `power:hibernate`, `power:signout` to `builtin_catalog()`.
- **Acceptance:** Searching `recycle`, `hibernate`, or `signout` surfaces the command and executes it.

### WP-02: Task Killer (`kill <process>`)
- **Action:** `ADD` `src-tauri/src/win32/process.rs`; `EDIT` `src-tauri/src/win32/mod.rs`, `src-tauri/src/model.rs`, `src-tauri/src/commands.rs`
- **Contract:**
  - `ProcessInfo { pub pid: u32, pub name: String, pub memory_kb: u64 }`
  - `list_processes() -> Result<Vec<ProcessInfo>, String>` (`CreateToolhelp32Snapshot` + `Process32FirstW`/`NextW`)
  - `kill_process(pid: u32) -> Result<(), String>` (`OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess`)
  - Add `PrefixMode::Kill` and `ItemKind::Process` (or `ActionKind::KillProcess`).
- **Acceptance:** Typing `kill notepad` lists matching processes; `Enter` terminates the selected PID.

### WP-03: Web Search & Custom Bangs
- **Action:** `EDIT` `src-tauri/src/config.rs`, `src-tauri/src/commands.rs`
- **Contract:** Populate default `web_shortcuts` (`g`, `yt`, `gh`, `ddg`, `wiki`) in `LaunchifyConfig::default()`. In `query(raw)`, if first token matches a `WebShortcut.keyword` followed by space + query, return a `SearchResult` with `target = url_template.replace("{query}", &percent_encode(q))`.
- **Acceptance:** Typing `g rust tauri` returns "Search Google for 'rust tauri'" and `Enter` opens the browser.

### WP-04: Live Currency Rates + Offline Cache
- **Action:** `EDIT` `src-tauri/Cargo.toml`, `src-tauri/src/storage.rs`, `src-tauri/src/app.rs`, `src-tauri/src/inline_tools/units.rs`
- **Contract:**
  - Add `ureq = { version = "2", features = ["json"] }` to `Cargo.toml`.
  - `CurrencyCache { updated_at_unix: u64, rates: HashMap<String, f64> }` saved atomically to `currency_rates.json`.
  - Background thread on startup fetches `https://open.er-api.com/v6/latest/USD` (5s timeout), updates `AppState` rates, and writes `currency_rates.json`. Offline falls back to cached/static rates.
- **Acceptance:** `100 USD in EUR` uses live rates when online and last-saved rates when offline.

### WP-05: Clipboard History (`cb <query>`)
- **Action:** `ADD` `src-tauri/src/clipboard.rs`; `EDIT` `src-tauri/src/model.rs`, `src-tauri/src/app.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- **Contract:**
  - `ClipboardEntry { pub id: String, pub text: String, pub timestamp_unix: u64 }`
  - `ClipboardStore` (max 50 items, deduplicated, persisted to `clipboard.json`).
  - Background thread polls `GetClipboardSequenceNumber()` every 500ms; on change, reads `CF_UNICODETEXT` and pushes to `ClipboardStore`.
  - `PrefixMode::Clipboard` on `cb ` / `clip ` prefix; `execute` copies entry back to clipboard.
- **Acceptance:** Copying text adds it to history; `cb <text>` filters it and `Enter` copies it back.

---

## Verification Plan
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml` (unit tests for bang parsing, currency cache fallback, clipboard deduplication, and process struct)
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- `npm.cmd run typecheck`
