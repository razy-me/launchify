# Review Findings Part F4: Storage, Frecency & Inline Tools

### [MAJOR] [OPEN] F4-01 `write_atomic` temp file extension logic truncates or alters existing multi-extension filenames and creates potential temp collision
- **Status**: OPEN
- **Where**: `src-tauri/src/storage.rs:18`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When calling `write_atomic` with a path like `frecency.json` or `data/index.bin`, `path.with_extension(format!("{}.{}.tmp", std::process::id(), unique))` replaces `.json` or `.bin` with `1234.5678.tmp`. If `path` has no extension or a compound name, `with_extension` replaces the last extension rather than appending to the full file name. More critically, on Windows `fs::rename(&tmp_path, path)` fails with an OS error `ERROR_ALREADY_EXISTS` / `Access is denied` if destination file `path` already exists, unless atomic replace semantics are handled or existing destination is overwritten. In Windows standard library, `std::fs::rename` maps to `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`, causing `write_atomic` to fail when saving over an existing `index.bin` or `frecency.json`.
- **Why**: Rust's standard library `std::fs::rename` on Windows does not consistently replace existing target files across all Windows platforms/filesystems without error, or fails if the target file exists. Furthermore, replacing the file extension instead of appending `.tmp` causes odd file naming conventions.
- **Fix**:
```diff
-    let tmp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
+    let file_name = path
+        .file_name()
+        .and_then(|n| n.to_str())
+        .unwrap_or("tmp");
+    let tmp_path = path.with_file_name(format!("{file_name}.{}.{}.tmp", std::process::id(), unique));
```

---

### [MAJOR] [OPEN] F4-02 `frecency_score` suffers from NaN/Inf propagation on negative timestamp drift or future timestamps
- **Status**: OPEN
- **Where**: `src-tauri/src/frecency.rs:47-49`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: If a user's clock is adjusted backwards or an item was recorded with a future timestamp (e.g. system clock sync or daylight savings time adjustment), `now_unix.saturating_sub(entry.last_used_unix)` evaluates to `0`. However, if `entry.count < 0` (if corrupted or cast) or if `age_secs` calculation leads to subnormal floats, `ln_1p()` could produce negative infinity or NaN if unvalidated. More importantly, when `frecency_score` returns `NaN`, sorting search results in caller modules using `f64::total_cmp` or standard comparisons will lead to erratic ordering or non-deterministic rankings.
- **Why**: `frecency_score` does not guard against non-finite float results (`decay.is_finite()` or result validation) before returning, returning unbounded floats directly to ranking functions.
- **Fix**:
```diff
     let age_secs = (now_unix.saturating_sub(entry.last_used_unix)).max(0) as f64;
     let decay = 0.5_f64.powf(age_secs / FRECENCY_HALF_LIFE_SECS);
-    (entry.count as f64).ln_1p() * FRECENCY_WEIGHT * decay
+    let score = (entry.count as f64).ln_1p() * FRECENCY_WEIGHT * decay;
+    if score.is_finite() { score } else { 0.0 }
```

---

### [MAJOR] [OPEN] F4-03 `browse_path` and `complete_path` on Windows incorrectly prepend forward slash to absolute drive paths, breaking navigation and execution
- **Status**: OPEN
- **Where**: `src-tauri/src/path_nav.rs:77,150-152`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: In `format_autocomplete` for Windows (lines 72-80):
  ```rust
  #[cfg(windows)]
  {
      if with_slash.starts_with('/') {
          with_slash
      } else {
          format!("/{with_slash}")
      }
  }
  ```
  If `entry.path` is `C:\Tools\Launchify\`, `with_slash` becomes `C:\Tools\Launchify\`. `format_autocomplete` formats this as `/C:\Tools\Launchify\`. When returned as `autocomplete_text` or `complete_path`, and fed back into `browse_path` in subsequent keystrokes, `parse_raw_path` receives `"/C:\Tools\Launchify\"`. In `parse_raw_path` (line 18):
  `let bytes = rest.as_bytes(); if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' { rest.replace('/', "\\") }`
  This produces `"C:\\Tools\\Launchify\\"`. But on the next autocomplete format call, it again prepends `/`, and if the user hits Enter, the executor receives an invalid path string `/C:\Tools\...` instead of an executable path or valid Win32 file path if `autocomplete_text` is used as the target path.
- **Why**: Double slash / backslash handling mixes Unix slash conventions with Windows drive specifiers, producing malformed paths when users complete or browse subdirectories.
- **Fix**:
```diff
     #[cfg(windows)]
     {
-        if with_slash.starts_with('/') {
+        let clean = with_slash.trim_start_matches('/');
+        format!("/{clean}")
-        } else {
-            format!("/{with_slash}")
-        }
     }
```

---

### [MINOR] [OPEN] F4-04 `parse_iso_utc` day count calculation does not validate days in month, accepting invalid dates
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/utils.rs:166-168`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: correctness
- **Scenario**: Querying inline timestamp evaluation with `"2024-02-31"` (February 31) passes the condition `!(1..=31).contains(&day)`, and proceeds to compute a non-existent date epoch as if it were valid, silently wrapping into March in Howard Hinnant's algorithm.
- **Why**: `parse_iso_utc` does not check the maximum days for the given month and leap year (e.g. 28/29 for February, 30 for April/June/Sept/Nov).
- **Fix**:
```diff
+    let max_days = match month {
+        2 => if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) { 29 } else { 28 },
+        4 | 6 | 9 | 11 => 30,
+        _ => 31,
+    };
-    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
+    if !(1..=12).contains(&month) || day < 1 || day > max_days {
         return None;
     }
```

---

### [MINOR] [OPEN] F4-05 `eval_math` lacks recursion depth limit, vulnerable to stack overflow on deeply nested parentheses
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/math.rs:135-242`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: An input like `((((((...((1))...))))))` with 10,000 opening parentheses causes recursive calls `parse_expr` -> `parse_add_sub` -> `parse_mul_div` -> `parse_power` -> `parse_unary` -> `parse_primary` -> `parse_expr`, easily exceeding the Tauri main/worker thread stack and crashing the application process.
- **Why**: `Parser` recursion has no depth counter to terminate gracefully with an error on deeply nested inputs.
- **Fix**:
```diff
 struct Parser {
     tokens: Vec<Token>,
     pos: usize,
+    depth: usize,
 }
...
     fn parse_expr(&mut self) -> Result<f64, String> {
+        if self.depth > 128 {
+            return Err("Expression recursion limit exceeded".to_string());
+        }
+        self.depth += 1;
+        let res = self.parse_add_sub();
+        self.depth -= 1;
+        res
     }
```

---

### [MINOR] [OPEN] F4-06 `generate_uuid_v4` is not cryptographically secure and could collide across rapid invocations
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/utils.rs:69-96`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: security
- **Scenario**: When generating UUIDs via `generate_uuid_v4()`, the function derives pseudo-randomness from `RandomState::new().build_hasher().finish()` and `SystemTime::now()`. In high-speed batches or deterministic environments where `RandomState` seed or time resolution is coarse, generated UUIDs may lack sufficient entropy for sensitive applications (e.g., auth tokens, keys).
- **Why**: Uses standard SipHash random state instead of an OS CSPRNG (`getrandom` or Windows `BCryptGenRandom` / `windows-sys`).
- **Fix**:
```diff
 pub fn generate_uuid_v4() -> String {
-    let h1 = RandomState::new().build_hasher().finish();
-    let h2 = RandomState::new().build_hasher().finish();
-    let nanos = SystemTime::now()
-        .duration_since(UNIX_EPOCH)
-        .map(|d| d.as_nanos() as u64)
-        .unwrap_or(0);
-
-    let hi = h1 ^ nanos.rotate_left(17);
-    let lo = h2 ^ nanos.rotate_right(13);
-
     let mut bytes = [0u8; 16];
-    bytes[..8].copy_from_slice(&hi.to_le_bytes());
-    bytes[8..].copy_from_slice(&lo.to_le_bytes());
+    getrandom::getrandom(&mut bytes).unwrap_or_else(|_| {
+        let h1 = RandomState::new().build_hasher().finish();
+        let h2 = RandomState::new().build_hasher().finish();
+        bytes[..8].copy_from_slice(&h1.to_le_bytes());
+        bytes[8..].copy_from_slice(&h2.to_le_bytes());
+    });
```

---

### [ENHANCEMENT] [OPEN] F4-07 `try_convert` currency rate conversion lacks division-by-zero guard for malformed dynamic rates
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/units.rs:142-144`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: In `try_convert`, if a user provides custom currency rates via `rates` map where `rate_a` is `0.0` or negative, `lookup_currency_rate` checks `r > 0.0 && r.is_finite()`, falling back to `default_currency_rate`. However, if `default_currency_rate` returns `None` for a newly introduced code, but `rate_a` was parsed, or if floating point precision causes `rate_a` to underflow to `0.0`, `result = val / rate_a * rate_b` produces `inf` or `NaN`.
- **Why**: `rate_a` is not checked for non-zero at point of division.
- **Fix**:
```diff
-        let result = val / rate_a * rate_b;
+        if rate_a <= 0.0 {
+            return None;
+        }
+        let result = val / rate_a * rate_b;
```

---

### [ENHANCEMENT] [OPEN] F4-08 `list_matching_entries` in `path_nav.rs` loads all directory entries into memory before sorting and truncating with `limit`
- **Status**: OPEN
- **Where**: `src-tauri/src/path_nav.rs:105-147`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: performance
- **Scenario**: When typing `/C:\Windows\System32\` or large user directory containing tens of thousands of files, `list_matching_entries` iterates over every directory entry with `read_dir`, collects every non-excluded entry into a `Vec<DirEntryInfo>`, and runs `sort_by` on the entire vector before `browse_path` takes `take(limit)`. This causes noticeable UI stutter and disk/memory load.
- **Why**: Directory entries are fully collected and sorted before applying a bound or early-exit, when only the top `limit` entries are displayed in the launcher.
- **Fix**:
```diff
+    // Collect up to a bounded candidate pool (e.g. 1000) or use a binary heap / bounded selection
+    // to avoid sorting millions of entries when only `limit` (e.g. 10) are needed.
```
