# Review Findings — Launchify (Tier 2 Full Codebase Audit)

**Date:** 2026-11-19  
**Scope:** Full repository (`src-tauri/src/**`, `src/**`, `tauri.conf.json`, `index.html`)  
**Auditors:** Tier 2 Modular Review (F1-F5, X1, V1 Verifier)  
**Summary:** `CRITICAL: 2` | `MAJOR: 8` | `MINOR: 15` | `ENHANCEMENT: 7`

---

## CRITICAL

### [CRITICAL] [FIXED] F1-01 Remote/Arbitrary Code Execution via Unsanitized Command Injection in PowerShell Execution Modes
- **Status**: FIXED
- **Where**: `src-tauri/src/win32/executor.rs:134-150`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: When running a shell target (e.g. from prefix `>`) or when an arbitrary string reaches `execute_target(..., LaunchMode::ShellHidden | ShellTerminal)`, `powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {encoded}` is spawned. Furthermore, when `wt.exe` is invoked, `format!("wt.exe powershell.exe -NoExit -EncodedCommand {encoded}")` passes the string to `wt.exe` without argument escaping or `--` boundary, allowing argument confusion / command hijacking in the Windows Terminal CLI parser.
- **Why**: `encode_ps_command` encodes arbitrary target strings into Base64 UTF-16LE without validating or isolating the command. In terminal mode, parameters passed to `wt.exe` are unquoted and lack the `--` separator.
- **Fix**:
```diff
--- a/src-tauri/src/win32/executor.rs
+++ b/src-tauri/src/win32/executor.rs
@@ -141,7 +141,7 @@ pub fn execute_target(target: &str, mode: LaunchMode) -> Result<(), String> {
             }
             LaunchMode::ShellTerminal => {
                 let encoded = encode_ps_command(target);
-                let wt_args = format!("powershell.exe -NoExit -EncodedCommand {encoded}");
+                let wt_args = format!("-- powershell.exe -NoExit -EncodedCommand \"{encoded}\"");
                 if shell_execute("open", "wt.exe", Some(&wt_args), SW_SHOWNORMAL).is_err() {
                     let ps_args = format!("-NoExit -EncodedCommand {encoded}");
                     shell_execute("open", "powershell.exe", Some(&ps_args), SW_SHOWNORMAL)?;
```

### [CRITICAL] [FIXED] F2-01 IPC `execute` Arbitrary Target Fallback Allows Execution of Untrusted Input
- **Status**: FIXED
- **Where**: `src-tauri/src/commands.rs:244-247`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: If a script in the webview invokes `invoke("execute", { raw: "calc.exe", action: "Open" })` with any string not prefixed by `shell:`, `win:`, or `copy:` and not present in `snapshot.items`, `commands.rs` falls through to `win32::executor::execute_target(&item_id, LaunchMode::Normal)`. This breaks the trust boundary: any unindexed binary or command string sent via IPC is executed on the host.
- **Why**: Missing validation / whitelist check for unindexed item IDs in the IPC execution sink.
- **Fix**:
```diff
--- a/src-tauri/src/commands.rs
+++ b/src-tauri/src/commands.rs
@@ -244,4 +244,4 @@ pub fn execute(
         } else {
-            win32::executor::execute_target(&item_id, LaunchMode::Normal)?;
-            ExecuteOutcome::Launched
+            return Err(format!("Unknown or unindexed item id: {item_id}"));
         }
```

---

## MAJOR

### [MAJOR] [FIXED] F1-02 Unchecked Process Handle Resource Leak and Incomplete Cloaked Window Filtering in Window Switcher
- **Status**: FIXED
- **Where**: `src-tauri/src/win32/switcher.rs:63-71`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When enumerating windows via `@` mode, `DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, ...)` can fail (e.g. access denied on elevated windows or during UAC transitions). The return `HRESULT` is ignored (`let _ = ...`), leaving `cloaked` as 0 and including invisible UWP background apps in switcher results.
- **Why**: Failure to check the HRESULT of `DwmGetWindowAttribute` causes uninitialized or zero-defaulted cloaked status to pass through.
- **Fix**:
```diff
--- a/src-tauri/src/win32/switcher.rs
+++ b/src-tauri/src/win32/switcher.rs
@@ -66,3 +66,3 @@ unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
         let mut cloaked: u32 = 0;
-        let _ = DwmGetWindowAttribute(
+        let hr = DwmGetWindowAttribute(
             hwnd,
@@ -71,2 +71,5 @@ unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
         );
+        if hr.is_err() || cloaked != 0 {
+            return true.into();
+        }
```

### [MAJOR] [FIXED] F1-04 `SetForegroundWindow` Focus Stealing Failure on Modern Windows
- **Status**: FIXED
- **Where**: `src-tauri/src/win32/switcher.rs:144-158`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When selecting an open window to switch to in `@` mode, `SetForegroundWindow(hwnd)` is called alone. On Windows 10/11, if Launchify does not currently hold foreground permission lock, `SetForegroundWindow` fails: the target window only blinks orange on the taskbar and the method returns `Err("SetForegroundWindow failed")`.
- **Why**: Windows restricts background or losing-focus processes from setting the foreground window without attaching input queues (`AttachThreadInput`) or calling `BringWindowToTop`.
- **Fix**:
```diff
--- a/src-tauri/src/win32/switcher.rs
+++ b/src-tauri/src/win32/switcher.rs
@@ -150,2 +150,7 @@ pub fn focus_window(hwnd_val: isize) -> Result<(), String> {
             if IsIconic(hwnd).as_bool() {
                 let _ = ShowWindow(hwnd, SW_RESTORE);
             }
+            let cur_thread = GetCurrentThreadId();
+            let target_thread = GetWindowThreadProcessId(hwnd, None);
+            let _ = AttachThreadInput(cur_thread, target_thread, true);
+            let _ = BringWindowToTop(hwnd);
             let ok = SetForegroundWindow(hwnd).as_bool();
+            let _ = AttachThreadInput(cur_thread, target_thread, false);
```

### [MAJOR] [FIXED] F2-02 / X-contract-01 IPC Parameter Name Mismatch in `autocomplete` Causes Complete Feature Failure
- **Status**: FIXED
- **Where**: `src/ipc.ts:20-25` vs `src-tauri/src/commands.rs:254-259`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: When user presses `Tab` to autocomplete a path or result, `ipc.ts` calls `invoke("autocomplete", { input, selectedId })`. On the Rust backend, the command expects `{ raw, top_item_title }`. Tauri v2 parameter deserialization rejects the invoke with `missing field 'raw'`, breaking Tab autocompletion completely.
- **Why**: Mismatch in JSON argument keys between TypeScript invoke wrapper and Rust command signature.
- **Fix**:
```diff
--- a/src/ipc.ts
+++ b/src/ipc.ts
@@ -21,5 +21,5 @@ export async function executeItem(
 export async function autocompleteQuery(
-  input: string,
-  selectedId: string | null
+  raw: string,
+  topItemTitle: string | null
 ): Promise<string | null> {
-  return invoke<string | null>("autocomplete", { input, selectedId });
+  return invoke<string | null>("autocomplete", { raw, topItemTitle });
 }
```

### [MAJOR] [FIXED] F2-03 / X-contract-03 Inline Preview Result Execution Passes `"__inline__"` and Fails to Copy
- **Status**: FIXED
- **Where**: `src/main.ts:81` vs `src-tauri/src/commands.rs:183-202`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: User computes an inline calculation (`= 2 + 2`) and presses `Enter`. `main.ts` calls `executeItem("__inline__", state.query, secondary)`. The backend does not handle `"__inline__"`, searches `snapshot.items`, fails to find it, and falls through to `execute_target("__inline__", Normal)`, failing with a ShellExecute error. The calculation result is never copied to the clipboard.
- **Why**: Backend `commands.rs::execute` only handles `copy:` prefix, while frontend sends `"__inline__"` as the item ID.
- **Fix**:
```diff
--- a/src-tauri/src/commands.rs
+++ b/src-tauri/src/commands.rs
@@ -193,2 +193,7 @@ pub fn execute(
             return Ok(ExecuteOutcome::CopiedToClipboard { value: text.to_string() });
+        } else if item_id == "__inline__" {
+            if let Some(preview) = inline_tools::evaluate(&raw, true, &cfg.currency_rates) {
+                win32::executor::copy_text_to_clipboard(&preview.copy_value)?;
+                return Ok(ExecuteOutcome::CopiedToClipboard { value: preview.copy_value });
+            }
         }
```

### [MAJOR] [FIXED] F3-01 Start Menu Scanner Hardcodes `icon_key` Breaking UI Icon Display
- **Status**: FIXED
- **Where**: `src-tauri/src/indexer/scanner.rs:104`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: In `scan_lnk_dir`, items are initialized with `icon_key: Some(format!("app_{stem}"))`. In `IndexManager::rebuild_now`, `icon_key` is only updated if extraction succeeds. If extraction fails or is skipped, `icon_key` retains the dummy value `app_Code`, and frontend requests a nonexistent icon path.
- **Why**: Pre-filling a non-hashed dummy icon key violates the invariant that `icon_key` is either a valid FNV-1a hash or `None`.
- **Fix**:
```diff
--- a/src-tauri/src/indexer/scanner.rs
+++ b/src-tauri/src/indexer/scanner.rs
@@ -104,3 +104,3 @@ fn scan_lnk_dir(dir: &Path, is_custom: bool, items: &mut Vec<IndexItem>) {
             target: path.to_string_lossy().to_string(),
-            icon_key: Some(format!("app_{stem}")),
+            icon_key: None,
         });
```

### [MAJOR] [FIXED] F3-02 Windows `extract_png_for_target` Leaks COM Apartment Initialization
- **Status**: FIXED
- **Where**: `src-tauri/src/icons.rs:77-79`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: `extract_png_for_target` calls `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` per icon extracted without ever calling `CoUninitialize()`. When scanning dozens of shortcuts in background threads, this leaks COM reference counts and can conflict with multi-threaded apartments (`RPC_E_CHANGED_MODE`).
- **Why**: Missing RAII guard or cleanup `CoUninitialize()` call.
- **Fix**:
```diff
--- a/src-tauri/src/icons.rs
+++ b/src-tauri/src/icons.rs
@@ -77,2 +77,7 @@ pub fn extract_png_for_target(target: &str, output_path: &Path) -> Result<(), St
         let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
+        let _com_guard = scopeguard::guard(hr, |res| {
+            if res.is_ok() {
+                CoUninitialize();
+            }
+        });
```

### [MAJOR] [FIXED] F4-01 Atomic Write Temp File Collision and Incomplete Replace Semantics on Windows
- **Status**: FIXED
- **Where**: `src-tauri/src/storage.rs:18-22`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: In `write_atomic`, `path.with_extension(format!("{}.{}.tmp", ...))` is renamed via `std::fs::rename(tmp, path)`. On Windows, standard `std::fs::rename` calls `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, which fails with `ERROR_ALREADY_EXISTS` if `path` already exists on disk.
- **Why**: Rust's standard `fs::rename` on Windows does not replace existing destination files atomically without explicit replacement handling.
- **Fix**:
```diff
--- a/src-tauri/src/storage.rs
+++ b/src-tauri/src/storage.rs
@@ -20,2 +20,4 @@ fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
     std::fs::write(&tmp_path, bytes).map_err(|e| format!("Failed to write temp file: {e}"))?;
+    #[cfg(windows)]
+    let _ = std::fs::remove_file(path);
     std::fs::rename(&tmp_path, path).map_err(|e| format!("Failed to rename temp file: {e}"))?;
```

### [MAJOR] [FIXED] F4-03 `browse_path` and `complete_path` on Windows Prepend Forward Slash to Absolute Paths
- **Status**: FIXED
- **Where**: `src-tauri/src/path_nav.rs:77`, `150-152`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When typing `C:\` or completing a drive path, `format_autocomplete` formats the Windows path as `/C:\Tools\`. On subsequent search or execution, shells and Windows APIs fail to resolve paths starting with `/C:`.
- **Why**: Unix-centric path formatting logic unconditionally adds a leading slash.
- **Fix**:
```diff
--- a/src-tauri/src/path_nav.rs
+++ b/src-tauri/src/path_nav.rs
@@ -150,3 +150,7 @@ fn format_autocomplete(dir: &Path, file_name: &str) -> String {
     let full = dir.join(file_name);
-    format!("{}/", full.to_string_lossy().replace('\\', "/"))
+    let s = full.to_string_lossy();
+    if s.chars().nth(1) == Some(':') {
+        format!("{}\\", s.trim_end_matches(['\\', '/']))
+    } else {
+        format!("{}/", s.replace('\\', "/"))
+    }
```

### [MAJOR] [FIXED] F5-02 Race Condition in Launch Fade Animation Leaves Launcher Hidden on Fast Re-open
- **Status**: FIXED
- **Where**: `src/main.ts:67-75`, `src/main.ts:153-176`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User launches an app via `Enter`, triggering a 650ms fade-out timer. If the user presses the Copilot key to re-open Launchify within that 650ms, the old timeout still fires and unconditionally hides the newly opened launcher window.
- **Why**: Active timer handle is unmanaged and not cancelled when `onLauncherShown` fires.
- **Fix**:
```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -21,2 +21,3 @@ function bootstrap(): void {
   let showTimer: number | null = null;
+  let executeTimer: number | null = null;
@@ -70,3 +71,4 @@ function bootstrap(): void {
-    setTimeout(() => {
+    executeTimer = window.setTimeout(() => {
       renderVisibility(refs, false);
       void hideLauncher();
+      executeTimer = null;
     }, 650);
@@ -160,2 +162,6 @@ function bootstrap(): void {
     if (showTimer !== null) {
       clearTimeout(showTimer);
+    }
+    if (executeTimer !== null) {
+      clearTimeout(executeTimer);
+      executeTimer = null;
     }
```

### [MAJOR] [FIXED] F5-03 Window Blur Event Prematurely Dismisses Launcher on Focus Transitions
- **Status**: FIXED
- **Where**: `src/keyboard.ts:61-63`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: During window focus initialization, clicking WebView controls, or opening IME candidate windows, the browser window temporarily fires a `blur` event. `onWindowBlur` immediately invokes `handlers.onHide()`, abruptly closing the launcher.
- **Why**: Unconditional immediate hide on `blur` without checking `document.hasFocus()` or debouncing.
- **Fix**:
```diff
--- a/src/keyboard.ts
+++ b/src/keyboard.ts
@@ -61,3 +61,7 @@ export function setupKeyboardNavigation(
   const onWindowBlur = (): void => {
-    handlers.onHide();
+    requestAnimationFrame(() => {
+      if (!document.hasFocus()) {
+        handlers.onHide();
+      }
+    });
   };
```

---

## MINOR

- **F1-03** (`src-tauri/src/win32/hook.rs:216-225`): Hook thread shutdown uses `PostThreadMessageW` without retry or error logging on thread termination.
- **F1-05** (`src-tauri/src/win32/executor.rs:160-179`): Memory leak in `copy_text_to_clipboard` if `GlobalAlloc` succeeds but `OpenClipboard` or `EmptyClipboard` fails before ownership is transferred.
- **F1-06** (`src-tauri/src/win32/hook.rs:161-190`): Hook state machine can desynchronize if `WM_KEYUP` events are dropped or intercepted by elevated apps.
- **F1-07** (`src-tauri/src/win32/executor.rs:21-39`): `RevealInExplorer` fails on targets containing double quotes or commas due to `/select,"path"` escaping issues.
- **F2-04** (`src-tauri/src/commands.rs:177-181`): Synchronous file I/O inside mutex lock during `record_selection` blocks query threads on disk writes.
- **F2-05** (`src-tauri/src/lib.rs:24-32`): Single instance mutex check silently exits secondary instance without bringing the existing instance to the foreground.
- **F2-06** (`src-tauri/src/tray.rs:45-51`): Tray menu "Open Config" fails silently if `config.json` has not yet been initialized on disk.
- **F2-07** (`src-tauri/src/commands.rs:280-285`): `collapse_to_row` inverts DPI scaling calculation when resizing.
- **F3-03** (`src-tauri/src/search/engine.rs:112`): `top_frecency_items` and `search` use `select_nth_unstable_by` with reversed comparison against subsequent `sort_by`.
- **F3-04** (`src-tauri/src/indexer/scanner.rs:45`): `scan_apps_folder` is an empty stub returning no modern packaged UWP apps.
- **F3-05** (`src-tauri/src/indexer/mod.rs:92`): 64-bit FNV-1a hash in `IndexManager` risks potential collisions across large sets of indexed files.
- **F4-04** (`src-tauri/src/inline_tools/utils.rs:65`): `parse_iso_utc` day count calculation does not validate days per month (e.g. accepts February 31).
- **F4-05** (`src-tauri/src/inline_tools/math.rs:32`): `eval_math` lacks recursion depth limiter, vulnerable to stack overflow on deeply nested parentheses.
- **F4-06** (`src-tauri/src/inline_tools/utils.rs:112`): `generate_uuid_v4` uses pseudo-random number generator susceptible to collisions under rapid invocations.
- **F5-01** (`src/renderer.ts:85-91`): `#selection-indicator` does not account for `scrollTop` if list container scrolls.
- **F5-04** (`src/renderer.ts:110`): Missing `aria-activedescendant` attribute relationship between search input and active list item.
- **F5-05** (`src/main.ts:84`): `executeSelected` catches execution errors silently without presenting an error toast or visual cue to the user.
- **F5-06** (`src/main.ts:50`): Unhandled promise rejections on window resize IPC calls.
- **X-contract-02** (`src-tauri/src/commands.rs:140-151`): Double serialization and Base64 encoding overhead for cached PNG icons.
- **X-contract-04** (`src-tauri/src/catalog.rs:82-83` vs `src-tauri/src/win32/executor.rs:182-195`): `catalog.rs` includes `power:signout` and `power:hibernate`, but `system_power_action` only implements lock, sleep, restart, shutdown, returning errors for signout/hibernate.
- **X-contract-05** (`src-tauri/src/commands.rs:275`): Dead IPC command `collapse_to_row` exposed by backend but never called by frontend.
- **X-contract-06** (`src-tauri/src/model.rs:104`): Adjacently tagged `ExecuteOutcome` serde enum serialization nesting risk.

---

## ENHANCEMENT

- **F1-08** (`src-tauri/src/win32/window.rs:82`): `SetWindowPos` should omit `SWP_NOSIZE` when re-anchoring across monitors with different DPI scale factors.
- **F1-09** (`src-tauri/src/win32/hook.rs:150`): Avoid injecting redundant `VK_MASK_DUMMY` keystrokes when Copilot key (`Win+Shift+F23`) is pressed.
- **F2-08** (`src-tauri/src/commands.rs:145`): Cache Base64 string representations in `IconCache` rather than re-reading disk and re-encoding PNG bytes on every query keystroke.
- **F3-06** (`src-tauri/src/search/ranking.rs:52`): Precompute or reuse word boundary indices instead of allocating a `Vec<usize>` per candidate item.
- **F4-07** (`src-tauri/src/inline_tools/units.rs:88`): Guard currency conversion against division-by-zero on malformed exchange rate values.
- **F5-07** (`src/main.ts:100`): Debounce or throttle search input events by ~15ms to eliminate IPC spam during burst typing.
- **X-contract-07** (`src-tauri/src/config.rs:31` vs `src/state.ts:3`): Expose a `get_config` Tauri IPC command so frontend can dynamically adapt `max_results` and theme settings.
