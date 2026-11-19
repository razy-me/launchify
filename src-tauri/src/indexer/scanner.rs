use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::synonym_keywords;
use crate::config::ExclusionFilter;
use crate::model::{IndexItem, ItemKind};

/// Scans only the top-level entries of each curated folder, skipping excluded
/// directories and media/excluded files.
pub fn scan_curated_folders(folders: &[PathBuf], filter: &ExclusionFilter) -> Vec<IndexItem> {
    let mut items = Vec::new();
    for folder in folders {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let is_dir = ft.is_dir();
            let excluded = if is_dir {
                filter.is_excluded_dir(&path)
            } else {
                filter.is_excluded_file(&path)
            };
            if excluded {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let kind = if is_dir {
                ItemKind::Folder
            } else {
                ItemKind::File
            };
            let target = path.to_string_lossy().into_owned();
            items.push(IndexItem {
                id: format!("curated:{target}"),
                title: name.to_string(),
                subtitle: folder.to_string_lossy().into_owned(),
                keywords: Vec::new(),
                kind,
                target,
                icon_key: None,
            });
        }
    }
    items
}

/// Scans standard Windows Start Menu `.lnk` shortcuts recursively.
pub fn scan_start_menu(filter: &ExclusionFilter) -> Vec<IndexItem> {
    let mut roots = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        roots.push(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Ok(programdata) = std::env::var("ProgramData") {
        roots.push(PathBuf::from(programdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }

    let mut items = Vec::new();
    for root in roots {
        scan_lnk_dir(&root, filter, &mut items);
    }
    items
}

fn scan_lnk_dir(dir: &Path, filter: &ExclusionFilter, out: &mut Vec<IndexItem>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            if !filter.is_excluded_dir(&path) {
                scan_lnk_dir(&path, filter, out);
            }
            continue;
        }
        let is_lnk = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
        if !is_lnk || filter.is_excluded_file(&path) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let target = path.to_string_lossy().into_owned();
        let keywords = synonym_keywords(stem);
        out.push(IndexItem {
            id: format!("app:{target}"),
            title: stem.to_string(),
            subtitle: "Application".to_string(),
            keywords,
            kind: ItemKind::App,
            target: target.clone(),
            icon_key: Some(format!("app_{stem}")),
        });
    }
}

/// Enumerates `shell:AppsFolder` store/packaged apps on Windows.
pub fn scan_apps_folder() -> Vec<IndexItem> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_folder_scans_top_level_and_skips_excluded() {
        let root = std::env::temp_dir().join(format!("launchify_scan_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::create_dir_all(root.join("projects").join("nested_dir")).unwrap();
        fs::write(root.join("notes.txt"), "hello").unwrap();
        fs::write(root.join("photo.png"), "img").unwrap();
        fs::write(root.join("projects").join("nested.txt"), "nested").unwrap();

        let filter = ExclusionFilter::default();
        let items = scan_curated_folders(std::slice::from_ref(&root), &filter);

        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"projects"));
        assert!(titles.contains(&"notes.txt"));
        assert!(!titles.contains(&"node_modules"));
        assert!(!titles.contains(&"photo.png"));
        assert!(!titles.contains(&"nested.txt"));
        assert!(!titles.contains(&"nested_dir"));

        let _ = fs::remove_dir_all(&root);
    }
}
