# Review Plan — Full Codebase Audit (Tier 2)

| ID | Name | Paths | LOC | Risk | Status |
|---|---|---|---|---|---|
| F01 | Frontend (UI, State, Keyboard, IPC) | `index.html`, `src/ipc.ts`, `src/keyboard.ts`, `src/main.ts`, `src/renderer.ts`, `src/state.ts`, `src/styles.css`, `src/types.ts` | ~1659 | MEDIUM | done |
| F02 | Backend Core, IPC Commands, Config & Storage | `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`, `src-tauri/src/app.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/config.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/model.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/logging.rs`, `src-tauri/src/catalog.rs`, `src-tauri/src/frecency.rs`, `src-tauri/src/icons.rs` | ~2140 | HIGH | done |
| F03 | Search Engine, Indexer & File Navigation | `src-tauri/src/find_files.rs`, `src-tauri/src/path_nav.rs`, `src-tauri/src/indexer/mod.rs`, `src-tauri/src/indexer/scanner.rs`, `src-tauri/src/indexer/watcher.rs`, `src-tauri/src/search/engine.rs`, `src-tauri/src/search/mod.rs`, `src-tauri/src/search/ranking.rs` | ~1170 | MEDIUM | done |
| F04 | Converter & Inline Tools | `src-tauri/src/converter/mod.rs`, `src-tauri/src/inline_tools/math.rs`, `src-tauri/src/inline_tools/mod.rs`, `src-tauri/src/inline_tools/units.rs`, `src-tauri/src/inline_tools/utils.rs` | ~1632 | HIGH | done |
| F05 | Win32 Layer (Executor, Hook, Switcher, Window) | `src-tauri/src/win32/executor.rs`, `src-tauri/src/win32/hook.rs`, `src-tauri/src/win32/mod.rs`, `src-tauri/src/win32/switcher.rs`, `src-tauri/src/win32/window.rs` | ~831 | HIGH | done |
