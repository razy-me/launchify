# Launchify Architecture Map

## Stack
- **Desktop Framework**: Tauri v2 (`src-tauri/`) + Vite + TypeScript vanilla DOM frontend (`src/`, `index.html`).
- **OS Target**: Windows (Win32 APIs via `windows` crate for global hotkeys, window management, ShellExecute, icons, window switching).
- **Persistence**: SQLite/JSON via `src-tauri/src/storage.rs` & `config.rs` (frecency, settings).

## Entry Points & Trust Boundaries
- **Frontend**: `index.html` -> `src/main.ts` (DOM events, keyboard shortcuts, IPC calls via `src/ipc.ts` to Tauri commands).
- **Backend**: `src-tauri/src/main.rs` -> `src-tauri/src/lib.rs` (Tauri builder, plugin/tray setup, command registration).
- **IPC Boundary**: `src-tauri/src/commands.rs` (`#[tauri::command]` handlers invoked by WebView).
  - Untrusted/user-controlled inputs: search queries, file paths, action kinds, conversion targets, window resize dimensions.
  - Sinks: `win32/executor.rs` (process execution / ShellExecute / admin elevation), `converter/mod.rs` (file I/O, external tools/conversions), `find_files.rs` / `path_nav.rs` (filesystem traversal), `storage.rs` (DB/file writes).
- **Global Input Hook**: `src-tauri/src/win32/hook.rs` (Win32 low-level keyboard hook / global shortcut handling).
- **Background Workers**: `src-tauri/src/indexer/` (directory scanner + file watcher updating shared search index).

## Shared Modules
- `src-tauri/src/model.rs` <-> `src/types.ts` (IPC DTOs: `SearchResult`, `ActionKind`, `Mode`, etc.)
- `src-tauri/src/app.rs` (shared `AppState`, mutexes/locks, window visibility lifecycle)
- `src-tauri/src/config.rs` & `src-tauri/src/storage.rs` (app configuration & frecency persistence)
