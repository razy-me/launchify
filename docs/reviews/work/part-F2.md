# Review Findings Part F2: App Lifecycle, Config, IPC Commands & Model

Scope: `src-tauri/src/app.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/config.rs`, `src-tauri/src/model.rs`, `src-tauri/src/logging.rs`, `src-tauri/src/tray.rs`  
Project Root: `C:\Tools\Launchify`  
Auditor: F2 (App Lifecycle, Config, IPC Commands & Model)  
Dual Lens: Enabled (F1 Standard + F1b Adversarial/Security/Concurrency)

---

### [CRITICAL] [OPEN] F2-01 IPC `execute` Arbitrary Target Fallback Allows Execution of Untrusted Input
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:244-247`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: A malicious webview context, malicious script injection, or malformed IPC payload invokes `execute` with `{ item_id: "calc.exe" }` or any command/executable path not present in index items or allowed prefixes.
- **Why**: When `item_id` is not prefixed with `shell:`, `win:`, or `copy:` and is not found in `snapshot.items`, `commands.rs` blindly falls through to `win32::executor::execute_target(&item_id, LaunchMode::Normal)`. This violates the trust boundary between the frontend renderer and native execution sink: any arbitrary string passed from the webview is executed without verification against known indexed items or validated paths.
- **Fix**:
```diff
-        } else {
-            win32::executor::execute_target(&item_id, LaunchMode::Normal)?;
-            ExecuteOutcome::Launched
-        }
+        } else {
+            return Err(format!("Unknown or unindexed item id: {item_id}"));
+        }
```

---

### [MAJOR] [OPEN] F2-02 `autocomplete` Command Parameter Name Mismatch Breaks Path and Result Completion
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:255-259` and `src/ipc.ts:20-25`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: User presses `Tab` in the launcher input to autocomplete a path (`/Tools`) or complete the selected item. Tauri rejects the invoke call with a deserialization error (`missing field input` / `missing field selectedId`), and autocomplete fails completely.
- **Why**: In `commands.rs:255-259`, the Tauri command parameters are named `raw: String` and `top_item_title: Option<String>`. However, the frontend in `src/ipc.ts:24` invokes `autocomplete` passing `{ input, selectedId }` (`autocompleteQuery(input, selectedId)` in `src/main.ts:132`). In Tauri v2, invoke arguments must match Rust argument names exactly unless renamed. Because `input` and `selectedId` do not match `raw` and `top_item_title`, argument deserialization fails at runtime. Furthermore, the frontend passes `selectedId` (e.g. an item ID) instead of the top item's title.
- **Fix**:
```diff
 #[tauri::command]
 pub fn autocomplete(
-    raw: String,
-    top_item_title: Option<String>,
+    input: String,
+    selected_id: Option<String>,
     state: State<'_, AppState>,
 ) -> Result<Option<String>, String> {
-    let (mode, rest) = parse_prefix_mode(&raw);
+    let (mode, rest) = parse_prefix_mode(&input);
     if mode == PrefixMode::Path {
         let cfg = state.config.load();
         let exclusions = ExclusionFilter::new(&cfg.extra_exclusions);
         return Ok(path_nav::complete_path(rest, &exclusions).map(|p| format!("/{p}")));
     }
-    Ok(top_item_title)
+    if let Some(id) = selected_id {
+        let snapshot = state.search_engine.snapshot();
+        if let Some(item) = snapshot.items.iter().find(|i| i.id == id) {
+            return Ok(Some(item.title.clone()));
+        }
+    }
+    Ok(None)
 }
```

---

### [MAJOR] [OPEN] F2-03 Inline Preview Result Execution Passes `"__inline__"` and Fails to Copy
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:183-202` and `src/main.ts:81`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User types `= 12 * 8` or `100 usd in eur` and presses Enter to copy the calculation/unit result to clipboard. Instead of copying the calculated value, the backend executes `__inline__` as a filename, resulting in a `ShellExecuteW failed with code 2` (file not found) error, and nothing is copied.
- **Why**: In `src/main.ts:81`, when an inline preview is active and executed, the frontend sends `executeItem("__inline__", state.query, secondary)`. The backend `execute` command in `src-tauri/src/commands.rs` does not recognize `"__inline__"`. Instead, it looks for prefix `"copy:"`, searches `snapshot.items`, and falls through to `execute_target("__inline__", Normal)`, which fails.
- **Fix**:
```diff
     let outcome = if let Some(cmd) = item_id.strip_prefix("shell:") {
         let mode = if secondary {
             LaunchMode::ShellTerminal
         } else {
             LaunchMode::ShellHidden
         };
         win32::executor::execute_target(cmd, mode)?;
         ExecuteOutcome::Launched
     } else if let Some(hwnd_str) = item_id.strip_prefix("win:") {
         let hwnd = hwnd_str
             .parse::<isize>()
             .map_err(|_| format!("invalid window handle: {hwnd_str}"))?;
         win32::switcher::focus_window(hwnd)?;
         ExecuteOutcome::WindowFocused
     } else if let Some(copy_val) = item_id.strip_prefix("copy:") {
         win32::executor::copy_text_to_clipboard(copy_val)?;
         ExecuteOutcome::CopiedToClipboard {
             value: copy_val.to_string(),
         }
+    } else if item_id == "__inline__" {
+        let cfg = state.config.load();
+        let (mode, rest) = parse_prefix_mode(&state.last_query.lock().map(|q| q.clone()).unwrap_or_default());
+        let preview = inline_tools::evaluate(rest, mode == PrefixMode::Calc, &cfg.currency_rates);
+        if let Some(prev) = preview {
+            win32::executor::copy_text_to_clipboard(&prev.copy_value)?;
+            ExecuteOutcome::CopiedToClipboard {
+                value: prev.copy_value,
+            }
+        } else {
+            return Err("No active inline preview to copy".to_string());
+        }
```

---

### [MAJOR] [OPEN] F2-04 Frecency Store Atomic Persistence Discards In-Memory Updates Under Rapid Selection
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:177-181`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: Rapid item selections or concurrent execution calls cause `storage::save_frecency_atomic` to run while holding or immediately after releasing `state.frecency.lock()`. If `save_frecency_atomic` fails or disk writes interleave, there is no retry or queueing. More critically, holding the lock during serializing and disk I/O blocks concurrent query calls.
- **Why**: `storage::save_frecency_atomic` performs disk serialization and file I/O inside the selection block while holding `state.frecency.lock()`. Query evaluation (`commands::query`) also acquires `state.frecency.lock()` on every keystroke (`commands.rs:131`). Blocking the frecency mutex with synchronous file I/O causes UI lag on query typing immediately following an execution.
- **Fix**:
```diff
         if let Ok(mut frecency) = state.frecency.lock() {
             record_selection(&mut frecency, &prefix, &item_id, now_unix());
-            let _ = storage::save_frecency_atomic(&state.app_dir.join("frecency.json"), &frecency);
+            let cloned_frecency = frecency.clone();
+            let save_path = state.app_dir.join("frecency.json");
+            std::thread::spawn(move || {
+                let _ = storage::save_frecency_atomic(&save_path, &cloned_frecency);
+            });
         }
```

---

### [MINOR] [OPEN] F2-05 Single Instance Check Leaves Zombie Processes Silently Exiting Without Focusing Existing Window
- **Status**: OPEN
- **Where**: `src-tauri/src/lib.rs:24-40`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: User launches Launchify while an instance is already running (e.g. from Start Menu or autorun). The second instance exits silently without signaling the first instance or bringing the existing launcher into view.
- **Why**: `acquire_single_instance` in `src-tauri/src/lib.rs` simply returns `None` and exits `run()` if `ERROR_ALREADY_EXISTS` is encountered. It does not find the existing window (`FindWindowW`) or post a message (e.g. `WM_APP` or trigger event) to show the already running instance. The user is left wondering why nothing happened.
- **Fix**:
```diff
         if GetLastError() == ERROR_ALREADY_EXISTS {
             let _ = windows::Win32::Foundation::CloseHandle(handle);
+            // Optionally find existing window and show it, or emit event
             return None;
         }
```

---

### [MINOR] [OPEN] F2-06 Tray Menu "Open Config" Fails Silently If `config.json` Does Not Yet Exist On Disk
- **Status**: OPEN
- **Where**: `src-tauri/src/tray.rs:32-40`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: robustness
- **Scenario**: User clicks tray icon -> "Open Config" on a fresh installation where `config.json` was deleted or failed initial atomic write.
- **Why**: In `src-tauri/src/tray.rs:34`, `state.app_dir.join("config.json")` is passed directly to `execute_target(..., LaunchMode::Normal)`. If the file was not created or was removed, `ShellExecuteW` fails with file not found. If missing, it should ensure `config.json` is generated before opening.
- **Fix**:
```diff
             "open_config" => {
                 if let Some(state) = app.try_state::<AppState>() {
                     let config_path = state.app_dir.join("config.json");
+                    if !config_path.exists() {
+                        let cfg = state.config.load();
+                        let _ = crate::config::save_config_atomic(&state.app_dir, &cfg);
+                    }
                     let _ = win32::executor::execute_target(
                         &config_path.to_string_lossy(),
                         LaunchMode::Normal,
                     );
                 }
             }
```

---

### [MINOR] [OPEN] F2-07 `collapse_to_row` Inverts DPI Scaling Calculation
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:278-290`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: On high-DPI displays (e.g. 150% or 200% scale factor), `collapse_to_row` calculates `new_y` by multiplying `y_offset * scale` and adds it to `pos.y` (which is already a physical coordinate from `window.outer_position()`). Then `window.set_size(LogicalSize::new(LAUNCHER_WIDTH as f64, height.max(36) as f64))` resets the window size using logical size.
- **Why**: `pos` returned by `window.outer_position()` is in physical pixels (`tauri::PhysicalPosition`). Multiplying `y_offset` (which is already client DIPs or CSS pixels from the DOM) by `scale` converts it to physical pixels. However, `window.set_size(LogicalSize::...)` uses logical pixels. In `src/main.ts`, `collapse_to_row` is actually never called by the frontend codebase; it is dead/unwired IPC code, but if called, the mix of physical offset and logical resizing causes incorrect window placement.
- **Fix**:
```diff
 #[tauri::command]
 pub fn collapse_to_row(y_offset: i32, height: u32, app: AppHandle) -> Result<(), String> {
     if let Some(window) = app.get_webview_window("main") {
         let scale = window.scale_factor().unwrap_or(1.0);
         if let Ok(pos) = window.outer_position() {
             let new_y = pos.y + (f64::from(y_offset) * scale).round() as i32;
             let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, new_y));
         }
-        window
-            .set_size(LogicalSize::new(LAUNCHER_WIDTH as f64, height.max(36) as f64))
-            .map_err(|e| format!("failed to collapse launcher: {e}"))?;
+        window
+            .set_size(tauri::PhysicalSize::new(
+                ((LAUNCHER_WIDTH as f64) * scale).round() as u32,
+                ((height.max(36) as f64) * scale).round() as u32,
+            ))
+            .map_err(|e| format!("failed to collapse launcher: {e}"))?;
     }
     Ok(())
 }
```

---

### [ENHANCEMENT] [OPEN] F2-08 In-Memory Base64 Re-encoding of Cached PNG Icons on Every Keystroke
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:140-151`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: performance
- **Scenario**: When user types in Global search mode, each keystroke calls `commands::query`, which searches the index and iterates over up to `cfg.max_results` results. For every result with an `icon_path`, it reads the PNG from disk (`std::fs::read(&p)`) and computes `encode_base64(&bytes)` on the fly.
- **Why**: Reading from disk and performing Base64 encoding on every keystroke causes unnecessary synchronous disk I/O and heap allocations in the query hot path.
- **Fix**:
```diff
+// Cache base64 strings or serve via Tauri custom protocol / asset protocol `asset://` instead of reading disk and base64-encoding on every query keystroke.
```

---

### [ENHANCEMENT] [OPEN] F2-09 Dead IPC Handler `collapse_to_row` Not Referenced in Frontend
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:278-290`
- **Source**: `r-review` (Tier 1/2) (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: Frontend uses CSS animations (`is-launch-fading`, `is-launched`) and hides the window after a timeout (`src/main.ts:51-76`), never invoking `collapse_to_row`.
- **Why**: `collapse_to_row` is dead code in the Tauri IPC layer, increasing attack surface and maintenance burden without active usage. Either wire it into the launch animation or remove it.
- **Fix**: Remove `collapse_to_row` from `commands.rs` and `lib.rs` invoke handlers if CSS row animation is the chosen architecture.
