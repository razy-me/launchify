pub mod scanner;
pub mod watcher;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_swap::ArcSwap;

use crate::catalog::{builtin_catalog, config_items};
use crate::config::{ExclusionFilter, LaunchifyConfig};
use crate::model::{IndexItem, IndexSnapshot, INDEX_SNAPSHOT_VERSION};
use crate::storage::save_index_atomic;

/// Coordinates full index rebuilds and atomic `ArcSwap` snapshot updates.
#[derive(Clone)]
pub struct IndexManager {
    snapshot: Arc<ArcSwap<IndexSnapshot>>,
    config: Arc<ArcSwap<LaunchifyConfig>>,
    app_dir: PathBuf,
}

impl IndexManager {
    /// Creates a new `IndexManager` backed by `snapshot` and `config`.
    pub fn new(
        snapshot: Arc<ArcSwap<IndexSnapshot>>,
        config: Arc<ArcSwap<LaunchifyConfig>>,
        app_dir: PathBuf,
    ) -> Self {
        Self {
            snapshot,
            config,
            app_dir,
        }
    }

    /// Returns a handle to the current `ArcSwap<IndexSnapshot>`.
    pub fn snapshot(&self) -> Arc<ArcSwap<IndexSnapshot>> {
        Arc::clone(&self.snapshot)
    }

    /// Rebuilds the index from built-in catalog, config items, Start Menu,
    /// `shell:AppsFolder`, and curated folders, swaps it into `ArcSwap`, and
    /// persists `index.bin` atomically.
    pub fn rebuild_now(&self) -> Result<IndexSnapshot, String> {
        let cfg = self.config.load();
        let filter = ExclusionFilter::new(&cfg.extra_exclusions);

        let mut raw_items = Vec::new();
        raw_items.extend(builtin_catalog());
        raw_items.extend(config_items(&cfg));
        raw_items.extend(scanner::scan_apps_folder());
        raw_items.extend(scanner::scan_start_menu(&filter));
        raw_items.extend(scanner::scan_curated_folders(&cfg.curated_folders, &filter));

        let mut seen = HashSet::with_capacity(raw_items.len());
        let mut items: Vec<IndexItem> = Vec::with_capacity(raw_items.len());
        for item in raw_items {
            if seen.insert(item.id.clone()) {
                items.push(item);
            }
        }

        let built_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix,
            items,
        };

        self.snapshot.store(Arc::new(snap.clone()));
        save_index_atomic(&self.app_dir.join("index.bin"), &snap)?;
        Ok(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rebuild_now_updates_arcswap_and_persists_index_bin() {
        let dir = std::env::temp_dir().join(format!("launchify_idx_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let snap = Arc::new(ArcSwap::from_pointee(IndexSnapshot::default()));
        let cfg = Arc::new(ArcSwap::from_pointee(LaunchifyConfig::default()));
        let mgr = IndexManager::new(Arc::clone(&snap), cfg, dir.clone());

        let built = mgr.rebuild_now().expect("rebuild_now");
        assert!(!built.items.is_empty());
        assert_eq!(snap.load().items.len(), built.items.len());
        assert!(dir.join("index.bin").is_file());

        let _ = fs::remove_dir_all(&dir);
    }
}
