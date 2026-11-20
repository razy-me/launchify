# Review Findings — F02: Backend Core, IPC Commands, Config & Storage

### [CRITICAL] [OPEN] F02-01 UTF-8 byte-index slicing panic in `query` on non-ASCII input
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:63-66`
- **Source**: `r-review` (2026-11-20)
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

### [CRITICAL] [OPEN] F02-02 Non-atomic `write_atomic` on Windows deletes target before rename (data loss) (lens: F1b)
- **Status**: OPEN
- **Where**: `src-tauri/src/storage.rs:31-41`
- **Source**: `r-review` (2026-11-20)
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

### [MAJOR] [OPEN] F02-03 Custom workflows ignore `args`/`run_in_terminal` and web shortcuts launch literal `{query}`
- **Status**: OPEN
- **Where**: `src-tauri/src/catalog.rs:102-130`, `src-tauri/src/commands.rs:314-323`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User configures a `WorkflowConfig` with `args: ["--port", "3000"]` and `run_in_terminal: true`, or a `WebShortcutConfig` with `url_template: "https://google.com/search?q={query}"` -> executing the item ignores `args`/`run_in_terminal` and opens the literal URL containing `{query}` instead of substituting the user's query.
- **Why**: `config_items()` drops `wf.args` and `wf.run_in_terminal` when building `IndexItem`, and `commands::execute` routes both `ItemKind::Workflow` and `ItemKind::WebShortcut` into the generic `_ => execute_target(&item.target, mode)` fallback without resolving workflow config or substituting `{query}`.
- **Fix**:
```diff
+                    ItemKind::Workflow => {
+                        let cfg = state.config.load();
+                        let wf_key = item.id.strip_prefix("workflow:");
+                        if let Some(wf) = wf_key.and_then(|k| cfg.custom_workflows.iter().find(|w| w.keyword == k)) {
+                            let full_cmd = if wf.args.is_empty() {
+                                wf.command.clone()
+                            } else {
+                                format!("{} {}", wf.command, wf.args.join(" "))
+                            };
+                            let mode = if wf.run_in_terminal || secondary {
+                                LaunchMode::ShellTerminal
+                            } else {
+                                LaunchMode::ShellHidden
+                            };
+                            win32::executor::execute_target(&full_cmd, mode)?;
+                        } else {
+                            win32::executor::execute_target(&item.target, LaunchMode::Normal)?;
+                        }
+                        ExecuteOutcome::Launched
+                    }
+                    ItemKind::WebShortcut => {
+                        let raw_q = state.last_query.lock().map(|q| q.clone()).unwrap_or_default();
+                        let kw = item.id.strip_prefix("web:").unwrap_or("");
+                        let arg = raw_q.trim().strip_prefix(kw).unwrap_or("").trim();
+                        let encoded = percent_encoding::utf8_percent_encode(arg, percent_encoding::NON_ALPHANUMERIC).to_string();
+                        let url = item.target.replace("{query}", &encoded);
+                        win32::executor::execute_target(&url, LaunchMode::Normal)?;
+                        ExecuteOutcome::Launched
+                    }
```

### [MAJOR] [OPEN] F02-04 `pick_convert_file` corrupts non-ASCII file paths from PowerShell stdout
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:431-451`
- **Source**: `r-review` (2026-11-20)
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

### [MAJOR] [OPEN] F02-05 Unbounded `bincode::deserialize_from` can OOM-abort startup on corrupt `index.bin` (lens: F1b)
- **Status**: OPEN
- **Where**: `src-tauri/src/storage.rs:57`
- **Source**: `r-review` (2026-11-20)
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

### [MAJOR] [OPEN] F02-06 Tray "Re-index" spawns unbounded concurrent threads that can race `update_index` (lens: F1b)
- **Status**: OPEN
- **Where**: `src-tauri/src/tray.rs:43-51`
- **Source**: `r-review` (2026-11-20)
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

### [MINOR] [OPEN] F02-07 `execute` records and persists frecency before validating `item_id` or executing the action
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:203-213`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User copies an inline calc result (`item_id = "__inline__"`), invokes `copy:...`, or triggers an invalid/failed `execute` call -> `record_selection` still mutates `frecency` and writes `frecency.json` to disk with synthetic/non-existent IDs.
- **Why**: Frecency recording runs unconditionally at the top of `execute` before action dispatch and only filters out `win:` and `shell:`.
- **Fix**:
```diff
-    if !item_id.is_empty() && !item_id.starts_with("win:") && !item_id.starts_with("shell:") {
-        let prefix = state...
-        ...
-    }
     let outcome = ...;
+    if !item_id.is_empty()
+        && item_id != "__inline__"
+        && !item_id.starts_with("win:")
+        && !item_id.starts_with("shell:")
+        && !item_id.starts_with("copy:")
+    {
+        let prefix = state.last_query.lock().map(|q| normalize_prefix(&q)).unwrap_or_default();
+        if let Ok(mut frecency) = state.frecency.lock() {
+            record_selection(&mut frecency, &prefix, &item_id, now_unix());
+            if let Err(e) = storage::save_frecency_atomic(&state.app_dir.join("frecency.json"), &frecency) {
+                tracing::warn!("Failed to persist frecency: {e}");
+            }
+        }
+    }
     Ok(outcome)
```

### [MINOR] [OPEN] F02-08 Unchecked `w * h * 4` buffer sizing before `unsafe` `GetDIBits` (lens: F1b)
- **Status**: OPEN
- **Where**: `src-tauri/src/icons.rs:130-157`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: security
- **Scenario**: A malformed executable or `.ico` file returns a bitmap with huge `bmWidth`/`bmHeight` -> `(w * h * 4) as usize` overflows `u32` in release builds, allocating an undersized `Vec`, and `unsafe { GetDIBits(...) }` writes past the heap buffer.
- **Why**: `w * h * 4` is computed in `u32` without `checked_mul` or a reasonable icon dimension cap (e.g. `<= 1024`).
- **Fix**:
```diff
-        if got_obj && bmp.bmWidth > 0 && bmp.bmHeight > 0 {
+        if got_obj && bmp.bmWidth > 0 && bmp.bmHeight > 0 && bmp.bmWidth <= 1024 && bmp.bmHeight <= 1024 {
             let w = bmp.bmWidth as u32;
             let h = bmp.bmHeight as u32;
+            let buf_len = (w as usize).checked_mul(h as usize).and_then(|v| v.checked_mul(4));
```

### [ENHANCEMENT] [OPEN] F02-09 Drop `frecency` mutex before icon disk I/O and reuse `base64` crate
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:39-52`, `src-tauri/src/commands.rs:161-188`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: On every keystroke in `PrefixMode::Global`, `query` holds `state.frecency.lock()` while synchronously reading up to 8 PNGs from disk and base64-encoding them with a hand-rolled encoder (even though `base64 = "0.22"` is already in `Cargo.toml`).
- **Why**: Scoping the `frecency` lock to `search_engine.search(...)` avoids blocking concurrent `execute` calls during disk I/O, and using `base64::engine::general_purpose::STANDARD.encode` removes duplicate encoding code.
- **Fix**:
```diff
-            let frecency = state
-                .frecency
-                .lock()
-                .map_err(|_| "frecency lock poisoned".to_string())?;
-            let mut results = state.search_engine.search(
-                rest,
-                &frecency,
-                cfg.max_results,
-                cfg.prefix_memory_threshold,
-                now_unix(),
-            );
+            let mut results = {
+                let frecency = state
+                    .frecency
+                    .lock()
+                    .map_err(|_| "frecency lock poisoned".to_string())?;
+                state.search_engine.search(
+                    rest,
+                    &frecency,
+                    cfg.max_results,
+                    cfg.prefix_memory_threshold,
+                    now_unix(),
+                )
+            };
```
