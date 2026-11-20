use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::model::{ItemKind, SearchResult};
use crate::search::ranking::{title_match_bonus, typo_fuzzy_score};

const MAX_SCAN_DEPTH: usize = 4;
const MAX_COLLECTED_FILES: usize = 4_000;

/// Returns default user directories to scan for recent files (`Desktop`, `Downloads`, `Documents`, plus `extra_roots`).
pub fn default_find_roots(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home_path = PathBuf::from(home);
        for sub in ["Desktop", "Downloads", "Documents"] {
            let p = home_path.join(sub);
            if p.is_dir() && seen.insert(p.clone()) {
                roots.push(p);
            }
        }
    }

    for extra_path in ["C:\\Tools"] {
        let p = PathBuf::from(extra_path);
        if p.is_dir() && seen.insert(p.clone()) {
            roots.push(p);
        }
    }

    for extra in extra_roots {
        if extra.is_dir() && seen.insert(extra.clone()) {
            roots.push(extra.clone());
        }
    }

    roots
}

/// Searches recently modified/created files under `roots`.
/// - Empty query: returns the most recent files.
/// - `.ext` or `*.ext`: filters by file extension, sorted by recency.
/// - Text query: fuzzy/typo matches on file name, ranked by match quality + recency.
pub fn search_recent_files(query: &str, roots: &[PathBuf], limit: usize) -> Vec<SearchResult> {
    if limit == 0 {
        return Vec::new();
    }

    let mut files: Vec<(PathBuf, String, u64)> = Vec::new();
    let mut visited_dirs = HashSet::new();

    for root in roots {
        if files.len() >= MAX_COLLECTED_FILES {
            break;
        }
        collect_files_rec(root, 0, &mut visited_dirs, &mut files);
    }

    let q = query.trim();
    let ext_filter = q
        .strip_prefix("*.")
        .or_else(|| q.strip_prefix('.'))
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace));

    let mut scored: Vec<(PathBuf, String, u64, f64)> = Vec::new();

    for (path, name, ts) in files {
        if let Some(ref ext) = ext_filter {
            let file_ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if file_ext.as_deref() != Some(ext.as_str()) {
                continue;
            }
            scored.push((path, name, ts, ts as f64));
        } else if q.is_empty() {
            scored.push((path, name, ts, ts as f64));
        } else {
            let q_lower = q.to_ascii_lowercase();
            let name_lower = name.to_ascii_lowercase();
            let bonus = title_match_bonus(&name, q);
            let fuzzy = if bonus > 0.0 {
                bonus
            } else if name_lower.contains(&q_lower) {
                600.0
            } else if let Some(t_score) = typo_fuzzy_score(q, &name) {
                t_score
            } else {
                continue;
            };
            // Combine match relevance with mild recency tie-breaker
            let recency_bonus = (ts as f64) * 1e-7;
            scored.push((path, name, ts, fuzzy + recency_bonus));
        }
    }

    scored.sort_by(|a, b| {
        b.3.partial_cmp(&a.3)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.1.cmp(&b.1))
    });
    scored.truncate(limit);

    scored
        .into_iter()
        .map(|(path, name, _ts, score)| {
            let full = path.to_string_lossy().to_string();
            let parent = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            SearchResult {
                id: format!("file:{full}"),
                title: name,
                subtitle: parent,
                kind: ItemKind::File,
                score,
                icon_path: None,
                secondary_hint: Some("Open containing folder".to_string()),
                autocomplete_text: None,
            }
        })
        .collect()
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | "node_modules" | "$recycle.bin" | "appdata" | "windows" | ".venv"
    )
}

fn file_timestamp_secs(meta: &std::fs::Metadata) -> u64 {
    let mod_t = meta.modified().unwrap_or(UNIX_EPOCH);
    let cre_t = meta.created().unwrap_or(UNIX_EPOCH);
    let best = if mod_t > cre_t { mod_t } else { cre_t };
    best.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn collect_files_rec(
    dir: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<(PathBuf, String, u64)>,
) {
    if depth > MAX_SCAN_DEPTH || out.len() >= MAX_COLLECTED_FILES {
        return;
    }
    if !visited.insert(dir.to_path_buf()) {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if out.len() >= MAX_COLLECTED_FILES {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if !should_skip_dir(&name) {
                collect_files_rec(&path, depth + 1, visited, out);
            }
        } else if meta.is_file() {
            let ts = file_timestamp_secs(&meta);
            out.push((path, name, ts));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_files_filters_by_extension_and_fuzzy_name() {
        let dir = std::env::temp_dir().join(format!("launchify_find_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app_setup.exe"), b"exe").unwrap();
        std::fs::write(dir.join("notes.txt"), b"txt").unwrap();
        std::fs::write(dir.join("Launchify_Guide.md"), b"md").unwrap();

        let exe_res = search_recent_files(".exe", &[dir.clone()], 8);
        assert_eq!(exe_res.len(), 1);
        assert_eq!(exe_res[0].title, "app_setup.exe");

        let fuzzy_res = search_recent_files("launchify", &[dir.clone()], 8);
        assert_eq!(fuzzy_res.len(), 1);
        assert_eq!(fuzzy_res[0].title, "Launchify_Guide.md");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
