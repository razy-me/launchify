# REVIEW backend — reviewer: fresh agent
Scope: `src-tauri/src/**` (`app.rs`, `commands.rs`, `icons.rs`, `tray.rs`, `storage.rs`, `indexer/mod.rs`, `indexer/watcher.rs`, `search/engine.rs`, `win32/hook.rs`, `win32/window.rs`, `win32/executor.rs`, `win32/switcher.rs`)

| # | Sev | Category | File:Line | Finding | Recommendation |
|---|-----|----------|-----------|---------|----------------|
| 1 | CRITICAL | bug | `src-tauri/src/icons.rs:145` | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` fails on `bgra.chunks_exact_mut(4)` (`clippy::chunks-exact-to-as-chunks`), breaking R25 lint hygiene. | Replace `bgra.chunks_exact_mut(4)` with `bgra.as_chunks_mut::<4>().0`. |
| 2 | CRITICAL | bug | `src-tauri/Cargo.toml` / `launchify_lib` | `cargo test --manifest-path src-tauri/Cargo.toml` fails at runtime with `0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND)` when executing `launchify_lib` unit tests. | Investigate DLL/entrypoint mismatch in test binary (e.g. `WebView2Loader.dll` or `windows` crate feature/linking) and restore passing `cargo test` (R25). |
| 3 | MAJOR | spec | `src-tauri/src/app.rs:35,145` | `watcher_handle` is initialized to `None` and `indexer::watcher::spawn_watcher` is never wired in `setup_app`; `ReadDirectoryChangesW` and `SHChangeNotifyRegister` are not active at runtime (R22). | Spawn `ReadDirectoryChangesW` (Start Menu + `curated_folders`) and `SHChangeNotifyRegister` (`shell:AppsFolder`) feeders into `spawn_watcher` in `setup_app` and store the `WatcherHandle`. |
| 4 | MAJOR | bug | `src-tauri/src/win32/window.rs:180-191` | `GetDpiForWindow(hwnd)` is called before `SetWindowPos` moves the window to the target monitor, so moving between monitors with different DPI scales uses the old monitor's DPI for `compute_launcher_position`. | Query the target `HMONITOR` DPI via `GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, ...)` before computing `(x, y)`. |
| 5 | MAJOR | bug | `src-tauri/src/win32/executor.rs:108-122` | `ShellHidden` and `ShellTerminal` interpolate `{target}` directly into `-Command "{target}"` without escaping embedded `"` characters, breaking commands that contain double quotes. | Escape double quotes (or use `-EncodedCommand` / proper PowerShell quoting) before passing `args` to `ShellExecuteW`. |
| 6 | MINOR | spec | `src-tauri/src/win32/executor.rs:82` | Uses `ShellExecuteW` instead of `ShellExecuteExW` as specified in R24. | Switch to `ShellExecuteExW` (`SHELLEXECUTEINFOW`) if process handle / extended error reporting is needed, or document equivalence. |
| 7 | MINOR | bug | `src-tauri/src/app.rs:63-81,154-157` | `show_launcher` is invoked directly from the `launchify-hook-listener` background thread, mutating window position/focus (`SetWindowPos`, `SetForegroundWindow`) off the main/UI thread. | Dispatch `show_launcher` onto the Tauri main thread via `app.run_on_main_thread(...)`. |
| 8 | MINOR | perf | `src-tauri/src/commands.rs:155` | `save_frecency_atomic` writes `frecency.json` synchronously on every `execute` call. | Acceptable for infrequent launches, but consider debouncing or offloading to a background thread if needed. |

## Challenged Decisions (D-xxx): agree / disagree + reason
- **D-001 (Tauri v2 + Vanilla TS):** Agree. Keeps backend/frontend boundary thin and aligns with R1–R4, R21.
- **D-002 (`bincode` for `index.bin` + atomic JSON for `config.json` & `frecency.json`):** Agree. `storage::write_atomic` + `bincode` v1 in `storage.rs` is clean and avoids SQLite DLL overhead.
- **D-003 (Offline static currency rates & custom recursive-descent math parser):** Agree. Strictly honors R17 (zero network requests).
- **D-004 (Dedicated `WH_KEYBOARD_LL` thread with pure state-machine core):** Agree. `win32/hook.rs` cleanly isolates `WinKeyStateMachine`, injects `VK_MASK_DUMMY` (`0xFF`) on solo `VK_LWIN` release, and unhooks via `UnhookWindowsHookEx` on `Drop`.
- **D-005 (Disjoint parallel execution of WP-06 and WP-07):** Agree. `commands.rs` and `model.rs` DTOs are cleanly separated.

## Missing / partially met requirements
- **R22 (Partially met):** `IndexManager` and `Debouncer` (`1500 ms` + EcoQoS) exist, but `setup_app` never starts `ReadDirectoryChangesW` / `SHChangeNotifyRegister` or `spawn_watcher` (`watcher_handle` stays `None`).
- **R24 (Partially met):** Uses `ShellExecuteW` rather than `ShellExecuteExW`; `ShellHidden`/`ShellTerminal` quote-escaping bug on `"` in `target`.
- **R25 (Not met):** `cargo clippy -- -D warnings` fails (`icons.rs:145`) and `cargo test` fails with `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`.

## Overall assessment
The Rust backend architecture (lock-free `ArcSwap` + `nucleo-matcher`, dedicated `WH_KEYBOARD_LL` hook thread with `Drop` cleanup, atomic storage, and `WS_EX_TOOLWINDOW` + Mica/Acrylic setup) is well-structured, but **2 CRITICAL** build/test gate failures (`clippy` error in `icons.rs:145` and `cargo test` `0xc0000139` crash) and **3 MAJOR** runtime/spec issues (unwired filesystem/shell watchers in `app.rs`, stale-monitor DPI in `window.rs`, and unescaped quotes in `executor.rs`) must be resolved before shipping.
