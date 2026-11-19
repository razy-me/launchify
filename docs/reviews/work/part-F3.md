# Fraction Review: F3 - Search Engine, Ranking & Indexer

### [MAJOR] [OPEN] F3-01 Start Menu scanner hardcodes icon_key to invalid format causing broken UI icons
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/scanner.rs:104`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When Launchify scans Start Menu shortcuts, `scanner.rs` generates items with `icon_key: Some(format!("app_{stem}"))` (e.g. `app_Visual Studio Code`). Then `IndexManager::rebuild_now` in `indexer/mod.rs:71-74` extracts the icon into `%LOCALAPPDATA%/Launchify/icons/<hash>.png`, where `<hash>` is the 16-hex fnv1a-64 hash of `target`. However, `item.icon_key` is only updated if `icon_cache.get_or_extract(&key, &item.target).is_some()`. If extraction fails (e.g., target is a broken shortcut or network lnk), `item.icon_key` remains `"app_{stem}"`. The frontend attempts to resolve `app_{stem}` as an icon path in `IconCache::icon_path`, which produces `app_Visual_Studio_Code.png`, a file that does not exist. More importantly, scanner pre-populates `icon_key` with a dummy key instead of `None`, breaking the invariant that `icon_key` is either `None` or a valid cache key.
- **Why**: `scanner.rs` unilaterally populates `icon_key` with a synthetic string `app_{stem}` instead of leaving it `None` for the index manager to assign the actual cache key.
- **Fix**:
```diff
--- a/src-tauri/src/indexer/scanner.rs
+++ b/src-tauri/src/indexer/scanner.rs
@@ -101,4 +101,4 @@ fn scan_lnk_dir(dir: &Path, filter: &ExclusionFilter, out: &mut Vec<IndexItem>)
             kind: ItemKind::App,
             target: target.clone(),
-            icon_key: Some(format!("app_{stem}")),
+            icon_key: None,
         });
```

---

### [MAJOR] [OPEN] F3-02 Windows `extract_png_for_target` does not uninitialize COM and risks uncooperative apartment initialization
- **Status**: OPEN
- **Where**: `src-tauri/src/icons.rs:77-79`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: `extract_png_for_target` calls `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` unconditionally on whatever thread invokes `get_or_extract` (e.g. background indexing thread or watcher thread) without a matching `CoUninitialize()`. If the calling thread was previously initialized as MTA (`COINIT_MULTITHREADED`), `CoInitializeEx` returns `RPC_E_CHANGED_MODE` (0x80010106), which is silently ignored via `let _ = ...`. While SHGetFileInfoW may work, leaking COM apartment state or mismatched apartment models across repeated icon extractions violates Windows COM threading contracts and can leak thread-local COM state or cause RPC errors in shell extensions called by `SHGetFileInfoW`.
- **Why**: COM initialization is performed repeatedly per icon without checking the return value or ensuring balanced `CoUninitialize()` via an RAII guard.
- **Fix**:
```diff
--- a/src-tauri/src/icons.rs
+++ b/src-tauri/src/icons.rs
@@ -72,3 +72,3 @@ fn extract_png_for_target(target: &str) -> Option<Vec<u8>> {
     };
-    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
+    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
     use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
@@ -77,4 +77,7 @@ fn extract_png_for_target(target: &str) -> Option<Vec<u8>> {
     unsafe {
-        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
+        let co_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
+        let should_uninit = co_init.is_ok();
+        struct ComGuard(bool);
+        impl Drop for ComGuard {
+            fn drop(&mut self) { if self.0 { unsafe { CoUninitialize(); } } }
+        }
+        let _guard = ComGuard(should_uninit);
```

---

### [MINOR] [OPEN] F3-03 `top_frecency_items` and `search` use `select_nth_unstable_by` with reversed comparison against subsequent `sort_by`
- **Status**: OPEN
- **Where**: `src-tauri/src/search/engine.rs:112-117` and `src-tauri/src/search/engine.rs:153-158`
- **Where**: `src-tauri/src/search/engine.rs:112`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When `scored.len() > max_results`, `select_nth_unstable_by(max_results, |a, b| b.1.partial_cmp(&a.1)...)` partitions elements such that the first `max_results` elements are arbitrary elements relative to the `max_results`-th element. In Rust, `slice.select_nth_unstable(k)` guarantees elements before index `k` are `<= elem[k]`, and elements after are `>= elem[k]` according to the comparator. Because the comparator sorts in descending order (`b.1.cmp(&a.1)`), elements before `max_results` have higher or equal scores than index `max_results`. However, index `max_results` is out of bounds for the truncated slice `0..max_results` (length `max_results`), and if `scored.len() == max_results`, it panics if called with `max_results == scored.len()` (though guarded by `> max_results`). More importantly, if `max_results` items are desired, index `max_results - 1` should be the pivot to partition the top `max_results` elements, or `max_results` is valid only because slice length is at least `max_results + 1`. Rust's `select_nth_unstable(n)` partitions into `0..n`, `n`, `n+1..len`. Truncating to `max_results` retains `0..max_results`, which are indeed elements whose comparator relation to `n` is `Less` (i.e. larger score). However, using `max_results` when `max_results == 0` is guarded, but when `max_results > 0`, the pivot `scored[max_results]` is dropped by `truncate(max_results)`. The elements kept are strictly the top `max_results`. However, `a.0.title.cmp(&b.0.title)` secondary tie-breaker in `b.cmp(a)` sorts titles in ascending order when scores match. While mathematically top-N is preserved, `select_nth_unstable_by` comparator:
`b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then_with(|| a.0.title.cmp(&b.0.title))` has a mismatch if `b.1 == a.1` between `a.cmp(b)` and `b.cmp(a)` if not careful.
- **Why**: `select_nth_unstable_by(max_results)` on a descending order puts elements with greater score into indices `0..max_results`. While valid, using `max_results` as index partitions `max_results` items before index `max_results`.
- **Fix**: Ensure safe indexing and maintain consistency:
```diff
--- a/src-tauri/src/search/engine.rs
+++ b/src-tauri/src/search/engine.rs
@@ -111,3 +111,3 @@ impl SearchEngine {
         if scored.len() > max_results {
-            scored.select_nth_unstable_by(max_results, |a, b| {
+            let (left, _, _) = scored.select_nth_unstable_by(max_results, |a, b| {
                 b.1.partial_cmp(&a.1)
```

---

### [MINOR] [OPEN] F3-04 `scan_apps_folder` is an empty stub returning no store apps
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/scanner.rs:110-112`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: When `IndexManager::rebuild_now` runs, it calls `scanner::scan_apps_folder()`. Any modern Windows packaged apps (e.g., Windows Terminal from Microsoft Store, Calculator, Spotify from Store, Slack, WhatsApp) installed via MSIX/AppX without standard Start Menu `.lnk` files will never be indexed or discoverable by Launchify search.
- **Why**: `scan_apps_folder()` is an unimplemented stub returning `Vec::new()`.
- **Fix**: Document contract or implement Shell COM enumeration for `shell:AppsFolder` (via `IShellItem` / `IEnumShellItems`).
```diff
--- a/src-tauri/src/indexer/scanner.rs
+++ b/src-tauri/src/indexer/scanner.rs
@@ -109,4 +109,5 @@ fn scan_lnk_dir(dir: &Path, filter: &ExclusionFilter, out: &mut Vec<IndexItem>)
 /// Enumerates `shell:AppsFolder` store/packaged apps on Windows.
 pub fn scan_apps_folder() -> Vec<IndexItem> {
+    // TODO: Enumerate shell:AppsFolder via IShellFolder / IEnumIDList to discover UWP/MSIX packaged apps
     Vec::new()
 }
```

---

### [MINOR] [OPEN] F3-05 FNV-1a 64-bit hash in `IndexManager` suffers potential collision across different targets
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/mod.rs:71` and `src-tauri/src/indexer/mod.rs:97-104`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: robustness
- **Scenario**: `fnv1a_64` produces a 64-bit hash formatted as 16 hex characters for the icon cache key. For long file paths or case-varying Windows paths (e.g. `C:\Program Files\...` vs `c:\program files\...`), case differences result in different hash keys for the same executable, causing redundant icon extractions and disk writes. Conversely, 64-bit FNV-1a has non-negligible collision risk compared to SHA-256 or BLAKE3 across thousands of indexed items.
- **Why**: Windows paths are case-insensitive, but `fnv1a_64(item.target.as_bytes())` hashes raw byte casing without normalization.
- **Fix**: Normalize target path to lowercase before hashing:
```diff
--- a/src-tauri/src/indexer/mod.rs
+++ b/src-tauri/src/indexer/mod.rs
@@ -70,3 +70,4 @@ impl IndexManager {
                 if is_app && !item.target.is_empty() {
-                    let key = format!("{:016x}", fnv1a_64(item.target.as_bytes()));
+                    let normalized_target = item.target.to_lowercase();
+                    let key = format!("{:016x}", fnv1a_64(normalized_target.as_bytes()));
                     if icon_cache.get_or_extract(&key, &item.target).is_some() {
```

---

### [ENHANCEMENT] [OPEN] F3-06 `word_starts` allocation in `is_word_boundary_match` per query evaluation
- **Status**: OPEN
- **Where**: `src-tauri/src/search/ranking.rs:35-38`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: During every keystroke search, `title_match_bonus` is called for every `IndexItem` in the catalog. Inside `is_word_boundary_match`, `title_lower.split(...).filter_map(...).collect()` allocates a new `Vec<char>` for every single item on every query, generating thousands of small heap allocations per search.
- **Why**: `word_starts` is collected into a heap vector instead of iterating over word start characters using an iterator without heap allocation.
- **Fix**:
```diff
--- a/src-tauri/src/search/ranking.rs
+++ b/src-tauri/src/search/ranking.rs
@@ -34,10 +34,2 @@ fn is_word_boundary_match(title_lower: &str, query_lower: &str) -> bool {
     }
-
-    let word_starts: Vec<char> = title_lower
-        .split(|c: char| !c.is_alphanumeric())
-        .filter_map(|w| w.chars().next())
-        .collect();
-
-    if word_starts.len() < q_chars.len() {
-        return false;
-    }
 
     // Check if query chars match a contiguous or ordered subsequence of word-start chars.
     let mut qi = 0;
-    for wc in word_starts {
+    let mut prev_is_non_alnum = true;
+    for c in title_lower.chars() {
+        let is_alnum = c.is_alphanumeric();
+        if is_alnum && prev_is_non_alnum {
+            if c == q_chars[qi] {
+                qi += 1;
+                if qi == q_chars.len() {
+                    return true;
+                }
+            }
+        }
+        prev_is_non_alnum = !is_alnum;
+    }
```

---

### [ENHANCEMENT] [OPEN] F3-07 Rebuild deduplication uses `title.to_ascii_lowercase()` which breaks non-ASCII app names
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/mod.rs:66`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: robustness
- **Scenario**: `seen_app_titles.insert(item.title.to_ascii_lowercase())` only lowercases ASCII characters. If an application title contains localized characters (e.g., German umlauts `Über-App`, Cyrillic, or French accents), two shortcuts differing only in non-ASCII casing will not be recognized as duplicates and will both be retained.
- **Why**: `to_ascii_lowercase` is used instead of Unicode `to_lowercase()`.
- **Fix**:
```diff
--- a/src-tauri/src/indexer/mod.rs
+++ b/src-tauri/src/indexer/mod.rs
@@ -65,3 +65,3 @@ impl IndexManager {
             );
-            if is_app && !seen_app_titles.insert(item.title.to_ascii_lowercase()) {
+            if is_app && !seen_app_titles.insert(item.title.to_lowercase()) {
                 continue;
```
