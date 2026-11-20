use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use crate::model::{FrecencyStore, IndexSnapshot, INDEX_SNAPSHOT_VERSION};

/// Writes `bytes` atomically to `path` via a `.tmp` sibling file and rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create parent dir {}: {e}", parent.display()))?;
    }

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), unique));
    {
        let file = File::create(&tmp_path)
            .map_err(|e| format!("failed to create temp file {}: {e}", tmp_path.display()))?;
        let mut writer = BufWriter::new(file);
        writer
            .write_all(bytes)
            .map_err(|e| format!("failed to write temp file {}: {e}", tmp_path.display()))?;
        writer
            .flush()
            .map_err(|e| format!("failed to flush temp file {}: {e}", tmp_path.display()))?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let src: Vec<u16> = tmp_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let res = unsafe {
            MoveFileExW(
                PCWSTR(src.as_ptr()),
                PCWSTR(dst.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if let Err(e) = res {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "failed to replace {} -> {}: {e}",
                tmp_path.display(),
                path.display()
            ));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "failed to rename {} -> {}: {e}",
            tmp_path.display(),
            path.display()
        )
    })
}

/// Loads `index.bin` using `bincode` v1.
///
/// Falls back to an empty `IndexSnapshot` if the file is missing, corrupt,
/// or has an incompatible version.
pub fn load_index(path: &Path) -> Result<IndexSnapshot, String> {
    if !path.exists() {
        return Ok(IndexSnapshot::default());
    }

    let file = File::open(path)
        .map_err(|e| format!("failed to open index {}: {e}", path.display()))?;
    let reader = BufReader::new(file);

    use bincode::Options;
    const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
    match bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .with_limit(MAX_INDEX_BYTES)
        .deserialize_from::<_, IndexSnapshot>(reader)
    {
        Ok(snap) if snap.version == INDEX_SNAPSHOT_VERSION => Ok(snap),
        Ok(snap) => {
            tracing::warn!(
                expected = INDEX_SNAPSHOT_VERSION,
                found = snap.version,
                "index snapshot version mismatch; resetting to empty"
            );
            Ok(IndexSnapshot::default())
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "corrupt index snapshot; resetting to empty"
            );
            Ok(IndexSnapshot::default())
        }
    }
}

/// Atomically persists `IndexSnapshot` to `path` using `bincode` v1.
pub fn save_index_atomic(path: &Path, snap: &IndexSnapshot) -> Result<(), String> {
    let bytes =
        bincode::serialize(snap).map_err(|e| format!("failed to serialize index: {e}"))?;
    write_atomic(path, &bytes)
}

/// Loads `frecency.json`, falling back to an empty `FrecencyStore` if missing or corrupt.
pub fn load_frecency(path: &Path) -> FrecencyStore {
    if !path.exists() {
        return FrecencyStore::default();
    }

    let Ok(raw) = fs::read_to_string(path) else {
        tracing::warn!(path = %path.display(), "failed to read frecency store; using empty");
        return FrecencyStore::default();
    };

    match serde_json::from_str(&raw) {
        Ok(store) => store,
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "corrupt frecency store; using empty"
            );
            FrecencyStore::default()
        }
    }
}

/// Atomically persists `FrecencyStore` as JSON to `path`.
pub fn save_frecency_atomic(path: &Path, store: &FrecencyStore) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("failed to serialize frecency store: {e}"))?;
    write_atomic(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FrecencyEntry, IndexItem, ItemKind};
    use std::collections::HashMap;
    use std::time::Instant;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "launchify_storage_test_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn index_bincode_round_trip_2000_items_fast() {
        let dir = temp_dir("roundtrip_2000");
        let index_path = dir.join("index.bin");

        let items: Vec<IndexItem> = (0..2000)
            .map(|i| IndexItem {
                id: format!("app:item.{i}"),
                title: format!("Application {i}"),
                subtitle: format!("C:\\Program Files\\Vendor\\App{i}\\app{i}.exe"),
                keywords: vec!["tool".into(), "utility".into()],
                kind: ItemKind::App,
                target: format!("C:\\Apps\\app{i}.exe"),
                icon_key: Some(format!("hash_{i:04}")),
            })
            .collect();

        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 1_700_000_000,
            items,
        };

        save_index_atomic(&index_path, &snap).unwrap();

        let start = Instant::now();
        let loaded = load_index(&index_path).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(loaded.items.len(), 2000);
        assert_eq!(loaded, snap);
        let budget_ms = if cfg!(debug_assertions) { 100 } else { 20 };
        assert!(
            elapsed.as_millis() < budget_ms,
            "bincode load of 2000 items took {:?} (expected < {budget_ms}ms)",
            elapsed
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_index_falls_back_to_empty() {
        let dir = temp_dir("corrupt_index");
        let index_path = dir.join("index.bin");
        fs::write(&index_path, b"not valid bincode bytes").unwrap();

        let loaded = load_index(&index_path).unwrap();
        assert_eq!(loaded.version, INDEX_SNAPSHOT_VERSION);
        assert!(loaded.items.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn frecency_atomic_save_and_corrupt_fallback() {
        let dir = temp_dir("frecency");
        let frec_path = dir.join("frecency.json");

        let mut store = FrecencyStore::default();
        store.entries.insert(
            "app:spotify".into(),
            FrecencyEntry {
                count: 5,
                last_used_unix: 1_700_000_123,
            },
        );
        let mut inner = HashMap::new();
        inner.insert("app:spotify".into(), 3);
        store.prefix_memory.insert("sp".into(), inner);

        save_frecency_atomic(&frec_path, &store).unwrap();
        let loaded = load_frecency(&frec_path);
        assert_eq!(loaded, store);

        fs::write(&frec_path, "{broken json").unwrap();
        let fallback = load_frecency(&frec_path);
        assert!(fallback.entries.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_overwrites_existing_file() {
        let dir = temp_dir("overwrite_existing");
        let target = dir.join("data.bin");

        write_atomic(&target, b"initial").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"initial");

        write_atomic(&target, b"updated").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"updated");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_index_rejects_oversized_corrupt_length_prefix() {
        let dir = temp_dir("oversized_index");
        let index_path = dir.join("index.bin");
        // version = 1 (u32 LE), items len = u64::MAX (would OOM without with_limit)
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&INDEX_SNAPSHOT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        fs::write(&index_path, &bytes).unwrap();

        let snap = load_index(&index_path).unwrap();
        assert!(snap.items.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
