# Verification Report: Critical & Major Review Findings (Batch 1)

Verified against current codebase state as Devil's Advocate.

---

## 1. Finding-by-Finding Verification

### docs/reviews/work/part-F1.md

- **F1-01** (`src-tauri/src/win32/executor.rs:134-150`): **CONFIRMED**
  - *Analysis*: `execute_target` takes an arbitrary `target: &str` and executes `powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {encoded}` or `wt.exe powershell.exe -NoExit -EncodedCommand {encoded}`. If `> shell` or IPC payload passes arbitrary PowerShell code, it runs directly. Additionally, in `wt.exe`, passing `powershell.exe -NoExit -EncodedCommand {encoded}` without `--` or quoting allows command injection / argument confusion into Windows Terminal CLI parser.
  - *Note*: Ensure the fix wraps arguments passed to `wt.exe` with `-- powershell.exe -NoExit -EncodedCommand "{encoded}"` or sanitizes inputs.

- **F1-02** (`src-tauri/src/win32/switcher.rs:63-71`): **CONFIRMED**
  - *Analysis*: In `enum_proc`, `let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, ...)` discards the `HRESULT`. If DWM fails or denies access (elevated/protected windows or UAC transitions), `cloaked` remains 0, erroneously treating cloaked/suspended background UWP application frames as visible windows.
  - *Corrected Fix*: `let hr = DwmGetWindowAttribute(...); if hr.is_err() || cloaked != 0 { return true.into(); }`.

- **F1-03** (`src-tauri/src/win32/hook.rs:216-225`, `238-248`): **DOWNGRADED(MINOR)**
  - *Analysis*: The concern claims `PostThreadMessageW` can fail if called before message queue creation or deadlocks `join()`. However, `start_hook_thread` uses a synchronous channel rendezvous (`ready_rx.recv()`) that only delivers `tid` *after* the thread enters its loop right before `GetMessageW`. While `PostThreadMessageW` ignoring errors is not ideal, `ready_rx.recv()` ensures the message queue exists. True deadlock risk is low, and app-level crash/hang scenario is an edge case.
  - *Action*: Downgraded to MINOR robustness hygiene.

- **F1-04** (`src-tauri/src/win32/switcher.rs:144-158`): **CONFIRMED**
  - *Analysis*: `focus_window` only calls `SetForegroundWindow(hwnd)`. On Windows 10/11, if Launchify does not currently have foreground lock or is transitioning, `SetForegroundWindow` fails without attaching input queues (`AttachThreadInput`) or passing foreground permissions. The target window only flashes on the taskbar and `focus_window` returns `Err("SetForegroundWindow failed")`.
  - *Fix Confirmed*: Use `AttachThreadInput` / `BringWindowToTop` before `SetForegroundWindow`.

---

### docs/reviews/work/part-F2.md

- **F2-01** (`src-tauri/src/commands.rs:244-247`): **CONFIRMED**
  - *Analysis*: When `item_id` does not match prefixes `shell:`, `win:`, or `copy:`, and is not found in `snapshot.items`, `commands.rs:245` unconditionally executes `win32::executor::execute_target(&item_id, LaunchMode::Normal)`. Any arbitrary binary/target string sent via IPC is executed without verification against known items or valid catalog entries.
  - *Fix Confirmed*: Reject unknown/unindexed `item_id` with an explicit `Err`.

- **F2-02** (`src-tauri/src/commands.rs:255-259` vs `src/ipc.ts:20-25`): **CONFIRMED**
  - *Analysis*: In Rust, parameters are `raw: String, top_item_title: Option<String>`. In TypeScript, `invoke("autocomplete", { input, selectedId })` sends fields `input` and `selectedId`. Tauri v2 parameter deserialization fails with `missing field raw`, completely breaking Tab autocomplete. (Duplicate of X-contract-01).

- **F2-03** (`src-tauri/src/commands.rs:183-202` vs `src/main.ts:81`): **CONFIRMED**
  - *Analysis*: In `src/main.ts:81`, executing inline preview sends `executeItem("__inline__", state.query, secondary)`. The backend `commands.rs:execute` does not recognize `"__inline__"`, searches `snapshot.items`, and falls through to `execute_target("__inline__", Normal)`, failing with `ShellExecuteW code 2`. Inline calculation/unit results cannot be copied or executed. (Duplicate of X-contract-03).

- **F2-04** (`src-tauri/src/commands.rs:177-181`): **DOWNGRADED(MINOR)**
  - *Analysis*: `storage::save_frecency_atomic` is called inside `if let Ok(mut frecency) = state.frecency.lock()`. The file is very small (~few KB JSON). While performing disk I/O under a lock can block query frecency reads, this only occurs when an item is executed (launcher closes anyway). Calling `std::thread::spawn` on every selection without a dedicated background writer or debounce could cause thread churn and out-of-order writes.
  - *Action*: Downgraded to MINOR performance/hygiene concern; async flush or background worker is cleaner.

---

### docs/reviews/work/part-F3.md

- **F3-01** (`src-tauri/src/indexer/scanner.rs:104`): **CONFIRMED**
  - *Analysis*: In `scan_lnk_dir`, `IndexItem` is constructed with `icon_key: Some(format!("app_{stem}"))`. In `IndexManager::rebuild_now`, `icon_cache.get_or_extract(&key, &item.target)` calculates `key = format!("{:016x}", fnv1a_64(...))`, but only overwrites `item.icon_key = Some(key)` if extraction succeeds. If extraction fails or target is non-extractable, `item.icon_key` remains `app_{stem}`, causing `icon_path` resolution to look for nonexistent files like `app_Code.png`. Pre-populating with a dummy key violates the model invariant.
  - *Fix Confirmed*: Set `icon_key: None` in scanner.

- **F3-02** (`src-tauri/src/icons.rs:77-79`): **CONFIRMED**
  - *Analysis*: `extract_png_for_target` calls `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` unconditionally per icon extraction without checking errors or calling `CoUninitialize()`. In worker threads or repeated scanning loops, this leaks COM apartment initializations or conflicts with MTA threads (`RPC_E_CHANGED_MODE`).
  - *Fix Confirmed*: Use an RAII drop guard to call `CoUninitialize()` when `CoInitializeEx` succeeded.

---

### docs/reviews/work/part-F4.md

- **F4-01** (`src-tauri/src/storage.rs:18`): **CONFIRMED**
  - *Analysis*: `path.with_extension(format!("{}.{}.tmp", ...))` strips the original extension and replaces it with `.tmp`. More crucially, on Windows, `std::fs::rename(tmp, path)` fails with `ERROR_ALREADY_EXISTS` / `Access is denied` if destination `path` already exists, because Rust's Windows `std::fs::rename` maps to `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`. Atomic overwrite will fail on existing index/frecency files.
  - *Fix Confirmed*: Use proper temp naming and ensure atomic replace semantics via `replace_file` or Windows API.

- **F4-02** (`src-tauri/src/frecency.rs:47-49`): **REJECTED(Guarded by types and logic)**
  - *Reason*: `entry.count` is a `u32` (cannot be negative). `now_unix.saturating_sub(entry.last_used_unix)` is clamped with `.max(0) as f64`. `FRECENCY_HALF_LIFE_SECS` is a positive constant `86400.0`. `0.5_f64.powf(...)` on non-negative floats always produces a value in `(0.0, 1.0]`. `(count as f64).ln_1p()` for `u32 >= 1` is strictly finite and non-negative. There is no mathematical possibility of `NaN` or `Inf` under any standard timestamp drift or valid u32 count.

- **F4-03** (`src-tauri/src/path_nav.rs:77`, `150-152`): **CONFIRMED**
  - *Analysis*: `format_autocomplete` formats Windows paths like `C:\Tools\` as `/C:\Tools\`. When this text is completed into the input and user types further, `browse_path` passes `"/C:\Tools\"` into `parse_raw_path`. If executed directly, Windows executables and shells do not recognize `/C:\...` as a valid absolute path.
  - *Fix Confirmed*: Sanitize leading slashes for Windows drive letters.

---

### docs/reviews/work/part-F5.md

- **F5-01** (`src/renderer.ts:85-91`, `src/styles.css:140-159`): **DOWNGRADED(MINOR)**
  - *Analysis*: Results list has a hard limit of `MAX_VISIBLE_RESULTS = 8`, and window height is resized to fit exactly the items (`computeTargetHeight(state.results.length)`). Under normal conditions the list container does not scroll vertically. However, if window height is clamped or constrained, `#selection-indicator` uses `translateY` without `scrollTop`. It is a valid UI edge-case, but not a MAJOR breaking bug.
  - *Action*: Downgraded to MINOR.

- **F5-02** (`src/main.ts:67-75`, `src/main.ts:153-176`): **CONFIRMED**
  - *Analysis*: `executeSelected` starts an untracked 650ms timeout. If the user quickly re-opens the launcher within 650ms via global hotkey (`onLauncherShown`), the old timer fires, checks `if (!state.visible)` (or if state changed), and calls `renderVisibility(refs, false)` and `hideLauncher()`, abruptly closing the newly opened launcher session.
  - *Fix Confirmed*: Track and clear timer handles (`clearTimeout`) across show/hide/execute state transitions.

- **F5-03** (`src/keyboard.ts:61-63`): **CONFIRMED**
  - *Analysis*: `window.addEventListener("blur", onWindowBlur)` calls `handlers.onHide()` immediately on any blur event. During initial window focus transfer, IME composition window opening, or toolwindow style application, transient `blur` events are dispatched by the browser engine, prematurely dismissing the launcher while the user is interacting with it.
  - *Fix Confirmed*: Guard blur with `document.hasFocus()` in `requestAnimationFrame` or short debounce.

---

### docs/reviews/work/cross-contract.md

- **X-contract-01** (`src/ipc.ts:20-25` vs `src-tauri/src/commands.rs:254-259`): **CONFIRMED**
  - *Analysis*: Duplicate of F2-02. Verified that parameter names `{ input, selectedId }` vs `raw, top_item_title` break Tab autocomplete in Tauri v2.

- **X-contract-02** (`src-tauri/src/commands.rs:140-151` vs `src/ipc.ts:43-55`): **DOWNGRADED(MINOR)**
  - *Analysis*: This is a performance optimization/architecture choice rather than a broken contract. Returning base64 data URLs in `icon_path` works and displays correctly in `<img>`, even if asset protocol or filesystem streaming would be more optimal.
  - *Action*: Downgraded to MINOR performance/optimization.

- **X-contract-03** (`src/main.ts:79-87` vs `src-tauri/src/commands.rs:163-248`): **CONFIRMED**
  - *Analysis*: Duplicate of F2-03. Verified that passing `"__inline__"` falls through to `execute_target("__inline__", Normal)`, which fails. Frontend should invoke `executeItem("copy:" + value)` or backend must support `"__inline__"`.

- **X-contract-04** (`src-tauri/src/catalog.rs:82-83` vs `src-tauri/src/win32/executor.rs:182-195`): **CONFIRMED**
  - *Analysis*: `catalog.rs` registers `power:signout` and `power:hibernate`. `win32::executor::system_power_action` only matches `lock`, `sleep`, `restart`, `shutdown`, returning `Err("Unknown power action")` for `signout` and `hibernate`.
  - *Fix Confirmed*: Add match arms for `signout` and `hibernate` in `system_power_action`.

---

## 2. Summary Counts

- **Total Assessed**: 17 findings (CRITICAL and MAJOR)
- **CONFIRMED**: 13
- **DOWNGRADED**: 3 (F1-03 -> MINOR, F2-04 -> MINOR, F5-01 -> MINOR, X-contract-02 -> MINOR)
- **REJECTED**: 1 (F4-02: mathematically impossible to produce NaN/Inf from u32 count and saturating timestamp delta)
- **Deduplication Notes**: F2-02 / X-contract-01 are duplicates; F2-03 / X-contract-03 are duplicates.
