use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::storage::write_atomic;

pub const DEFAULT_MAX_RESULTS: usize = 8;
pub const DEFAULT_PREFIX_MEMORY_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub name: String,
    pub keyword: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub run_in_terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebShortcutConfig {
    pub keyword: String,
    pub name: String,
    pub url_template: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchifyConfig {
    pub curated_folders: Vec<PathBuf>,
    pub custom_workflows: Vec<WorkflowConfig>,
    pub web_shortcuts: Vec<WebShortcutConfig>,
    pub currency_rates: HashMap<String, f64>,
    pub extra_exclusions: Vec<String>,
    pub prefix_memory_threshold: u32,
    pub max_results: usize,
}

impl Default for LaunchifyConfig {
    fn default() -> Self {
        let mut currency_rates = HashMap::new();
        currency_rates.insert("USD".into(), 1.0);
        currency_rates.insert("EUR".into(), 0.92);
        currency_rates.insert("GBP".into(), 0.79);
        currency_rates.insert("JPY".into(), 152.0);
        currency_rates.insert("CHF".into(), 0.88);
        currency_rates.insert("CAD".into(), 1.39);
        currency_rates.insert("AUD".into(), 1.53);

        let web_shortcuts = vec![
            WebShortcutConfig {
                keyword: "g".into(),
                name: "Google".into(),
                url_template: "https://www.google.com/search?q={query}".into(),
            },
            WebShortcutConfig {
                keyword: "yt".into(),
                name: "YouTube".into(),
                url_template: "https://www.youtube.com/results?search_query={query}".into(),
            },
            WebShortcutConfig {
                keyword: "gh".into(),
                name: "GitHub".into(),
                url_template: "https://github.com/search?q={query}".into(),
            },
            WebShortcutConfig {
                keyword: "ddg".into(),
                name: "DuckDuckGo".into(),
                url_template: "https://duckduckgo.com/?q={query}".into(),
            },
            WebShortcutConfig {
                keyword: "wiki".into(),
                name: "Wikipedia".into(),
                url_template: "https://en.wikipedia.org/wiki/Special:Search?search={query}".into(),
            },
        ];

        Self {
            curated_folders: Vec::new(),
            custom_workflows: Vec::new(),
            web_shortcuts,
            currency_rates,
            extra_exclusions: Vec::new(),
            prefix_memory_threshold: DEFAULT_PREFIX_MEMORY_THRESHOLD,
            max_results: DEFAULT_MAX_RESULTS,
        }
    }
}

/// Clamps and normalizes configuration values into safe bounds.
pub fn validate(mut cfg: LaunchifyConfig) -> LaunchifyConfig {
    cfg.max_results = cfg.max_results.clamp(1, 8);
    if cfg.prefix_memory_threshold < 1 {
        cfg.prefix_memory_threshold = 1;
    }
    cfg.currency_rates
        .retain(|k, v| !k.trim().is_empty() && v.is_finite() && *v > 0.0);
    cfg
}

/// Loads `config.json` from `app_dir`, creating a default config file if missing.
pub fn load_or_create(app_dir: &Path) -> Result<LaunchifyConfig, String> {
    fs::create_dir_all(app_dir)
        .map_err(|e| format!("failed to create app dir {}: {e}", app_dir.display()))?;

    let config_path = app_dir.join("config.json");
    if !config_path.exists() {
        let default_cfg = validate(LaunchifyConfig::default());
        save_config_atomic(app_dir, &default_cfg)?;
        return Ok(default_cfg);
    }

    let raw = fs::read_to_string(&config_path)
        .map_err(|e| format!("failed to read {}: {e}", config_path.display()))?;
    let parsed: LaunchifyConfig = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse {}: {e}", config_path.display()))?;

    Ok(validate(parsed))
}

/// Atomically writes `config.json` inside `app_dir`.
pub fn save_config_atomic(app_dir: &Path, cfg: &LaunchifyConfig) -> Result<(), String> {
    let config_path = app_dir.join("config.json");
    let json = serde_json::to_vec_pretty(cfg)
        .map_err(|e| format!("failed to serialize config: {e}"))?;
    write_atomic(&config_path, &json)
}

/// Hard-coded + user-configured exclusion filter for indexing and `/` path navigation.
#[derive(Debug, Clone, Default)]
pub struct ExclusionFilter {
    extra_exclusions: Vec<String>,
}

const EXCLUDED_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "__pycache__",
    "$recycle.bin",
    "system volume information",
    ".cache",
];

const EXCLUDED_MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "svg", "mp3", "wav", "flac", "aac", "ogg",
    "m4a", "mp4", "mkv", "avi", "mov", "wmv", "webm",
];

impl ExclusionFilter {
    pub fn new(extra_exclusions: &[String]) -> Self {
        let normalized = extra_exclusions
            .iter()
            .map(|s| s.trim().to_ascii_lowercase().replace('/', "\\"))
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            extra_exclusions: normalized,
        }
    }

    /// Returns `true` if `path` is a hard-excluded directory or matches `extra_exclusions`.
    pub fn is_excluded_dir(&self, path: &Path) -> bool {
        let norm = normalize_path(path);

        if norm == "c:\\windows"
            || norm.starts_with("c:\\windows\\")
            || norm == "c:\\program files"
            || norm.starts_with("c:\\program files\\")
            || norm == "c:\\program files (x86)"
            || norm.starts_with("c:\\program files (x86)\\")
        {
            return true;
        }

        // AppData is excluded except for Start Menu shortcuts.
        if let Some(appdata_idx) = norm.find("\\appdata\\") {
            let after_appdata = &norm[appdata_idx + "\\appdata\\".len()..];
            let is_start_menu = after_appdata == "roaming\\microsoft\\windows\\start menu"
                || after_appdata.starts_with("roaming\\microsoft\\windows\\start menu\\");
            if !is_start_menu {
                return true;
            }
        } else if norm.ends_with("\\appdata") {
            return true;
        }

        for component in norm.split('\\') {
            if EXCLUDED_DIR_NAMES.contains(&component) {
                return true;
            }
        }

        self.matches_extra(&norm)
    }

    /// Returns `true` if `path` is a media file or resides in an excluded directory.
    pub fn is_excluded_file(&self, path: &Path) -> bool {
        if let Some(parent) = path.parent() {
            if self.is_excluded_dir(parent) {
                return true;
            }
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lower = ext.to_ascii_lowercase();
            if EXCLUDED_MEDIA_EXTENSIONS.contains(&lower.as_str()) {
                return true;
            }
        }

        let norm = normalize_path(path);
        self.matches_extra(&norm)
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        self.is_excluded_dir(path) || self.is_excluded_file(path)
    }

    fn matches_extra(&self, norm_path: &str) -> bool {
        self.extra_exclusions
            .iter()
            .any(|pat| norm_path.contains(pat.as_str()))
    }
}

fn normalize_path(path: &Path) -> String {
    let s = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    s.strip_prefix("\\\\?\\").unwrap_or(&s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_clamps_bounds() {
        let mut cfg = LaunchifyConfig {
            max_results: 99,
            prefix_memory_threshold: 0,
            ..Default::default()
        };
        cfg.currency_rates.insert("BAD".into(), -5.0);
        cfg.currency_rates.insert("".into(), 1.0);

        let validated = validate(cfg);
        assert_eq!(validated.max_results, 8);
        assert_eq!(validated.prefix_memory_threshold, 1);
        assert!(!validated.currency_rates.contains_key("BAD"));
        assert!(!validated.currency_rates.contains_key(""));

        let min_cfg = validate(LaunchifyConfig {
            max_results: 0,
            ..Default::default()
        });
        assert_eq!(min_cfg.max_results, 1);
    }

    #[test]
    fn load_or_create_writes_default_and_reloads() {
        let dir = std::env::temp_dir().join(format!(
            "launchify_config_test_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);

        let created = load_or_create(&dir).unwrap();
        assert_eq!(created.max_results, 8);
        assert_eq!(created.prefix_memory_threshold, 3);
        assert!(dir.join("config.json").exists());

        let reloaded = load_or_create(&dir).unwrap();
        assert_eq!(reloaded, created);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exclusion_filter_blocks_system_build_and_media() {
        let filter = ExclusionFilter::new(&["secret_vault".into()]);

        // Hard-excluded system & build dirs
        assert!(filter.is_excluded_dir(Path::new(r"C:\Windows\System32")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Program Files\Git")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Program Files (x86)\Foo")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Users\Razy\AppData\Local\Temp")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Projects\app\node_modules\lodash")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Projects\app\.git\objects")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Projects\app\target\debug")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Projects\app\dist")));
        assert!(filter.is_excluded_dir(Path::new(r"C:\Projects\app\__pycache__")));

        // AppData Start Menu exception
        assert!(!filter.is_excluded_dir(Path::new(
            r"C:\Users\Razy\AppData\Roaming\Microsoft\Windows\Start Menu\Programs"
        )));

        // Extra exclusions
        assert!(filter.is_excluded_dir(Path::new(r"C:\Users\Razy\secret_vault\keys")));

        // Media extensions
        assert!(filter.is_excluded_file(Path::new(r"C:\Users\Razy\Projects\logo.png")));
        assert!(filter.is_excluded_file(Path::new(r"C:\Users\Razy\Projects\demo.mp4")));
        assert!(filter.is_excluded_file(Path::new(r"C:\Users\Razy\Projects\track.mp3")));

        // Allowed project file
        assert!(!filter.is_excluded_file(Path::new(r"C:\Users\Razy\Projects\README.md")));
    }
}
