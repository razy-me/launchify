# Review Plan: Launchify Full Codebase Audit

| ID | Name | Paths | LOC (approx) | Risk | Status |
|---|---|---|---|---|---|
| F1 | Win32 Hook, Window, Switcher & Executor | `src-tauri/src/win32/*.rs` | ~700 | HIGH | done |
| F2 | App Lifecycle, Config, IPC Commands & Model | `src-tauri/src/app.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/config.rs`, `src-tauri/src/model.rs`, `src-tauri/src/logging.rs`, `src-tauri/src/tray.rs` | ~900 | HIGH | done |
| F3 | Search Engine, Ranking & Indexer | `src-tauri/src/search/*.rs`, `src-tauri/src/indexer/*.rs`, `src-tauri/src/icons.rs`, `src-tauri/src/catalog.rs` | ~850 | MEDIUM | done |
| F4 | Storage, Frecency & Inline Tools | `src-tauri/src/storage.rs`, `src-tauri/src/frecency.rs`, `src-tauri/src/path_nav.rs`, `src-tauri/src/inline_tools/*.rs` | ~950 | MEDIUM | done |
| F5 | Frontend UI, State, Renderer & Styles | `src/*.ts`, `src/styles.css`, `index.html` | ~650 | MEDIUM | done |
| X1 | Cross-Cutting: Contract & IPC Consistency | Across IPC boundary (`commands.rs`, `ipc.ts`, `types.ts`, `state.ts`) | — | HIGH | done |
| V1 | Verification of Severe Findings | High/Medium findings from F1-F5, X1 | — | HIGH | done |
