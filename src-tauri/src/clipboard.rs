use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::storage;

const MAX_CLIPBOARD_ENTRIES: usize = 50;
static NEXT_ENTRY_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub text: String,
    pub timestamp_unix: u64,
}

pub struct ClipboardStore {
    entries: Mutex<Vec<ClipboardEntry>>,
    path: PathBuf,
}

impl ClipboardStore {
    /// Loads persisted clipboard history from `path`, or initializes an empty store.
    pub fn load_or_new(path: PathBuf) -> Self {
        let mut entries: Vec<ClipboardEntry> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        entries.truncate(MAX_CLIPBOARD_ENTRIES);
        Self {
            entries: Mutex::new(entries),
            path,
        }
    }

    /// Inserts `text` at the front of history, deduplicating and capping at 50 entries.
    pub fn push(&self, text: String, now_unix: u64) {
        if text.trim().is_empty() {
            return;
        }

        let seq = NEXT_ENTRY_SEQ.fetch_add(1, Ordering::Relaxed);
        let entry = ClipboardEntry {
            id: format!("{now_unix}-{seq}"),
            text,
            timestamp_unix: now_unix,
        };

        let snapshot = {
            let Ok(mut guard) = self.entries.lock() else {
                return;
            };
            guard.retain(|e| e.text != entry.text);
            guard.insert(0, entry);
            guard.truncate(MAX_CLIPBOARD_ENTRIES);
            guard.clone()
        };

        if let Ok(bytes) = serde_json::to_vec(&snapshot) {
            if let Err(e) = storage::write_atomic(&self.path, &bytes) {
                tracing::warn!("Failed to persist clipboard history: {e}");
            }
        }
    }

    /// Searches clipboard history newest-first using case-insensitive substring matching.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ClipboardEntry> {
        let Ok(guard) = self.entries.lock() else {
            return Vec::new();
        };
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return guard.iter().take(limit).cloned().collect();
        }
        let needle = trimmed.to_lowercase();
        guard
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&needle))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Looks up a clipboard entry by its `id`.
    pub fn get_by_id(&self, id: &str) -> Option<ClipboardEntry> {
        let guard = self.entries.lock().ok()?;
        guard.iter().find(|e| e.id == id).cloned()
    }
}

/// Spawns a background thread that watches the Windows clipboard for text changes.
#[cfg(windows)]
pub fn spawn_clipboard_watcher(store: Arc<ClipboardStore>) {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    const CF_UNICODETEXT_FMT: u32 = 13;

    let _ = std::thread::Builder::new()
        .name("launchify-clipboard-watcher".into())
        .spawn(move || {
            let mut last_seq = unsafe { GetClipboardSequenceNumber() };

            loop {
                std::thread::sleep(Duration::from_millis(500));
                let seq = unsafe { GetClipboardSequenceNumber() };
                if seq == 0 || seq == last_seq {
                    continue;
                }
                last_seq = seq;

                let text = unsafe {
                    if OpenClipboard(None).is_err() {
                        continue;
                    }
                    let result = match GetClipboardData(CF_UNICODETEXT_FMT) {
                        Ok(handle) => {
                            let hglobal = HGLOBAL(handle.0);
                            let ptr = GlobalLock(hglobal) as *const u16;
                            if ptr.is_null() {
                                None
                            } else {
                                let mut len = 0usize;
                                while *ptr.add(len) != 0 {
                                    len += 1;
                                }
                                let slice = std::slice::from_raw_parts(ptr, len);
                                let s = String::from_utf16_lossy(slice);
                                let _ = GlobalUnlock(hglobal);
                                Some(s)
                            }
                        }
                        Err(_) => None,
                    };
                    let _ = CloseClipboard();
                    result
                };

                if let Some(t) = text {
                    let now_unix = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    store.push(t, now_unix);
                }
            }
        });
}

/// No-op on non-Windows platforms.
#[cfg(not(windows))]
pub fn spawn_clipboard_watcher(_store: Arc<ClipboardStore>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> ClipboardStore {
        let path = std::env::temp_dir().join(format!(
            "launchify-cb-test-{name}-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        ClipboardStore::load_or_new(path)
    }

    #[test]
    fn push_ignores_empty_and_deduplicates() {
        let store = temp_store("dedup");
        store.push("   ".to_string(), 100);
        assert!(store.search("", 10).is_empty());

        store.push("first".to_string(), 101);
        store.push("second".to_string(), 102);
        store.push("first".to_string(), 103);

        let all = store.search("", 10);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].text, "first");
        assert_eq!(all[0].timestamp_unix, 103);
        assert_eq!(all[1].text, "second");
    }

    #[test]
    fn push_caps_at_fifty_entries() {
        let store = temp_store("cap50");
        for i in 0..60 {
            store.push(format!("item-{i}"), i as u64);
        }
        let all = store.search("", 100);
        assert_eq!(all.len(), 50);
        assert_eq!(all[0].text, "item-59");
        assert_eq!(all[49].text, "item-10");
    }

    #[test]
    fn search_is_case_insensitive_and_respects_limit() {
        let store = temp_store("search");
        store.push("Alpha Beta".to_string(), 1);
        store.push("gamma delta".to_string(), 2);
        store.push("ALPHA two".to_string(), 3);

        let hits = store.search("alpha", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "ALPHA two");

        let all_alpha = store.search("alpha", 10);
        assert_eq!(all_alpha.len(), 2);
        assert_eq!(all_alpha[0].text, "ALPHA two");
        assert_eq!(all_alpha[1].text, "Alpha Beta");
    }
}
