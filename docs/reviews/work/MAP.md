# System Map: Launchify

## Stack & Architecture
- **Framework**: Tauri v2 (`launchify v0.1.0`), Rust 2021 edition backend, Vanilla TypeScript + Vite frontend.
- **Entry Points**:
  - Backend: `src-tauri/src/main.rs` -> `src-tauri/src/lib.rs` -> `src-tauri/src/app.rs`.
  - Native Low-Level Hook: `src-tauri/src/win32/hook.rs` (`WH_KEYBOARD_LL` intercepting Copilot key `Win + Shift + F23`).
  - Native Windows: `src-tauri/src/win32/window.rs` (DPI-aware positioning, transparent window).
  - Native Window Switcher: `src-tauri/src/win32/switcher.rs` (EnumWindows, focus switching).
  - Native Executor: `src-tauri/src/win32/executor.rs` (ShellExecuteW, powershell -EncodedCommand, clipboard).
  - Frontend: `index.html` -> `src/main.ts` -> `src/ipc.ts`, `src/keyboard.ts`, `src/renderer.ts`.
- **Trust Boundaries**:
  - Tauri IPC invokes: `query`, `execute`, `autocomplete`, `hide_launcher`, `resize_launcher`, `collapse_to_row`.
  - Execution sink: `executor.rs` receives `Action` variants (`OpenPath`, `RunCommand`, `Power`, `FocusWindow`, `Copy`).
  - Hook thread: receives all system-wide keystrokes via Win32 low-level hook.
- **Shared Modules & Data Structures**:
  - `src-tauri/src/model.rs`: `LauncherItem`, `Action`, `Category`, `QueryMatch`.
  - `src-tauri/src/search/engine.rs`: Lock-free `ArcSwap` index search with `nucleo-matcher`.
  - `src-tauri/src/storage.rs` & `frecency.rs`: JSON on-disk persistence for usage statistics & frecency scoring.
  - `src-tauri/src/icons.rs`: SHGetFileInfoW icon extraction and caching in `%LOCALAPPDATA%/Launchify/icons/`.
  - `src-tauri/src/indexer/`: Background indexing, scanning Start Menu / shell:AppsFolder, file watcher.
  - `src-tauri/src/inline_tools/`: Calculator/eval (`math.rs`), unit converter (`units.rs`), utils (`utils.rs`).
