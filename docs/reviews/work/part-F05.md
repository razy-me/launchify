# F05 — Win32 Layer (Executor, Hook, Switcher, Window)

### [MAJOR] [OPEN] F05-01 `system_power_action("sleep")` hibernates instead of sleeping due to `rundll32` signature mismatch
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:186-191`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User invokes the `sleep` power action on a Windows machine with hibernation enabled -> `rundll32.exe` invokes `powrprof.dll!SetSuspendState` using the `rundll32` callback ABI (`HWND, HINSTANCE, LPSTR, int`), passing a non-zero `HWND` as the first parameter (`bHibernate`), which hibernates the machine instead of sleeping.
- **Why**: `SetSuspendState(BOOLEAN bHibernate, BOOLEAN bForce, BOOLEAN bWakeupEventsDisabled)` is not a `rundll32`-compatible entry point; `rundll32.exe` ignores the `0,1,0` string as parsed arguments and passes its own window handle as `bHibernate`. Calling `SetSuspendState` directly via Win32 FFI (or `powrprof` in the `windows` crate) avoids the ABI mismatch and eliminates the `rundll32.exe` subprocess.
- **Fix**:
```diff
-            "sleep" => shell_execute(
-                "open",
-                "rundll32.exe",
-                Some("powrprof.dll,SetSuspendState 0,1,0"),
-                SW_HIDE,
-            ),
+            "sleep" => {
+                // Call SetSuspendState(bHibernate=false, bForce=false, bWakeupEventsDisabled=false) directly
+                // via windows::Win32::System::Power::SetSuspendState instead of rundll32.exe.
+                let ok = unsafe {
+                    windows::Win32::System::Power::SetSuspendState(false, false, false)
+                };
+                if ok {
+                    Ok(())
+                } else {
+                    Err("SetSuspendState failed".to_string())
+                }
+            }
```

### [MAJOR] [OPEN] F05-02 `LAlt+RShift` keyboard layout switch falsely triggers the launcher
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:19-60`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User presses `Left Alt` + `Right Shift` (standard Windows keyboard layout switch) -> `LAlt` down sets `trigger_down = true`, `RShift` down hits the same branch and leaves `other_key_pressed = false`, and releasing either key returns `HookDecision::Trigger`, unexpectedly opening the launcher.
- **Why**: `WinKeyStateMachine` tracks `trigger_down: bool` without recording which trigger key (`VK_TRIGGER_CODE` vs `VK_RSHIFT_CODE`) initiated the press, so pressing the second trigger key while the first is held is not treated as an `other_key_pressed` combo.
- **Fix**:
```diff
 pub struct WinKeyStateMachine {
-    trigger_down: bool,
+    active_trigger: Option<u16>,
     other_key_pressed: bool,
 }
@@ -33,22 +33,25 @@ impl WinKeyStateMachine {
         }
 
         if vk == VK_TRIGGER_CODE || vk == VK_RSHIFT_CODE {
             if is_key_down {
-                if !self.trigger_down {
-                    self.trigger_down = true;
+                match self.active_trigger {
+                    None => {
+                        self.active_trigger = Some(vk);
+                        self.other_key_pressed = false;
+                    }
+                    Some(active) if active != vk => {
+                        self.other_key_pressed = true;
+                    }
+                    _ => {}
+                }
+                return HookDecision::PassThrough;
+            } else {
+                let should_trigger =
+                    self.active_trigger == Some(vk) && !self.other_key_pressed;
+                if self.active_trigger == Some(vk) {
+                    self.active_trigger = None;
                     self.other_key_pressed = false;
                 }
-                return HookDecision::PassThrough;
-            } else {
-                let should_trigger = self.trigger_down && !self.other_key_pressed;
-                self.trigger_down = false;
-                self.other_key_pressed = false;
                 return if should_trigger {
                     HookDecision::Trigger
                 } else {
                     HookDecision::PassThrough
                 };
             }
         }
 
-        if self.trigger_down && is_key_down {
+        if self.active_trigger.is_some() && is_key_down {
             self.other_key_pressed = true;
         }
```

### [MAJOR] [OPEN] F05-03 Potential deadlock in `WinKeyHookHandle::drop` due to `PostThreadMessageW` startup race (`lens: F1b`)
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:162-176`, `src-tauri/src/win32/hook.rs:195-205`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: `WinKeyHookHandle::start` returns and the handle is dropped before the newly spawned hook thread executes `GetMessageW` -> `PostThreadMessageW(thread_id, WM_QUIT, ...)` fails with `ERROR_INVALID_THREAD_ID` because the thread's message queue has not been created yet; the hook thread then blocks in `GetMessageW` forever while `stop_hook_thread` deadlocks in `handle.join()`.
- **Why**: In Win32, a thread's message queue is created lazily on its first message-queue call (`PeekMessageW`/`GetMessageW`). Signaling `ready_tx.send(Ok(tid))` before forcing queue creation via `PeekMessageW` creates a classic startup race with `PostThreadMessageW`.
- **Fix**:
```diff
                 let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0) {
                     Ok(h) => h,
                     Err(e) => {
                         let _ = ready_tx.send(Err(format!("SetWindowsHookExW failed: {e}")));
                         return;
                     }
                 };
 
+                let mut msg = MSG::default();
+                // Force creation of the thread message queue before signaling readiness.
+                let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
                 let _ = ready_tx.send(Ok(tid));
 
-                let mut msg = MSG::default();
                 while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
```

### [MAJOR] [OPEN] F05-04 Argument injection into `explorer.exe` in `LaunchMode::RevealInExplorer` (`lens: F1b`)
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:131-134`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: An IPC caller passes a `target` containing double quotes (e.g. `C:\foo" /root,"\\attacker\share`) with `LaunchMode::RevealInExplorer` -> `format!("/select,\"{target}\"")` breaks out of the quoted argument and injects arbitrary `explorer.exe` flags/targets.
- **Why**: `target` is interpolated directly into a command-line string without rejecting `"` or interior NUL (`\0`) characters (which are illegal in Windows filesystem paths anyway).
- **Fix**:
```diff
             LaunchMode::RevealInExplorer => {
+                if target.contains('"') || target.contains('\0') {
+                    return Err("Invalid characters in target path".to_string());
+                }
                 let args = format!("/select,\"{target}\"");
                 shell_execute("open", "explorer.exe", Some(&args), SW_SHOWNORMAL)
             }
```

### [MINOR] [OPEN] F05-05 `GetMessageW` error return (`-1`) causes infinite 100%-CPU spin loop
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:173-176`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: `GetMessageW` returns `-1` (error) -> `BOOL(-1).as_bool()` evaluates to `true` (`!= 0`), causing the message loop to spin indefinitely without exiting or unhooking.
- **Why**: `GetMessageW` has a tri-state return value (`> 0` message, `0` `WM_QUIT`, `-1` error); checking `.as_bool()` treats `-1` as a valid message.
- **Fix**:
```diff
-                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
+                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                     let _ = TranslateMessage(&msg);
                     DispatchMessageW(&msg);
                 }
```

### [MINOR] [OPEN] F05-06 `focus_window` attaches to target thread instead of foreground thread and ignores `AttachThreadInput` result
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/switcher.rs:153-163`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: `focus_window(hwnd)` is called after the launcher window has already hidden (so another window holds foreground) -> attaching `cur_thread` to `target_thread` does not grant foreground-lock permission from the current foreground thread, causing `SetForegroundWindow(hwnd)` to fail and return `Err`.
- **Why**: Windows requires the calling thread to be attached to the *current foreground* thread (as `window.rs` does in `show_and_focus_launcher` and `restore_foreground_window`), not the destination window's thread, to bypass the foreground lock. Additionally, `AttachThreadInput`'s boolean result should be checked before detaching.
- **Fix**:
```diff
             let cur_thread = GetCurrentThreadId();
-            let target_thread = GetWindowThreadProcessId(hwnd, None);
-            let attach = target_thread != 0 && target_thread != cur_thread;
-            if attach {
-                let _ = AttachThreadInput(cur_thread, target_thread, true);
-            }
+            let fg = GetForegroundWindow();
+            let fg_thread = if !fg.0.is_null() {
+                GetWindowThreadProcessId(fg, None)
+            } else {
+                0
+            };
+            let attached = fg_thread != 0
+                && fg_thread != cur_thread
+                && AttachThreadInput(cur_thread, fg_thread, true).as_bool();
             let _ = BringWindowToTop(hwnd);
             let ok = SetForegroundWindow(hwnd).as_bool();
-            if attach {
-                let _ = AttachThreadInput(cur_thread, target_thread, false);
+            if attached {
+                let _ = AttachThreadInput(cur_thread, fg_thread, false);
             }
```
