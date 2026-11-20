use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ExclusionFilter;
use crate::model::{ItemKind, SearchResult};

/// Normalizes a `/` launcher path input into an OS `PathBuf` and optional filename filter.
fn parse_raw_path(raw: &str) -> (PathBuf, String) {
    let trimmed = raw.trim();

    #[cfg(windows)]
    let normalized = {
        if trimmed == "/" {
            "C:\\".to_string()
        } else if let Some(rest) = trimmed.strip_prefix('/') {
            // Support `/c/Tools` or `/C:/Tools` or `/Tools`
            let bytes = rest.as_bytes();
            if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                rest.replace('/', "\\")
            } else if bytes.len() >= 2
                && bytes[0].is_ascii_alphabetic()
                && (bytes[1] == b'/' || bytes[1] == b'\\')
            {
                let drive = (bytes[0] as char).to_ascii_uppercase();
                let tail = rest[1..].replace('/', "\\");
                format!("{drive}:{tail}")
            } else if bytes.len() == 1 && bytes[0].is_ascii_alphabetic() {
                let drive = (bytes[0] as char).to_ascii_uppercase();
                format!("{drive}:\\")
            } else {
                format!("C:\\{}", rest.replace('/', "\\"))
            }
        } else {
            trimmed.replace('/', "\\")
        }
    };

    #[cfg(not(windows))]
    let normalized = if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    };

    #[cfg(windows)]
    let normalized = if normalized.len() == 2
        && normalized.as_bytes()[0].is_ascii_alphabetic()
        && normalized.as_bytes()[1] == b':'
    {
        format!("{normalized}\\")
    } else {
        normalized
    };

    let path = PathBuf::from(&normalized);
    let ends_with_sep = normalized.ends_with('\\') || normalized.ends_with('/');

    if ends_with_sep || path.is_dir() {
        (path, String::new())
    } else {
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(&normalized));
        let filter = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        (dir, filter)
    }
}

fn format_autocomplete(path: &Path, is_dir: bool) -> String {
    let s = path.to_string_lossy();
    let with_slash = if is_dir && !s.ends_with('\\') && !s.ends_with('/') {
        format!("{s}{}", std::path::MAIN_SEPARATOR)
    } else {
        s.into_owned()
    };

    #[cfg(windows)]
    {
        let bytes = with_slash.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            with_slash
        } else if with_slash.starts_with('/') {
            with_slash
        } else {
            format!("/{with_slash}")
        }
    }
    #[cfg(not(windows))]
    {
        with_slash
    }
}

struct DirEntryInfo {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

fn list_matching_entries(
    dir: &Path,
    filter: &str,
    exclusions: &ExclusionFilter,
) -> Vec<DirEntryInfo> {
    if exclusions.is_excluded_dir(dir) {
        return Vec::new();
    }

    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        if is_dir {
            if exclusions.is_excluded_dir(&path) {
                continue;
            }
        } else if exclusions.is_excluded_file(&path) {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };

        if !filter.is_empty() && !name.to_ascii_lowercase().starts_with(filter) {
            continue;
        }

        entries.push(DirEntryInfo { name, path, is_dir });
    }

    // Directories first, then alphabetical case-insensitive.
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    entries
}

/// Browses the filesystem for `/` path navigation mode, respecting `ExclusionFilter`.
pub fn browse_path(raw: &str, exclusions: &ExclusionFilter, limit: usize) -> Vec<SearchResult> {
    let (dir, filter) = parse_raw_path(raw);
    let entries = list_matching_entries(&dir, &filter, exclusions);

    entries
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(idx, entry)| {
            let full_str = entry.path.to_string_lossy().into_owned();
            let autocomplete = format_autocomplete(&entry.path, entry.is_dir);
            SearchResult {
                id: format!("path:{full_str}"),
                title: entry.name,
                subtitle: full_str,
                kind: if entry.is_dir {
                    ItemKind::Folder
                } else {
                    ItemKind::File
                },
                icon_path: None,
                secondary_hint: Some(if entry.is_dir {
                    "Open Folder".to_string()
                } else {
                    "Copy Path".to_string()
                }),
                score: 1000.0 - idx as f64,
                autocomplete_text: Some(autocomplete),
            }
        })
        .collect()
}

/// Returns the `Tab` completion string for the first matching non-excluded entry.
pub fn complete_path(raw: &str, exclusions: &ExclusionFilter) -> Option<String> {
    let (dir, filter) = parse_raw_path(raw);
    let entries = list_matching_entries(&dir, &filter, exclusions);
    entries
        .first()
        .map(|entry| format_autocomplete(&entry.path, entry.is_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_and_complete_respect_exclusions() {
        let temp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test_tmp")
            .join("launchify_wp02_path_nav_test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join("alpha_dir")).unwrap();
        fs::create_dir_all(temp.join("node_modules")).unwrap();
        fs::write(temp.join("notes.txt"), "hello").unwrap();
        fs::write(temp.join("photo.png"), "png").unwrap();

        let exclusions = ExclusionFilter::new(&[]);
        let raw = format!("/{}{}", temp.to_string_lossy(), std::path::MAIN_SEPARATOR);
        let results = browse_path(&raw, &exclusions, 10);

        let titles: Vec<&str> = results.iter().map(|r| r.title.as_str()).collect();
        assert!(titles.contains(&"alpha_dir"));
        assert!(titles.contains(&"notes.txt"));
        assert!(!titles.contains(&"node_modules"));
        assert!(!titles.contains(&"photo.png"));

        let partial = format!("/{}{}al", temp.to_string_lossy(), std::path::MAIN_SEPARATOR);
        let completed = complete_path(&partial, &exclusions).unwrap();
        assert!(completed.contains("alpha_dir"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[cfg(windows)]
    #[test]
    fn format_autocomplete_windows_drive_path_has_no_leading_slash() {
        let dir_result = format_autocomplete(Path::new(r"C:\Tools\alpha_dir"), true);
        assert_eq!(dir_result, r"C:\Tools\alpha_dir\");

        let file_result = format_autocomplete(Path::new(r"C:\Tools\notes.txt"), false);
        assert_eq!(file_result, r"C:\Tools\notes.txt");
    }

    #[cfg(windows)]
    #[test]
    fn drive_only_input_resolves_to_drive_root() {
        let (path1, filter1) = parse_raw_path("/C:");
        assert_eq!(path1, PathBuf::from(r"C:\"));
        assert!(filter1.is_empty());

        let (path2, filter2) = parse_raw_path("C:");
        assert_eq!(path2, PathBuf::from(r"C:\"));
        assert!(filter2.is_empty());
    }
}
