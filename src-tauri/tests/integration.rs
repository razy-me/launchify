use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use launchify_lib::catalog::{builtin_catalog, config_items};
use launchify_lib::config::{
    load_or_create, save_config_atomic, ExclusionFilter, LaunchifyConfig, WebShortcutConfig,
};
use launchify_lib::frecency::{
    frecency_score, normalize_prefix, prefix_memory_boost, record_selection,
};
use launchify_lib::inline_tools::evaluate;
use launchify_lib::model::{
    FrecencyStore, IndexItem, IndexSnapshot, ItemKind, INDEX_SNAPSHOT_VERSION,
};
use launchify_lib::path_nav::{browse_path, complete_path};
use launchify_lib::search::engine::SearchEngine;
use launchify_lib::storage::{
    load_frecency, load_index, save_frecency_atomic, save_index_atomic,
};

fn temp_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("launchify_int_{name}_{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn integration_search_frecency_and_prefix_memory() {
    let items = vec![
        IndexItem {
            id: "app:code".into(),
            kind: ItemKind::App,
            title: "Visual Studio Code".into(),
            subtitle: "Code Editor".into(),
            target: r"C:\Apps\Code.exe".into(),
            keywords: vec!["vscode".into(), "editor".into()],
            icon_key: None,
        },
        IndexItem {
            id: "app:cmd".into(),
            kind: ItemKind::App,
            title: "Command Prompt".into(),
            subtitle: "Terminal".into(),
            target: r"C:\Windows\System32\cmd.exe".into(),
            keywords: vec!["terminal".into(), "shell".into()],
            icon_key: None,
        },
    ];
    let snap = IndexSnapshot {
        version: INDEX_SNAPSHOT_VERSION,
        built_at_unix: 1_700_000_000,
        items,
    };
    let engine = SearchEngine::new(snap);
    let mut store = FrecencyStore::default();
    let now = 1_700_000_100;

    // Record 2 selections for "app:cmd" under prefix "co" (threshold = 2)
    let prefix = normalize_prefix("co");
    record_selection(&mut store, &prefix, "app:cmd", now);
    record_selection(&mut store, &prefix, "app:cmd", now + 1);

    assert!(frecency_score(store.entries.get("app:cmd").unwrap(), now + 2) > 0.0);
    assert!(prefix_memory_boost(&store, &prefix, "app:cmd", 2) > 0.0);

    let results = engine.search("co", &store, 8, 2, now + 2);
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "app:cmd");
}

#[test]
fn integration_catalog_config_and_search() {
    let mut cfg = LaunchifyConfig::default();
    cfg.web_shortcuts.push(WebShortcutConfig {
        keyword: "gh".into(),
        name: "GitHub".into(),
        url_template: "https://github.com/search?q={query}".into(),
    });
    let mut all_items = builtin_catalog();
    all_items.extend(config_items(&cfg));

    let snap = IndexSnapshot {
        version: INDEX_SNAPSHOT_VERSION,
        built_at_unix: 1_700_000_000,
        items: all_items,
    };
    let engine = SearchEngine::new(snap);
    let store = FrecencyStore::default();

    let results = engine.search("gh", &store, 8, 2, 1_700_000_000);
    assert!(results.iter().any(|r| r.kind == ItemKind::WebShortcut));

    let settings = engine.search("display", &store, 8, 2, 1_700_000_000);
    assert!(settings.iter().any(|r| r.kind == ItemKind::SystemSetting));
}

#[test]
fn integration_inline_tools_with_config_rates() {
    let mut rates = HashMap::new();
    rates.insert("USD".into(), 1.0);
    rates.insert("EUR".into(), 0.9);

    let math_preview = evaluate("2 + 3 * 4", false, &rates).expect("math preview");
    assert_eq!(math_preview.display, "14");

    let forced = evaluate("42", true, &rates).expect("forced calc");
    assert_eq!(forced.display, "42");

    let conv = evaluate("100 usd to eur", false, &rates).expect("currency conversion");
    assert!(conv.display.contains("90"));
}

#[test]
fn integration_path_nav_with_exclusions() {
    let dir = temp_test_dir("path_nav");
    let visible_dir = dir.join("Projects");
    let hidden_dir = dir.join("node_modules");
    fs::create_dir_all(&visible_dir).unwrap();
    fs::create_dir_all(&hidden_dir).unwrap();
    fs::write(dir.join("notes.txt"), "hello").unwrap();

    let exclusions = ExclusionFilter::new(&[]);
    let query = format!("{}\\", dir.to_string_lossy());
    let results = browse_path(&query, &exclusions, 10);

    assert!(results.iter().any(|r| r.title == "Projects"));
    assert!(results.iter().any(|r| r.title == "notes.txt"));
    assert!(!results.iter().any(|r| r.title == "node_modules"));

    let partial = format!("{}\\Pro", dir.to_string_lossy());
    let completed = complete_path(&partial, &exclusions);
    assert!(completed.is_some());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn integration_storage_and_config_roundtrip() {
    let dir = temp_test_dir("storage");
    let mut cfg = load_or_create(&dir).expect("load_or_create");
    cfg.max_results = 6;
    save_config_atomic(&dir, &cfg).expect("save_config_atomic");
    let reloaded = load_or_create(&dir).expect("reload");
    assert_eq!(reloaded.max_results, 6);

    let snap = IndexSnapshot {
        version: INDEX_SNAPSHOT_VERSION,
        built_at_unix: 1_700_000_000,
        items: vec![],
    };
    let idx_path = dir.join("index.bin");
    save_index_atomic(&idx_path, &snap).expect("save_index_atomic");
    let loaded_snap = load_index(&idx_path).expect("load_index");
    assert_eq!(loaded_snap.version, INDEX_SNAPSHOT_VERSION);

    let mut frecency = FrecencyStore::default();
    record_selection(&mut frecency, "vs", "app:code", 1_700_000_000);
    let frec_path = dir.join("frecency.json");
    save_frecency_atomic(&frec_path, &frecency).expect("save_frecency_atomic");
    let loaded_frec = load_frecency(&frec_path);
    assert!(loaded_frec.entries.contains_key("app:code"));

    let _ = fs::remove_dir_all(&dir);
}
