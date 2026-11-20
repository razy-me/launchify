# Review Findings — F04 Converter & Inline Tools

### [MAJOR] [OPEN] F04-01 Unbounded recursion in math parser allows stack overflow crash
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/math.rs:187-242`
- **Source**: `r-review` (2026-11-20) — `lens: F1b`
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: Pasting a deeply nested expression such as `"(".repeat(5000)` or `"-".repeat(5000) + "1"` or `"2^".repeat(5000) + "1"` into the launcher triggers `eval_math`, which recurses unconditionally in `parse_primary`, `parse_unary`, and `parse_power` until the thread overflows its stack and aborts the entire Tauri backend process.
- **Why**: The recursive-descent parser tracks token position (`pos`) but has no recursion depth counter or maximum nesting limit. Because `inline_tools::evaluate` runs on untrusted user input (including pasted strings), an accidental or adversarial deep input crashes the app.
- **Fix**:
```diff
 struct Parser {
     tokens: Vec<Token>,
     pos: usize,
+    depth: usize,
 }
 
+const MAX_RECURSION_DEPTH: usize = 128;
+
 impl Parser {
     fn new(tokens: Vec<Token>) -> Self {
-        Self { tokens, pos: 0 }
+        Self { tokens, pos: 0, depth: 0 }
     }
+
+    fn with_depth<F>(&mut self, f: F) -> Result<f64, String>
+    where
+        F: FnOnce(&mut Self) -> Result<f64, String>,
+    {
+        if self.depth >= MAX_RECURSION_DEPTH {
+            return Err("Expression nesting too deep".to_string());
+        }
+        self.depth += 1;
+        let res = f(self);
+        self.depth -= 1;
+        res
+    }
```
Wrap recursive calls in `parse_power`, `parse_unary`, and `parse_primary` with `self.with_depth(...)`.

### [MAJOR] [OPEN] F04-02 Unary minus binds tighter than exponentiation (`-2^2` evaluates to `4`)
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/math.rs:187-210`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Evaluating `-2^2` returns `4` instead of `-4`, and `-2^2 + 4` returns `8` instead of `0`.
- **Why**: `parse_power` calls `self.parse_unary()` for the base before checking for `Token::Caret`. In standard arithmetic precedence, exponentiation binds tighter than unary negation on the base (`-2^2 == -(2^2)`), while unary negation is still allowed in the exponent (`2^-3 == 2^(-3)`).
- **Fix**:
```diff
     fn parse_mul_div(&mut self) -> Result<f64, String> {
-        let mut left = self.parse_power()?;
+        let mut left = self.parse_unary()?;
         while let Some(tok) = self.peek() {
             match tok {
                 Token::Star => {
                     self.next();
-                    left *= self.parse_power()?;
+                    left *= self.parse_unary()?;
                 }
                 Token::Slash => {
                     self.next();
-                    let right = self.parse_power()?;
+                    let right = self.parse_unary()?;
                     ...
                 }
                 Token::Percent => {
                     self.next();
-                    let right = self.parse_power()?;
+                    let right = self.parse_unary()?;
                     ...
                 }
                 _ => break,
             }
         }
         Ok(left)
     }
 
+    fn parse_unary(&mut self) -> Result<f64, String> {
+        match self.peek() {
+            Some(Token::Plus) => {
+                self.next();
+                self.parse_unary()
+            }
+            Some(Token::Minus) => {
+                self.next();
+                Ok(-self.parse_unary()?)
+            }
+            _ => self.parse_power(),
+        }
+    }
+
     fn parse_power(&mut self) -> Result<f64, String> {
-        let base = self.parse_unary()?;
+        let base = self.parse_primary()?;
         if matches!(self.peek(), Some(Token::Caret)) {
             self.next();
-            let exp = self.parse_power()?;
+            let exp = self.parse_unary()?;
             Ok(base.powf(exp))
         } else {
             Ok(base)
         }
     }
```

### [MAJOR] [OPEN] F04-03 External converter subprocesses lack timeouts and argument-injection guards
- **Status**: OPEN
- **Where**: `src-tauri/src/converter/mod.rs:436-456`, `src-tauri/src/converter/mod.rs:571-652`
- **Source**: `r-review` (2026-11-20) — `lens: F1b`
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: (1) Converting a malformed or very large media/PDF/document file via `ffmpeg`, `pandoc`, `pdftotext`, `pdftoppm`, or `magick` calls `.output()`, which blocks the worker thread indefinitely with no timeout and buffers unbounded stderr in memory. (2) If a relative path starting with `-` (e.g. `--lua-filter=evil.lua` or `-format`) is passed to `pandoc`, `pdftotext`, `pdftoppm`, or `magick`, it is parsed as a CLI flag rather than a file path.
- **Why**: Subprocesses are invoked directly with `Command::new(...).arg(source).output()` without canonicalizing/prefixing paths (or passing `--` where supported) and without a timeout or bounded output reader.
- **Fix**:
```diff
+fn safe_cli_path(path: &Path) -> PathBuf {
+    fs::canonicalize(path).unwrap_or_else(|_| {
+        if path.is_relative() {
+            Path::new(".").join(path)
+        } else {
+            path.to_path_buf()
+        }
+    })
+}
```
Pass `safe_cli_path(source)` / `safe_cli_path(out_path)` to external tools, and spawn with a timeout (killing the child on expiry) rather than blocking indefinitely on `.output()`.

### [MAJOR] [OPEN] F04-04 TOCTOU race in `build_output_path` allows concurrent conversions to clobber files
- **Status**: OPEN
- **Where**: `src-tauri/src/converter/mod.rs:103-125`, `src-tauri/src/converter/mod.rs:679`
- **Source**: `r-review` (2026-11-20) — `lens: F1b`
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: Triggering two conversions of the same source file to the same target format concurrently (e.g. double-clicking Convert) causes both calls to observe `!primary.exists()` in `build_output_path` and select the identical `out_path`, resulting in one conversion silently overwriting or corrupting the other's output (especially with `ffmpeg -y`).
- **Why**: Collision avoidance relies on a non-atomic `Path::exists()` check before writing, rather than atomically reserving the destination file with `OpenOptions::new().write(true).create_new(true)`.
- **Fix**:
```diff
-fn build_output_path(source: &Path, target_ext: &str) -> PathBuf {
+fn reserve_output_path(source: &Path, target_ext: &str) -> Result<PathBuf, String> {
     let parent = source.parent().unwrap_or_else(|| Path::new("."));
     let stem = source
         .file_stem()
         .and_then(|s| s.to_str())
         .unwrap_or("output");
-    let primary = parent.join(format!("{stem}.{target_ext}"));
-    if !primary.exists() {
-        return primary;
-    }
-    ...
+    for idx in 0u32..10_000 {
+        let candidate = match idx {
+            0 => parent.join(format!("{stem}.{target_ext}")),
+            1 => parent.join(format!("{stem}_converted.{target_ext}")),
+            n => parent.join(format!("{stem}_converted_{n}.{target_ext}")),
+        };
+        match fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
+            Ok(_) => return Ok(candidate),
+            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
+            Err(e) => return Err(format!("Failed to reserve output {}: {e}", candidate.display())),
+        }
+    }
+    Err("Too many existing converted files in destination directory".to_string())
 }
```
(Remove the reserved placeholder file on conversion error.)

### [MINOR] [OPEN] F04-05 `format_number` rounds tiny values to `0` and expands huge floats into 300+ digit strings
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/math.rs:297-309`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Evaluating `1e-12` or `1 / 1000000000000` displays `"0"` instead of `1e-12`, while `2^1000` displays a 302-character integer string that overflows the inline preview UI.
- **Why**: `format!("{val:.10}")` uses fixed-point formatting with 10 decimal places rather than switching to scientific notation for very small (`|val| < 1e-9`) or very large (`|val| >= 1e15`) magnitudes.
- **Fix**:
```diff
 pub fn format_number(val: f64) -> String {
     if val == 0.0 {
         return "0".to_string();
     }
+    let abs = val.abs();
+    if abs < 1e-9 || abs >= 1e15 {
+        return format!("{val:.6e}");
+    }
     // Round to 10 decimal places to suppress 0.1 + 0.2 = 0.30000000000000004
     let rounded = format!("{val:.10}");
```

### [MINOR] [OPEN] F04-06 Per-byte `String` allocation in `txt` to `hex` conversion
- **Status**: OPEN
- **Where**: `src-tauri/src/converter/mod.rs:524-530`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: Converting a 50 MB `.txt` file to `.hex` performs 50 million separate heap allocations (`format!("{b:02x}")` per byte), causing severe latency and allocator churn.
- **Why**: `bytes.iter().map(|b| format!("{b:02x}")).collect()` allocates a temporary `String` for every byte instead of writing into a single pre-allocated buffer.
- **Fix**:
```diff
         ("txt", "hex") => {
             let bytes =
                 fs::read(source).map_err(|e| format!("Failed to read {}: {e}", source.display()))?;
-            let encoded: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
+            use std::fmt::Write as _;
+            let mut encoded = String::with_capacity(bytes.len() * 2);
+            for b in bytes {
+                let _ = write!(&mut encoded, "{b:02x}");
+            }
             fs::write(out_path, encoded)
                 .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))
         }
```

### [MINOR] [OPEN] F04-07 Unchecked float overflow in unit and currency conversion can return `"inf"`
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/units.rs:128-147`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: Entering `1e308 tb to b` or `1e308 usd to jpy` overflows `f64` to infinity and displays `"inf B"` or `"inf JPY"` in the inline preview.
- **Why**: `try_convert` validates `val.is_finite()` on the input, but does not verify `result.is_finite()` after multiplying/dividing by conversion factors or exchange rates.
- **Fix**:
```diff
         let result = if cat_a == UnitCategory::Temperature {
             convert_temperature(val, label_a, label_b)
         } else {
             val * factor_a / factor_b
         };
+        if !result.is_finite() {
+            return None;
+        }
         let num_str = format_number(result);
```
Add the same `if !result.is_finite() { return None; }` guard after `let result = val / rate_a * rate_b;` on line 143.

### [MINOR] [OPEN] F04-08 `parse_iso_utc` accepts invalid calendar dates like `2023-02-31` and silently rolls them over
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/utils.rs:163-168`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: Typing `2023-02-31` or `2023-04-31` produces a Unix timestamp corresponding to `2023-03-03` or `2023-05-01` instead of rejecting the invalid date.
- **Why**: `parse_iso_utc` only checks `1..=31` for `day` without validating the actual number of days in the given `(year, month)` (including leap years).
- **Fix**:
```diff
-    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
+    if !(1..=12).contains(&month) {
+        return None;
+    }
+    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
+    let max_day = match month {
+        2 => if is_leap { 29 } else { 28 },
+        4 | 6 | 9 | 11 => 30,
+        _ => 31,
+    };
+    if !(1..=max_day).contains(&day) {
         return None;
     }
```

### [MINOR] [OPEN] F04-09 `generate_uuid_v4` uses non-cryptographic `RandomState` hashing of empty input
- **Status**: OPEN
- **Where**: `src-tauri/src/inline_tools/utils.rs:69-82`
- **Source**: `r-review` (2026-11-20) — `lens: F1b`
- **Confidence**: HIGH
- **Category**: security
- **Scenario**: A user runs `uuid` in the launcher to generate a UUID v4 for an API key, token, or unguessable identifier; the output is derived from `std::collections::hash_map::RandomState` (which is a per-thread SipHash seed incremented deterministically, not a CSPRNG) XORed with wall-clock nanoseconds.
- **Why**: `RandomState::new().build_hasher().finish()` is explicitly not cryptographically secure and hashes zero bytes. RFC 4122 §4.4 recommends a cryptographic random source for UUID v4.
- **Fix**: Use `getrandom::fill(&mut bytes)` (or `uuid::Uuid::new_v4()`) instead of `RandomState` + `SystemTime`.
