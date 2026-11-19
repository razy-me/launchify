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
- Defined in `src-tauri/src/model.rs` (WP-01) and mirrored in `src/types.ts` (WP-06).
