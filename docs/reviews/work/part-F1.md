# Review Findings Part F1: Win32 Hook, Window, Switcher & Executor

## Summary
- **Scope**: `src-tauri/src/win32/hook.rs`, `src-tauri/src/win32/window.rs`, `src-tauri/src/win32/switcher.rs`, `src-tauri/src/win32/executor.rs`, `src-tauri/src/win32/mod.rs`
- **Dual Lens**: Applied standard lens and adversarial F1b lens (security, concurrency/races, Win32 API contracts, resource leaks, edge-case failure modes).
- **Finding Counts**:
  - CRITICAL: 1
  - MAJOR: 3
  - MINOR: 3
  - ENHANCEMENT: 2

---

### [CRITICAL] [OPEN] F1-01 Remote/Arbitrary Code Execution via Unsanitized Command Injection in PowerShell Execution Modes
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:134-150`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: When running a shell target (e.g. from prefix `>`), an attacker or malicious URI / shortcut / IPC payload containing command delimiters or PowerShell syntax (or malicious target in `LaunchMode::ShellHidden` / `LaunchMode::ShellTerminal`) executes arbitrary PowerShell code under the current user's security context. Furthermore, in `LaunchMode::ShellTerminal`, `wt.exe powershell.exe -NoExit -EncodedCommand {encoded}` passes the argument through `wt.exe`'s command line parser where unquoted or space-separated elements cause argument injection or fail to execute if `target` contains specific sequences. More critically, `ShellHidden` and `ShellTerminal` execute arbitrary strings via `powershell.exe -EncodedCommand` without restriction or sandbox.
- **Why**: `encode_ps_command` encodes the arbitrary input string as UTF-16LE Base64 and executes it directly inside `powershell.exe` with `-EncodedCommand`. When `wt.exe` is invoked, `wt_args` is passed to `ShellExecuteW("open", "wt.exe", Some(&wt_args), SW_SHOWNORMAL)` where `wt.exe` interprets arguments according to Windows Terminal CLI rules, not raw Windows command lines.
- **Fix**:
```diff
--- a/src-tauri/src/win32/executor.rs
+++ b/src-tauri/src/win32/executor.rs
@@ -142,7 +142,7 @@ pub fn execute_target(target: &str, mode: LaunchMode) -> Result<(), String> {
             LaunchMode::ShellTerminal => {
                 let encoded = encode_ps_command(target);
-                let wt_args = format!("powershell.exe -NoExit -EncodedCommand {encoded}");
+                let wt_args = format!("-- powershell.exe -NoExit -EncodedCommand \"{encoded}\"");
                 if shell_execute("open", "wt.exe", Some(&wt_args), SW_SHOWNORMAL).is_ok() {
                     Ok(())
                 } else {
```

---

### [MAJOR] [OPEN] F1-02 Unchecked Process Handle Resource Leak and Incomplete Cloaked Window Filtering in Window Switcher
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/switcher.rs:63-71` & `src-tauri/src/win32/switcher.rs:105-120`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: On every window search (`@` prefix query typed by the user, triggered on keystrokes), `enumerate_visible_windows` runs `EnumWindows` over all desktop windows. For every non-cloaked visible window, `OpenProcess` is called. If `QueryFullProcessImageNameW` returns an error or when `len == 0`, or in edge conditions, the process handle must be closed. While `CloseHandle` is called right after `QueryFullProcessImageNameW`, if `QueryFullProcessImageNameW` returns buffer errors or `OpenProcess` fails repeatedly across hundreds of windows every keystroke, high handle turnover occurs. More critically: `DwmGetWindowAttribute` can return an `HRESULT` failure (e.g. for elevated or protected windows). If it fails, `cloaked` remains 0, erroneously classifying invisible/suspended UWP app windows (ApplicationFrameWindow cloaked in background/other virtual desktops) as visible windows, flooding the switcher list with phantom windows.
- **Why**: `let _ = DwmGetWindowAttribute(...)` ignores the `HRESULT`. If DWM fails or denies access, `cloaked` defaults to 0, so background/cloaked windows pass the filter.
- **Fix**:
```diff
--- a/src-tauri/src/win32/switcher.rs
+++ b/src-tauri/src/win32/switcher.rs
@@ -63,7 +63,7 @@ mod win_impl {
         let mut cloaked: u32 = 0;
-        let _ = DwmGetWindowAttribute(
+        let hr = DwmGetWindowAttribute(
             hwnd,
             DWMWA_CLOAKED,
             (&mut cloaked as *mut u32).cast(),
             std::mem::size_of::<u32>() as u32,
         );
-        if cloaked != 0 {
+        if hr.is_err() || cloaked != 0 {
             return true.into();
         }
```

---

### [MAJOR] [OPEN] F1-03 Unsynchronized Cross-Thread Hook Destruction and Message Loop Deadlock Potential
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:216-225` & `src-tauri/src/win32/hook.rs:238-248`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: When `WinKeyHookHandle` is dropped (e.g., during app reload, reconfiguration, or shutdown in `AppState`), `stop_hook_thread` posts `WM_QUIT` to `thread_id` and joins the thread handle. However, `PostThreadMessageW` can fail silently (and its return value is discarded with `let _`) if the target thread has not yet created its message queue or if it is currently inside a blocking Win32 call. Furthermore, during app shutdown or crash scenarios, if `join()` blocks indefinitely because `PostThreadMessageW` was posted before `GetMessageW` was reached or if the message queue was purged, the main thread hangs permanently.
- **Why**: `PostThreadMessageW` returns a `BOOL` that indicates whether the message was placed in the thread's message queue. If called before the message queue is initialized (or if dropped immediately after start), `PostThreadMessage` fails with `ERROR_INVALID_THREAD_ID`. Additionally, `UnhookWindowsHookEx` is only called after `GetMessageW` exits; if `join()` hangs, the global low-level hook remains hooked in the OS chain.
- **Fix**:
```diff
--- a/src-tauri/src/win32/hook.rs
+++ b/src-tauri/src/win32/hook.rs
@@ -242,7 +242,12 @@ mod win_impl {
     ) {
         unsafe {
-            let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
+            for _ in 0..10 {
+                if PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).as_bool() {
+                    break;
+                }
+                std::thread::sleep(std::time::Duration::from_millis(10));
+            }
         }
         if let Some(handle) = join_handle {
             let _ = handle.join();
```

---

### [MAJOR] [OPEN] F1-04 `SetForegroundWindow` Focus Stealing Failure on Modern Windows
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/window.rs:153-159` & `src-tauri/src/win32/window.rs:210-223` & `src-tauri/src/win32/switcher.rs:150-156`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: In `switcher.rs:focus_window`, `SetForegroundWindow(hwnd)` is called directly without `AttachThreadInput` or `LockSetForegroundWindow(LSFW_UNLOCK)`. On Windows 10/11, when a background process (or even Launchify if its window was just hidden or losing focus) calls `SetForegroundWindow`, Windows Foreground Lock rules prohibit the window from taking focus, causing the target window to only flash on the taskbar instead of activating. `focus_window` then returns `Err("SetForegroundWindow failed for hwnd ...")`, breaking the `@` switcher.
- **Why**: Windows explicitly restricts `SetForegroundWindow` unless the caller process is the current foreground process, received the last input event, or attaches its input queue via `AttachThreadInput` to the foreground thread. `window.rs` uses `AttachThreadInput`, but `switcher.rs` does not.
- **Fix**:
```diff
--- a/src-tauri/src/win32/switcher.rs
+++ b/src-tauri/src/win32/switcher.rs
@@ -43,6 +43,7 @@ mod win_impl {
     use windows::Win32::System::Threading::{
-        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
+        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
         PROCESS_QUERY_LIMITED_INFORMATION,
     };
     use windows::Win32::UI::WindowsAndMessaging::{
-        EnumWindows, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
+        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
         GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow,
@@ -152,7 +153,16 @@ mod win_impl {
                 let _ = ShowWindow(hwnd, SW_RESTORE);
             }
+            let fg = GetForegroundWindow();
+            let fg_tid = if !fg.0.is_null() { GetWindowThreadProcessId(fg, None) } else { 0 };
+            let cur_tid = GetCurrentThreadId();
+            let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, true).as_bool();
+            let _ = BringWindowToTop(hwnd);
+            let ok = SetForegroundWindow(hwnd).as_bool();
+            if attached {
+                let _ = AttachThreadInput(cur_tid, fg_tid, false);
+            }
-            if !SetForegroundWindow(hwnd).as_bool() {
+            if !ok {
                 return Err(format!("SetForegroundWindow failed for hwnd {hwnd_raw}"));
             }
```

---

### [MINOR] [OPEN] F1-05 Memory Leak in `copy_text_to_clipboard` if `OpenClipboard` or `EmptyClipboard` Fails
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:160-176`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: In `copy_text_to_clipboard`, `GlobalAlloc(GMEM_MOVEABLE, byte_len)` allocates unmanaged global memory. If `SetClipboardData` fails, `GlobalFree(hmem)` is called. However, if the allocation succeeded and an error occurs between `GlobalAlloc` and `SetClipboardData` or if ownership transfer fails, memory must be freed. Furthermore, once `SetClipboardData` succeeds, the system owns the memory handle and the application must NOT free it. While this is correctly handled in the success path, `OpenClipboard(None)` can fail if another process currently holds the clipboard open (e.g. clipboard managers, office apps). Instead of retrying briefly (standard Win32 clipboard practice with a loop of 5-10 retries with backoff), it fails immediately.
- **Why**: `OpenClipboard` frequently returns `ERROR_ACCESS_DENIED` when clipboard monitor apps or browsers are reading the clipboard. Immediate failure causes user copy actions to fail spuriously.
- **Fix**:
```diff
--- a/src-tauri/src/win32/executor.rs
+++ b/src-tauri/src/win32/executor.rs
@@ -158,3 +158,11 @@ mod win_impl {
         unsafe {
-            OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {e}"))?;
+            let mut opened = false;
+            for _ in 0..10 {
+                if OpenClipboard(None).is_ok() {
+                    opened = true;
+                    break;
+                }
+                std::thread::sleep(std::time::Duration::from_millis(20));
+            }
+            if !opened {
+                return Err("Failed to open clipboard (busy)".to_string());
+            }
```

---

### [MINOR] [OPEN] F1-06 Hook State Machine Desynchronization on Lost Key Up Events
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:42-57`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: In `WinKeyStateMachine`, `lwin_down` and `shift_down` are updated only on explicit `VK_LWIN_CODE`, `VK_LSHIFT_CODE`, or `VK_RSHIFT_CODE` events. If the user presses `Win+Shift`, then an OS dialogue (UAC prompt, Ctrl+Alt+Del, lock screen, or another low-level hook that drops events) intercepts the release (`KEYUP`), `self.lwin_down` or `self.shift_down` remains stuck in `true`. Subsequent single presses of `F23` or future key actions will falsely trigger `TriggerSoloWin`. Moreover, unit tests in `hook.rs` (lines 255-313) test solo `VK_LWIN` tap behavior from an older design (`solo_lwin_tap_triggers_launcher`), but `on_key_event` implementation was changed to only trigger on `VK_F23_CODE && is_key_down && self.lwin_down && self.shift_down`, meaning the unit tests in `hook.rs:255-313` are actually testing outdated assumptions or will fail.
- **Why**: The state machine tracks modifier states purely via event delta without verifying physical state via `GetAsyncKeyState`, and test cases are out of sync with Copilot key logic.
- **Fix**:
```diff
--- a/src-tauri/src/win32/hook.rs
+++ b/src-tauri/src/win32/hook.rs
@@ -51,6 +51,10 @@ impl WinKeyStateMachine {
             return HookDecision::PassThrough;
         }
 
+        #[cfg(windows)]
+        let lwin_actual = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(VK_LWIN_CODE as i32) < 0 };
+        #[cfg(not(windows))]
+        let lwin_actual = self.lwin_down;
         if vk == VK_F23_CODE && is_key_down && self.lwin_down && self.shift_down {
             return HookDecision::TriggerSoloWin;
         }
```

---

### [MINOR] [OPEN] F1-07 `RevealInExplorer` Fails or Behaves Incorrectly on Targets with Quotes or Commas
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:130-133`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When `LaunchMode::RevealInExplorer` is called with a folder or file target containing double quotes (e.g. `C:\My "Special" Folder`), the argument string is formatted as `format!("/select,\"{target}\"")`. Unescaped quotes break the `explorer.exe` `/select,` argument parser, causing Explorer either to fail to open or to open the default Documents directory instead of highlighting the selected item.
- **Why**: Explorer command line arguments require path sanitization and quote stripping/escaping.
- **Fix**:
```diff
--- a/src-tauri/src/win32/executor.rs
+++ b/src-tauri/src/win32/executor.rs
@@ -130,4 +130,5 @@ mod win_impl {
             LaunchMode::RevealInExplorer => {
-                let args = format!("/select,\"{target}\"");
+                let sanitized = target.replace('"', "");
+                let args = format!("/select,\"{sanitized}\"");
                 shell_execute("open", "explorer.exe", Some(&args), SW_SHOWNORMAL)
             }
```

---

### [ENHANCEMENT] [OPEN] F1-08 [lens: F1b] `SetWindowPos` Fails to Re-anchor Correctly When Work Area DPI Differs from Window DPI
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/window.rs:180-205`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: In `show_and_focus_launcher`, `GetDpiForMonitor` is queried for the target monitor, while `logical_width` (680) is scaled. However, `SetWindowPos` is called with `SWP_NOSIZE`. If the launcher window was previously on a 100% DPI monitor (width 680 physical pixels) and the cursor is now on a 200% DPI monitor (where width should be 1360 physical pixels), `SetWindowPos` only repositions `(x, y)` without resizing the window, causing the window to appear with incorrect dimensions until Tauri's webview eventually fires a resize event.
- **Why**: `SWP_NOSIZE` prevents `SetWindowPos` from setting the physical width and height corresponding to the destination monitor's DPI scale factor.
- **Fix**: Pass calculated physical width/height and omit `SWP_NOSIZE` when moving across monitors with differing DPI scales.

---

### [ENHANCEMENT] [OPEN] F1-09 [lens: F1b] Start Menu Mask Key Injection Floods Input Queue with Redundant `VK_MASK_DUMMY`
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:150-153` & `src-tauri/src/win32/hook.rs:161-189`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: robustness
- **Scenario**: When `TriggerSoloWin` fires on Copilot key (`VK_F23`), `inject_mask_key()` sends both key-down and key-up for `VK_MASK_DUMMY` (0xFF). While harmless, for Copilot key (`Win+Shift+F23`), Windows 11 does not trigger the Start Menu on `Win+Shift+F23` release because `Shift` and `F23` already prevent Windows from treating it as a solo Windows key release. Injecting `VK_MASK_DUMMY` is only necessary when suppressing a true solo `VK_LWIN` tap.
- **Why**: The Copilot key sequence (`Win+Shift+F23`) already breaks the solo-Win sequence in Windows shell input manager. Injecting extra synthetic key events can interfere with low-latency games or applications monitoring raw input.
- **Fix**: Remove unnecessary dummy key injection or guard it so it only fires if solo-Win suppression is specifically desired.
