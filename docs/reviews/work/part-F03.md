# Fraction Review: F03 — Search Engine, Indexer & File Navigation

### [MAJOR] [OPEN] F03-01 Unsynchronized concurrent `IndexManager::rebuild_now` races on snapshot and `index.bin`
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/mod.rs:46-94`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: A background watcher flush (`watcher.rs:96`) and a manual/IPC-triggered rebuild invoke `IndexManager::rebuild_now(&self)` concurrently on cloned `IndexManager` handles.
- **Why**: `IndexManager` has no mutex or single-flight guard around `rebuild_now`. Two concurrent rebuilds scan the filesystem and extract icons in parallel, then race on `self.snapshot.store(...)` (allowing an older rebuild to overwrite a newer snapshot) and on `save_index_atomic(&self.app_dir.join("index.bin"), &snap)` (causing Windows sharing violations or corrupted temp-file renames).
- **Fix**:
```rust
// In IndexManager:
pub struct IndexManager {
    snapshot: Arc<ArcSwap<IndexSnapshot>>,
    config: Arc<ArcSwap<LaunchifyConfig>>,
    app_dir: PathBuf,
    rebuild_lock: Arc<std::sync::Mutex<()>>,
}

// At the start of rebuild_now(&self):
let _guard = self
    .rebuild_lock
    .lock()
    .map_err(|_| "IndexManager rebuild lock poisoned".to_string())?;
```

### [MAJOR] [OPEN] F03-02 Missing length-difference guard in `typo_fuzzy_score` causes massive heap allocations and long-query freeze
- **Status**: OPEN
- **Where**: `src-tauri/src/search/ranking.rs:109-112` (called from `src-tauri/src/search/engine.rs:95` and `src-tauri/src/find_files.rs:90`)
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: User types a 3+ character query (or pastes a long string, e.g. 500+ chars) into the launcher; `SearchEngine::search` or `search_recent_files` evaluates `typo_fuzzy_score(q, title)` on every non-matching item in the index (thousands of items).
- **Why**: Line 109 calls `damerau_levenshtein(q, title)` unconditionally without checking `title.chars().count().abs_diff(q_len) <= max_edits` (a guard that was added for per-word checks on line 116 but omitted for the full title). Since `damerau_levenshtein` allocates `vec![vec![0usize; m + 1]; n + 1]`, every non-matching item incurs `n + 1` heap allocations per keystroke, and a long pasted query can freeze the search thread or exhaust memory.
- **Fix**:
```rust
let title_len = title.chars().count();
if title_len.abs_diff(q_len) <= max_edits {
    let d_full = damerau_levenshtein(q, title);
    if d_full <= max_edits {
        best_dist = Some(d_full);
    }
}
```

### [MINOR] [OPEN] F03-03 Drive-only input `/C:` or `C:` resolves to process CWD instead of drive root `C:\`
- **Status**: OPEN
- **Where**: `src-tauri/src/path_nav.rs:18-19, 33-35, 48-50`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: On Windows, the user types `/C:` or `C:` in path navigation mode expecting to browse the root of drive `C:`.
- **Why**: `parse_raw_path` produces `"C:"` without a trailing backslash. On Windows, `"C:"` is a drive-relative path referring to the process's current working directory on drive `C:`, so `PathBuf::from("C:").is_dir()` returns `true` and `browse_path` lists the application's working directory instead of `C:\`.
- **Fix**:
```rust
if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
    let mut s = rest.replace('/', "\\");
    if s.len() == 2 {
        s.push('\\');
    }
    s
}
// And in the `else` branch for `trimmed.replace('/', "\\")`:
let mut s = trimmed.replace('/', "\\");
let b = s.as_bytes();
if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
    s.push('\\');
}
s
```

### [MINOR] [OPEN] F03-04 Indexer watcher thread wakes every 250ms while idle, defeating EcoQoS power savings
- **Status**: OPEN
- **Where**: `src-tauri/src/indexer/watcher.rs:79-98`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: performance
- **Scenario**: Launchify runs in the background with no filesystem changes occurring.
- **Why**: `spawn_watcher` unconditionally uses `default(Duration::from_millis(250))` in `crossbeam_channel::select!`, waking the thread 4 times per second indefinitely even when `debouncer` is clean (`!dirty`). This causes unnecessary CPU wakeups and negates the battery savings intended by `set_current_thread_eco_qos()`.
- **Fix**:
```rust
if !debouncer.dirty {
    crossbeam_channel::select! {
        recv(stop_rx) -> _ => break,
        recv(event_rx) -> msg => {
            if msg.is_err() {
                break;
            }
            debouncer.record_event(start.elapsed().as_millis() as u64);
        }
    }
} else {
    crossbeam_channel::select! {
        recv(stop_rx) -> _ => break,
        recv(event_rx) -> msg => {
            if msg.is_err() {
                break;
            }
            debouncer.record_event(start.elapsed().as_millis() as u64);
        }
        default(Duration::from_millis(250)) => {}
    }
}
```
