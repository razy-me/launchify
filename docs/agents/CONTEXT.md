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

## Module: `search` & `frecency` (WP-03)
- `frecency.rs`: `normalize_prefix(query)`, `record_selection(store, query_prefix, item_id, now_unix)`, `frecency_score(entry, now_unix) -> f64` (7-day half-life decay), `prefix_memory_boost(store, query_prefix, item_id, threshold) -> f64` (strong boost at `count >= threshold`).
- `search/ranking.rs`: `title_match_bonus(title, query) -> f64` (exact > prefix > word-boundary > substring), `secondary_hint_for_kind(kind) -> Option<String>`.
- `search/engine.rs`: `SearchEngine` wrapping `Arc<ArcSwap<IndexSnapshot>>` with `new`, `from_arc`, `update_index`, `snapshot`, `search(query, frecency, max_results, prefix_threshold, now_unix) -> Vec<SearchResult>` using `nucleo-matcher`.
