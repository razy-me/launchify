## Review: Full Codebase Audit (Launchify)
**Date**: 2026-11-20 | **Tier**: 2 (Medium)
**Files**: 40 | **Agents Used**: 6 (5 fraction reviewers + 1 F3 verifier)

| CRITICAL | MAJOR | MINOR | ENHANCEMENT |
|---|---|---|---|
| 2 | 16 | 16 | 1 |

### Fix First (top items)
1. [CRITICAL] [OPEN] F02-01 UTF-8 byte-index slicing panic in `query` on non-ASCII input -- `src-tauri/src/commands.rs:63-66`
2. [CRITICAL] [OPEN] F02-02 Non-atomic `write_atomic` on Windows deletes target before rename (data loss) -- `src-tauri/src/storage.rs:31-41`
3. [MAJOR] [OPEN] F01-01 Stale `index` closure on reused DOM rows executes wrong item on click -- `src/renderer.ts:393-397`
4. [MAJOR] [OPEN] F01-02 Double-execution and state mutation via keyboard during close/launch-fade window -- `src/keyboard.ts:24-33`
5. [MAJOR] [OPEN] F04-01 Unbounded recursion in math parser allows stack overflow crash -- `src-tauri/src/inline_tools/math.rs:187-242`
6. [MAJOR] [OPEN] F05-04 Argument injection into `explorer.exe` in `LaunchMode::RevealInExplorer` -- `src-tauri/src/win32/executor.rs:131-134`

### Systemic Patterns
- **Unsynchronized rebuilds / shared state races**: `tray.rs` spawns raw threads bypassing `watch_tx` (`F02-06`) and `IndexManager::rebuild_now` lacks a single-flight mutex (`F03-01`) -> serialize rebuilds through `watch_tx` and guard `rebuild_now` with a mutex.
- **Unbounded input/subprocess handling**: `bincode::deserialize_from` without limit (`F02-05`), unbounded recursion in `math.rs` (`F04-01`), and converter subprocesses without timeouts or path hardening (`F04-03`) -> enforce explicit bounds, timeouts, and safe path/argument handling at trust boundaries.

### Findings

### [CRITICAL] [FIXED] F02-01 UTF-8 byte-index slicing panic in `query` on non-ASCII input
- **Status**: FIXED
- **Where**: `src-tauri/src/commands.rs:63-66`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User types a query containing multi-byte UTF-8 characters into the launcher search bar (e.g. `"aä"`, `"🚀"`, or `"äää"`) -> `trimmed_raw[..2]` or `trimmed_raw[..5]` slices mid-character and panics the `query` IPC command handler.
- **Why**: `trimmed_raw.len() >= 5` and `trimmed_raw[..5]` (and `>= 2` / `[..2]`) index by byte offset without checking UTF-8 char boundaries, unlike `parse_prefix_mode` at line 30 which safely uses `.get(..8)`.
- **Fix**:
```diff
-    let find_rest = if trimmed_raw.len() >= 5 && trimmed_raw[..5].eq_ignore_ascii_case("find:") {
-        Some(trimmed_raw[5..].trim_start())
-    } else if trimmed_raw.len() >= 2 && trimmed_raw[..2].eq_ignore_ascii_case("f:") {
-        Some(trimmed_raw[2..].trim_start())
-    } else {
-        None
-    };
+    let find_rest = if trimmed_raw.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("find:")) {
+        Some(trimmed_raw[5..].trim_start())
+    } else if trimmed_raw.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("f:")) {
+        Some(trimmed_raw[2..].trim_start())
+    } else {
+        None
+    };
```

### [CRITICAL] [FIXED] F02-02 Non-atomic `write_atomic` on Windows deletes target before rename (data loss)
- **Status**: FIXED
- **Where**: `src-tauri/src/storage.rs:31-41`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: `save_config_atomic`, `save_frecency_atomic`, or `save_index_atomic` runs on Windows while an antivirus/indexer briefly locks the target or the process crashes between `remove_file` and `rename` -> `fs::remove_file(path)` deletes the existing file, `fs::rename` fails, the error handler deletes `tmp_path`, and both old and new data are permanently lost.
- **Why**: Deleting the destination before renaming breaks atomicity on Windows. `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` (already available via `Win32_Storage_FileSystem` in `Cargo.toml`) replaces the destination atomically without a deletion window.
- **Fix**:
```diff
-    #[cfg(windows)]
-    let _ = fs::remove_file(path);
-
-    fs::rename(&tmp_path, path).map_err(|e| {
-        let _ = fs::remove_file(&tmp_path);
-        format!(
-            "failed to rename {} -> {}: {e}",
-            tmp_path.display(),
-            path.display()
-        )
-    })
+    #[cfg(windows)]
+    {
+        use std::os::windows::ffi::OsStrExt;
+        use windows::core::PCWSTR;
+        use windows::Win32::Storage::FileSystem::{
+            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
+        };
+        let src: Vec<u16> = tmp_path.as_os_str().encode_wide().chain(Some(0)).collect();
+        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
+        let res = unsafe {
+            MoveFileExW(
+                PCWSTR(src.as_ptr()),
+                PCWSTR(dst.as_ptr()),
+                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
+            )
+        };
+        res.map_err(|e| {
+            let _ = fs::remove_file(&tmp_path);
+            format!("failed to replace {} -> {}: {e}", tmp_path.display(), path.display())
+        })
+    }
+    #[cfg(not(windows))]
+    fs::rename(&tmp_path, path).map_err(|e| {
+        let _ = fs::remove_file(&tmp_path);
+        format!("failed to rename {} -> {}: {e}", tmp_path.display(), path.display())
+    })
```

### [MAJOR] [FIXED] F01-01 Stale `index` closure on reused DOM rows executes wrong item on click
- **Status**: FIXED
- **Where**: `src/renderer.ts:393-397` (`src/renderer.ts:147-153`)
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User types `"ch"` so item X renders at index `3` (`createResultRow` binds `mousedown` closing over `index = 3`). User types `"r"` (`"chr"`), re-ranking item X to index `0`. `renderState` reuses the existing DOM node and updates `existing.dataset.index = "0"`, then user clicks item X at index `0` -> `onSelectAndExecute(3, secondary)` fires, executing `state.results[3]` (a different item) or nothing if fewer than 4 results exist.
- **Why**: `createResultRow` captures the initial `index` parameter in its `mousedown` listener closure, whereas FLIP row reuse in `renderState` only updates `existing.dataset.index` and never rebinds the listener.
- **Fix**:
```diff
   row.addEventListener("mousedown", (e) => {
     e.preventDefault();
+    const currentIndex = Number(row.dataset.index);
     const secondary = e.shiftKey || e.ctrlKey;
-    onSelectAndExecute(index, secondary);
+    onSelectAndExecute(currentIndex, secondary);
   });
```

### [MAJOR] [FIXED] F01-02 Double-execution and state mutation via keyboard during close/launch-fade window
- **Status**: FIXED
- **Where**: `src/keyboard.ts:24-33` (`src/main.ts:55-88`, `src/main.ts:100-140`)
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User selects an app or shell command and presses `Enter` twice quickly (or holds `Enter` past key-repeat), or presses `Enter` in the action menu and taps `Enter` again during the 650ms `is-launch-fading` animation -> `state.visible` is `false`, but `onKeyDown` still dispatches `onExecute` and `executeSelected` does not check `state.visible`, causing the item/command to be executed multiple times.
- **Why**: `#launcher.is-launch-fading` only disables pointer events (`pointer-events: none`), while the window remains focused for 650ms and neither `onKeyDown` nor `executeSelected`/`executeWithAction` guards on `state.visible`.
- **Fix**:
```diff
   const onKeyDown = (e: KeyboardEvent): void => {
+    if (!state.visible) {
+      return;
+    }
```

### [MAJOR] [FIXED] F01-03 Inactivity timer auto-hides launcher during active Convert mode
- **Status**: FIXED
- **Where**: `src/main.ts:180-189`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User enters `Convert` mode and either spends >15 seconds choosing a file in the native OS file picker (`pickConvertFile`) or runs an `ffmpeg`/`pandoc` conversion that takes >15 seconds without pressing keys in the WebView -> `inactivityTimer` fires and calls `hide()`, hiding the launcher and clearing Convert state via `onLauncherHidden`.
- **Why**: `onWindowBlur` in `src/keyboard.ts:142` explicitly exempts `state.mode === "Convert"` so file picking, drag-and-drop, and conversions are not interrupted, but `resetInactivityTimer` in `src/main.ts` has no such exemption.
- **Fix**:
```diff
   const resetInactivityTimer = (): void => {
     clearInactivityTimer();
-    if (!state.visible) {
+    if (!state.visible || state.mode === "Convert") {
       return;
     }
     inactivityTimer = window.setTimeout(() => {
       inactivityTimer = null;
+      if (state.mode === "Convert") {
+        return;
+      }
       hide();
     }, INACTIVITY_TIMEOUT_MS);
   };
```

### [MAJOR] [FIXED] F02-03 Custom workflows ignore `args`/`run_in_terminal` and web shortcuts launch literal `{query}`
- **Status**: FIXED
- **Where**: `src-tauri/src/catalog.rs:102-130`, `src-tauri/src/commands.rs:314-323`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User configures a `WorkflowConfig` with `args: ["--port", "3000"]` and `run_in_terminal: true`, or a `WebShortcutConfig` with `url_template: "https://google.com/search?q={query}"` -> executing the item ignores `args`/`run_in_terminal` and opens the literal URL containing `{query}` instead of substituting the user's query.
- **Why**: `config_items()` drops `wf.args` and `wf.run_in_terminal` when building `IndexItem`, and `commands::execute` routes both `ItemKind::Workflow` and `ItemKind::WebShortcut` into the generic `_ => execute_target(&item.target, mode)` fallback without resolving workflow config or substituting `{query}`.
- **Fix**:
```diff
+ItemKind::Workflow => {
+    // Resolve WorkflowConfig by keyword, properly quote args containing spaces,
+    // and respect wf.run_in_terminal || secondary for LaunchMode::ShellTerminal vs ShellHidden.
+}
+ItemKind::WebShortcut => {
+    // Strip keyword prefix case-insensitively from last_query, percent-encode the argument,
+    // and replace "{query}" in item.target before calling execute_target.
+}
```

### [MAJOR] [FIXED] F02-04 `pick_convert_file` corrupts non-ASCII file paths from PowerShell stdout
- **Status**: FIXED
- **Where**: `src-tauri/src/commands.rs:431-451`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User picks a file containing non-ASCII characters (e.g. `C:\Users\Razy\Prüfung.png`) via `pick_convert_file` -> Windows PowerShell 5.1 emits the path in the OEM codepage (e.g. CP850), `String::from_utf8_lossy` replaces non-ASCII bytes with `U+FFFD` (`Prfung.png`), and subsequent `convert_file_cmd` fails with file-not-found.
- **Why**: `powershell.exe` defaults `[Console]::OutputEncoding` to the console OEM codepage rather than UTF-8.
- **Fix**:
```diff
                 .args([
                     "-NoProfile",
                     "-Sta",
                     "-Command",
-                    "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; if ($f.ShowDialog() -eq 'OK') { Write-Output $f.FileName }",
+                    "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; if ($f.ShowDialog() -eq 'OK') { Write-Output $f.FileName }",
                 ])
```

### [MAJOR] [FIXED] F02-05 Unbounded `bincode::deserialize_from` can OOM-abort startup on corrupt `index.bin`
- **Status**: FIXED
- **Where**: `src-tauri/src/storage.rs:57`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: A truncated/corrupted or tampered `index.bin` contains a large length prefix for `items` or a `String` field -> `bincode::deserialize_from` attempts to allocate gigabytes of memory and aborts the process on startup instead of falling back to `IndexSnapshot::default()`.
- **Why**: `bincode::deserialize_from` uses an unlimited allocation budget by default; `bincode::options().with_limit(...)` is required to cap allocations on untrusted/corrupt files.
- **Fix**:
```diff
+    use bincode::Options;
+    const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
-    match bincode::deserialize_from::<_, IndexSnapshot>(reader) {
+    match bincode::DefaultOptions::new()
+        .with_fixint_encoding()
+        .allow_trailing_bytes()
+        .with_limit(MAX_INDEX_BYTES)
+        .deserialize_from::<_, IndexSnapshot>(reader)
+    {
```

### [MAJOR] [FIXED] F02-06 Tray "Re-index" spawns unbounded concurrent threads that can race `update_index`
- **Status**: FIXED
- **Where**: `src-tauri/src/tray.rs:43-51`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: User clicks "Re-index" multiple times in the tray menu (or while the watcher/initial index thread is running) -> each click spawns a new OS thread running `rebuild_now()` concurrently; an older rebuild finishing after a newer one overwrites `search_engine` with a stale snapshot and contends on `index.bin`.
- **Why**: `AppState` already has a serialized `watch_tx` channel wired to the single watcher thread that runs `rebuild_now()` and updates `search_engine`, but `tray.rs` bypasses it and spawns raw threads.
- **Fix**:
```diff
             "reindex" => {
                 if let Some(state) = app.try_state::<AppState>() {
-                    let idx_mgr = Arc::clone(&state.index_manager);
-                    let engine = Arc::clone(&state.search_engine);
-                    std::thread::spawn(move || match idx_mgr.rebuild_now() {
-                        Ok(snap) => engine.update_index(snap),
-                        Err(e) => tracing::warn!("Manual re-index failed: {e}"),
-                    });
+                    let _ = state.watch_tx.try_send(());
                 }
             }
```

### [MAJOR] [FIXED] F03-01 Unsynchronized concurrent `IndexManager::rebuild_now` races on snapshot and `index.bin`
- **Status**: FIXED
- **Where**: `src-tauri/src/indexer/mod.rs:46-94`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: A background watcher flush (`watcher.rs:96`) and a manual/IPC-triggered rebuild invoke `IndexManager::rebuild_now(&self)` concurrently on cloned `IndexManager` handles.
- **Why**: `IndexManager` has no mutex or single-flight guard around `rebuild_now`. Two concurrent rebuilds scan the filesystem and extract icons in parallel, then race on `self.snapshot.store(...)` and `save_index_atomic(&self.app_dir.join("index.bin"), &snap)`.
- **Fix**:
```diff
 pub struct IndexManager {
     snapshot: Arc<ArcSwap<IndexSnapshot>>,
     config: Arc<ArcSwap<LaunchifyConfig>>,
     app_dir: PathBuf,
+    rebuild_lock: Arc<std::sync::Mutex<()>>,
 }
 // At start of rebuild_now(&self):
+let _guard = self.rebuild_lock.lock().map_err(|_| "IndexManager rebuild lock poisoned".to_string())?;
```

### [MAJOR] [FIXED] F03-02 Missing length-difference guard in `typo_fuzzy_score` causes massive heap allocations and long-query freeze
- **Status**: FIXED
- **Where**: `src-tauri/src/search/ranking.rs:109-112`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: User types a 3+ character query (or pastes a long string, e.g. 500+ chars) into the launcher; `SearchEngine::search` or `search_recent_files` evaluates `typo_fuzzy_score(q, title)` on every non-matching item in the index.
- **Why**: Line 109 calls `damerau_levenshtein(q, title)` unconditionally without checking `title.chars().count().abs_diff(q_len) <= max_edits`, causing `n + 1` heap allocations per non-matching item per keystroke.
- **Fix**:
```diff
-let d_full = damerau_levenshtein(q, title);
-if d_full <= max_edits {
-    best_dist = Some(d_full);
-}
+let title_len = title.chars().count();
+if title_len.abs_diff(q_len) <= max_edits {
+    let d_full = damerau_levenshtein(q, title);
+    if d_full <= max_edits {
+        best_dist = Some(d_full);
+    }
+}
```

### [MAJOR] [FIXED] F04-01 Unbounded recursion in math parser allows stack overflow crash
- **Status**: FIXED
- **Where**: `src-tauri/src/inline_tools/math.rs:187-242`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: Pasting a deeply nested expression such as `"(".repeat(5000)` or `"-".repeat(5000) + "1"` or `"2^".repeat(5000) + "1"` into the launcher triggers `eval_math`, which recurses unconditionally in `parse_primary`, `parse_unary`, and `parse_power` until the thread overflows its stack and aborts the backend process.
- **Why**: The recursive-descent parser tracks token position (`pos`) but has no recursion depth counter or maximum nesting limit.
- **Fix**:
```diff
 struct Parser {
     tokens: Vec<Token>,
     pos: usize,
+    depth: usize,
 }
+const MAX_RECURSION_DEPTH: usize = 128;
```
Wrap recursive calls in `parse_power`, `parse_unary`, and `parse_primary` with a depth-guarded helper returning `Err("Expression nesting too deep")`.

### [MAJOR] [FIXED] F04-02 Unary minus binds tighter than exponentiation (`-2^2` evaluates to `4`)
- **Status**: FIXED
- **Where**: `src-tauri/src/inline_tools/math.rs:187-210`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Evaluating `-2^2` returns `4` instead of `-4`, and `-2^2 + 4` returns `8` instead of `0`.
- **Why**: `parse_power` calls `self.parse_unary()` for the base before checking for `Token::Caret`. In standard arithmetic precedence, exponentiation binds tighter than unary negation on the base (`-2^2 == -(2^2)`), while unary negation is still allowed in the exponent (`2^-3 == 2^(-3)`).
- **Fix**:
```diff
-// parse_mul_div -> parse_power -> parse_unary -> parse_primary
+// parse_mul_div -> parse_unary -> parse_power -> parse_primary (with exponent calling parse_unary)
```

### [MAJOR] [FIXED] F04-03 External converter subprocesses lack timeouts and argument-injection guards
- **Status**: FIXED
- **Where**: `src-tauri/src/converter/mod.rs:436-456`, `src-tauri/src/converter/mod.rs:571-652`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: (1) Converting a malformed or very large media/PDF/document file via `ffmpeg`, `pandoc`, `pdftotext`, `pdftoppm`, or `magick` calls `.output()`, blocking the worker thread indefinitely with no timeout and buffering unbounded stderr. (2) A relative path starting with `-` is parsed as a CLI flag rather than a file path.
- **Why**: Subprocesses are invoked directly with `Command::new(...).arg(source).output()` without canonicalizing/prefixing paths and without a timeout.
- **Fix**:
```diff
+fn safe_cli_path(path: &Path) -> PathBuf {
+    fs::canonicalize(path).unwrap_or_else(|_| {
+        if path.is_relative() { Path::new(".").join(path) } else { path.to_path_buf() }
+    })
+}
```
Pass `safe_cli_path(...)` to external tools and spawn with a timeout (killing the child on expiry).

### [MAJOR] [OPEN] F05-01 `system_power_action("sleep")` hibernates instead of sleeping due to `rundll32` signature mismatch
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:186-191`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User invokes the `sleep` power action on a Windows machine with hibernation enabled -> `rundll32.exe` invokes `powrprof.dll!SetSuspendState` using the `rundll32` callback ABI (`HWND, HINSTANCE, LPSTR, int`), passing a non-zero `HWND` as `bHibernate`, which hibernates the machine instead of sleeping.
- **Why**: `SetSuspendState` is not a `rundll32`-compatible entry point.
- **Fix**: Add `"Win32_System_Power"` to `windows` features in `src-tauri/Cargo.toml` and call `unsafe { windows::Win32::System::Power::SetSuspendState(false, false, false) }` directly.

### [MAJOR] [OPEN] F05-02 `LAlt+RShift` keyboard layout switch falsely triggers the launcher
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:19-60`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User presses `Left Alt` + `Right Shift` (standard Windows keyboard layout switch) -> `LAlt` down sets `trigger_down = true`, `RShift` down hits the same branch and leaves `other_key_pressed = false`, and releasing either key returns `HookDecision::Trigger`, unexpectedly opening the launcher.
- **Why**: `WinKeyStateMachine` tracks `trigger_down: bool` without recording which trigger key initiated the press.
- **Fix**: Replace `trigger_down: bool` with `active_trigger: Option<u16>` and set `other_key_pressed = true` when a different trigger key is pressed while one is active.

### [MAJOR] [OPEN] F05-03 Potential deadlock in `WinKeyHookHandle::drop` due to `PostThreadMessageW` startup race
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:162-176`, `src-tauri/src/win32/hook.rs:195-205`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: `WinKeyHookHandle::start` returns and the handle is dropped before the newly spawned hook thread executes `GetMessageW` -> `PostThreadMessageW(thread_id, WM_QUIT, ...)` fails with `ERROR_INVALID_THREAD_ID`, and `stop_hook_thread` deadlocks in `handle.join()`.
- **Why**: In Win32, a thread's message queue is created lazily on its first message-queue call. Signaling `ready_tx.send(Ok(tid))` before calling `PeekMessageW` races with `PostThreadMessageW`.
- **Fix**:
```diff
+let mut msg = MSG::default();
+let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
 let _ = ready_tx.send(Ok(tid));
```

### [MAJOR] [OPEN] F05-04 Argument injection into `explorer.exe` in `LaunchMode::RevealInExplorer`
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/executor.rs:131-134`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: An IPC caller passes a `target` containing double quotes (e.g. `C:\foo" /root,"\\attacker\share`) with `LaunchMode::RevealInExplorer` -> `format!("/select,\"{target}\"")` breaks out of the quoted argument and injects arbitrary `explorer.exe` flags/targets.
- **Why**: `target` is interpolated directly into a command-line string without rejecting `"` or interior NUL (`\0`) characters.
- **Fix**:
```diff
 LaunchMode::RevealInExplorer => {
+    if target.contains('"') || target.contains('\0') {
+        return Err("Invalid characters in target path".to_string());
+    }
     let args = format!("/select,\"{target}\"");
     shell_execute("open", "explorer.exe", Some(&args), SW_SHOWNORMAL)
 }
```

### [MINOR] [FIXED] F01-04 Entering Convert mode leaves `.action-menu` orphaned and window stuck at 896px width
- **Status**: FIXED
- **Where**: `src/renderer.ts:79-102`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User opens `.action-menu` (`ArrowRight`, widening window to `896px`) then types `convert` -> `renderState` takes the early `Convert` return, leaving `.action-menu` in `document.body` and `lastWidthPx` at `896`.
- **Why**: The `Convert` branch returns early before `.action-menu` cleanup and calls `resizeLauncher(targetHeight)` without resetting width to `680`.
- **Fix**: Remove `.action-menu`, clear `resultsList.style.minHeight`, and reset `lastWidthPx = 680` / `resizeLauncher(targetHeight, 680)` in the `Convert` branch.

### [MINOR] [FIXED] F01-05 `onAutocomplete` race condition overwrites newer input or runs after hide
- **Status**: FIXED
- **Where**: `src/main.ts:228-241`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User presses `Tab` and immediately types another character or presses `Escape` before `autocompleteQuery` resolves -> stale completion overwrites `refs.input.value` and triggers `runQuery`.
- **Why**: `onAutocomplete` does not check `state.visible`, `refs.input.value`, or `state.querySeq` when the promise resolves.
- **Fix**: Capture `rawAtRequest` and `seqAtRequest` before calling `autocompleteQuery` and bail if `!state.visible || refs.input.value !== rawAtRequest || state.querySeq !== seqAtRequest`.

### [MINOR] [FIXED] F01-06 "Reveal in Explorer" button clipped off-screen by `overflow: hidden` on `.convert-status`
- **Status**: FIXED
- **Where**: `src/styles.css:416-426` (`src/renderer.ts:528-541`)
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Converting a file with a path >55 chars pushes `.convert-reveal-btn` past the right edge of `.convert-status`, where `overflow: hidden` clips it out of view.
- **Why**: `.convert-status` applies `white-space: nowrap; overflow: hidden;` to the whole container instead of only the text span.
- **Fix**: Make `.convert-status` a flex row, wrap the text in `.convert-status-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }`, and set `.convert-reveal-btn { flex-shrink: 0; }`.

### [MINOR] [FIXED] F01-07 Reused result rows retain `is-launched`/`is-green` classes when launcher reopens during fade
- **Status**: FIXED
- **Where**: `src/renderer.ts:147-153`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: Re-triggering the launcher within 650ms of launching an item reuses the DOM row without stripping `is-launched` and `is-green`.
- **Why**: `renderState` only removes `is-entering` on reused rows.
- **Fix**: `existing.classList.remove("is-entering", "is-launched", "is-green");`

### [MINOR] [FIXED] F02-07 `execute` records and persists frecency before validating `item_id` or executing the action
- **Status**: FIXED
- **Where**: `src-tauri/src/commands.rs:203-213`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Copying an inline calc result (`__inline__`) or `copy:` item, or a failed `execute`, still mutates and persists `frecency.json`.
- **Why**: Frecency recording runs unconditionally at the top of `execute` before dispatch.
- **Fix**: Move frecency recording after successful execution and exclude `__inline__` and `copy:` prefixes.

### [MINOR] [FIXED] F02-08 Unchecked `w * h * 4` buffer sizing before `unsafe` `GetDIBits`
- **Status**: FIXED
- **Where**: `src-tauri/src/icons.rs:130-157`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: security
- **Scenario**: A malformed executable/`.ico` with huge `bmWidth`/`bmHeight` overflows `(w * h * 4) as usize` in `u32`, causing `GetDIBits` to write past the `Vec` buffer.
- **Why**: No dimension cap or `checked_mul` before allocating the pixel buffer.
- **Fix**: Cap `bmWidth <= 1024 && bmHeight <= 1024` and use `checked_mul` in `usize`.

### [MINOR] [FIXED] F03-03 Drive-only input `/C:` or `C:` resolves to process CWD instead of drive root `C:\`
- **Status**: FIXED
- **Where**: `src-tauri/src/path_nav.rs:18-19, 33-35, 48-50`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Typing `/C:` or `C:` lists the process's current working directory on `C:` instead of `C:\`.
- **Why**: On Windows, `"C:"` without a trailing backslash is drive-relative.
- **Fix**: Append `'\\'` when the normalized path is a 2-byte `<letter>:` string.

### [MINOR] [FIXED] F03-04 Indexer watcher thread wakes every 250ms while idle, defeating EcoQoS power savings
- **Status**: FIXED
- **Where**: `src-tauri/src/indexer/watcher.rs:79-98`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: Idle background app wakes the watcher thread 4 times/sec indefinitely.
- **Why**: `select!` unconditionally includes `default(Duration::from_millis(250))` even when `!debouncer.dirty`.
- **Fix**: Block without `default(...)` timeout when `!debouncer.dirty`.

### [MINOR] [FIXED] F04-04 TOCTOU race in `build_output_path` allows concurrent conversions to clobber files
- **Status**: FIXED
- **Where**: `src-tauri/src/converter/mod.rs:103-125`, `src-tauri/src/converter/mod.rs:679`
- **Source**: `r-review` (Tier 2) (2026-11-20) — *Downgraded from MAJOR by F3 Verifier*
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: Triggering two conversions of the same source file to the same target format concurrently causes both calls to observe `!primary.exists()` and select the same `out_path`.
- **Why**: Uses non-atomic `Path::exists()` instead of `OpenOptions::new().write(true).create_new(true)`.
- **Fix**: Atomically reserve the output path with `create_new(true)` (removing placeholder on error, and keeping compatibility with `pdftoppm -singlefile`).

### [MINOR] [FIXED] F04-05 `format_number` rounds tiny values to `0` and expands huge floats into 300+ digit strings
- **Status**: FIXED
- **Where**: `src-tauri/src/inline_tools/math.rs:297-309`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: `1e-12` displays `"0"` and `2^1000` displays a 302-char string.
- **Why**: Always uses fixed-point `format!("{val:.10}")`.
- **Fix**: Use scientific notation (`format!("{val:.6e}")`) when `abs < 1e-9 || abs >= 1e15`.

### [MINOR] [FIXED] F04-06 Per-byte `String` allocation in `txt` to `hex` conversion
- **Status**: FIXED
- **Where**: `src-tauri/src/converter/mod.rs:524-530`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: Converting a large `.txt` file to `.hex` allocates one `String` per byte via `format!("{b:02x}")`.
- **Why**: `bytes.iter().map(|b| format!("{b:02x}")).collect()` does not reuse a single buffer.
- **Fix**: Pre-allocate `String::with_capacity(bytes.len() * 2)` and use `write!(&mut encoded, "{b:02x}")`.

### [MINOR] [FIXED] F04-07 Unchecked float overflow in unit and currency conversion can return `"inf"`
- **Status**: FIXED
- **Where**: `src-tauri/src/inline_tools/units.rs:128-147`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: `1e308 tb to b` or `1e308 usd to jpy` displays `"inf B"` / `"inf JPY"`.
- **Why**: `result.is_finite()` is not checked after multiplication/division.
- **Fix**: Return `None` if `!result.is_finite()`.

### [MINOR] [FIXED] F04-08 `parse_iso_utc` accepts invalid calendar dates like `2023-02-31` and silently rolls them over
- **Status**: FIXED
- **Where**: `src-tauri/src/inline_tools/utils.rs:163-168`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: `2023-02-31` produces the timestamp for `2023-03-03` instead of being rejected.
- **Why**: Only checks `(1..=31).contains(&day)` without per-month/leap-year day limits.
- **Fix**: Validate `day` against the actual max days for `(year, month)`.

### [MINOR] [DEFERRED] F04-09 `generate_uuid_v4` uses non-cryptographic `RandomState` hashing of empty input
- **Status**: DEFERRED (Requires adding `getrandom` or `uuid` crate to `src-tauri/Cargo.toml`, which is outside cluster C4 owned files)
- **Where**: `src-tauri/src/inline_tools/utils.rs:69-82`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: `uuid` command generates predictable UUIDs from `RandomState` + `SystemTime` rather than a CSPRNG.
- **Why**: `RandomState` is not cryptographically secure.
- **Fix**: Use `getrandom::fill(&mut bytes)` or `uuid::Uuid::new_v4()`.

### [MINOR] [OPEN] F05-05 `GetMessageW` error return (`-1`) causes infinite 100%-CPU spin loop
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/hook.rs:173-176`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: `GetMessageW` returning `-1` evaluates `BOOL(-1).as_bool() == true`, spinning forever.
- **Why**: `GetMessageW` returns `> 0`, `0`, or `-1`.
- **Fix**: Change loop condition to `while GetMessageW(&mut msg, None, 0, 0).0 > 0`.

### [MINOR] [OPEN] F05-06 `focus_window` attaches to target thread instead of foreground thread and ignores `AttachThreadInput` result
- **Status**: OPEN
- **Where**: `src-tauri/src/win32/switcher.rs:153-163`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: `focus_window(hwnd)` fails to bring target window to foreground after launcher hides.
- **Why**: Attaches to `target_thread` instead of `GetForegroundWindow()`'s thread, and does not check `AttachThreadInput` return value before detaching.
- **Fix**: Attach to the current foreground window's thread and detach only if `AttachThreadInput(...).as_bool()` succeeded.

### [ENHANCEMENT] [FIXED] F02-09 Drop `frecency` mutex before icon disk I/O and reuse `base64` crate
- **Status**: FIXED
- **Where**: `src-tauri/src/commands.rs:39-52`, `src-tauri/src/commands.rs:161-188`
- **Source**: `r-review` (Tier 2) (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: `query` holds `state.frecency.lock()` across synchronous PNG reads and hand-rolled base64 encoding.
- **Why**: Scoping the lock to `search_engine.search(...)` avoids blocking concurrent `execute` calls, and `base64 = "0.22"` is already in `Cargo.toml`.
- **Fix**: Scope the `frecency` lock block around `search_engine.search(...)` and use `base64::engine::general_purpose::STANDARD.encode`.

### Coverage / Not Reviewed
- Reviewed: All 40 project source files across Frontend (`src/`, `index.html`), Backend Core/IPC/Storage/Config, Search/Indexer/Navigation, Converter/Inline Tools, and Win32 Layer.
- Excluded / Not reviewed: Generated Tauri JSON schemas (`src-tauri/gen/`), lockfiles (`package-lock.json`, `Cargo.lock`), `node_modules/`, `dist/`, `target/`, and integration tests (`src-tauri/tests/integration.rs`).
