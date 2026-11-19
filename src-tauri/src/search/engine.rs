use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::frecency::{frecency_score, prefix_memory_boost};
use crate::model::{FrecencyStore, IndexItem, IndexSnapshot, SearchResult};
use crate::search::ranking::{secondary_hint_for_kind, title_match_bonus};

/// Lock-free fuzzy search engine backed by `ArcSwap<IndexSnapshot>` and `nucleo-matcher`.
#[derive(Debug, Clone)]
pub struct SearchEngine {
    index: Arc<ArcSwap<IndexSnapshot>>,
}

impl SearchEngine {
    /// Create a new `SearchEngine` initialized with `snapshot`.
    pub fn new(snapshot: IndexSnapshot) -> Self {
        Self {
            index: Arc::new(ArcSwap::from_pointee(snapshot)),
        }
    }

    /// Create a `SearchEngine` sharing an existing `Arc<ArcSwap<IndexSnapshot>>`.
    pub fn from_arc(index: Arc<ArcSwap<IndexSnapshot>>) -> Self {
        Self { index }
    }

    /// Atomically swap in a new `IndexSnapshot` without blocking readers.
    pub fn update_index(&self, snapshot: IndexSnapshot) {
        self.index.store(Arc::new(snapshot));
    }

    /// Return a handle to the current `IndexSnapshot`.
    pub fn snapshot(&self) -> Arc<IndexSnapshot> {
        self.index.load_full()
    }

    /// Search the current snapshot for `query`, returning up to `max_results` ranked items.
    pub fn search(
        &self,
        query: &str,
        frecency: &FrecencyStore,
        max_results: usize,
        prefix_threshold: u32,
        now_unix: i64,
    ) -> Vec<SearchResult> {
        if max_results == 0 {
            return Vec::new();
        }

        let snap = self.index.load();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            return self.top_frecency_items(&snap.items, frecency, max_results, now_unix);
        }

        let pattern = Pattern::parse(trimmed, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut buf: Vec<char> = Vec::with_capacity(128);

        let mut scored: Vec<(&IndexItem, f64)> = Vec::new();

        for item in &snap.items {
            let title_fuzzy = pattern.score(Utf32Str::new(&item.title, &mut buf), &mut matcher);

            let kw_fuzzy = if title_fuzzy.is_none() {
                item.keywords
                    .iter()
                    .filter_map(|kw| pattern.score(Utf32Str::new(kw, &mut buf), &mut matcher))
                    .max()
            } else {
                None
            };

            let sub_fuzzy = if title_fuzzy.is_none() && kw_fuzzy.is_none() {
                pattern.score(Utf32Str::new(&item.subtitle, &mut buf), &mut matcher)
            } else {
                None
            };

            let base_fuzzy = if let Some(s) = title_fuzzy {
                s as f64
            } else if let Some(s) = kw_fuzzy {
                (s as f64) * 0.75
            } else if let Some(s) = sub_fuzzy {
                (s as f64) * 0.5
            } else {
                continue;
            };

            let bonus = title_match_bonus(&item.title, trimmed);
            let frec = frecency
                .entries
                .get(&item.id)
                .map(|e| frecency_score(e, now_unix))
                .unwrap_or(0.0);
            let pm_boost = prefix_memory_boost(frecency, trimmed, &item.id, prefix_threshold);

            let total_score = base_fuzzy + bonus + frec + pm_boost;
            scored.push((item, total_score));
        }

        if scored.len() > max_results {
            scored.select_nth_unstable_by(max_results, |a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.0.title.cmp(&b.0.title))
            });
            scored.truncate(max_results);
        }

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.title.cmp(&b.0.title))
        });

        scored
            .into_iter()
            .map(|(item, score)| to_search_result(item, score))
            .collect()
    }

    fn top_frecency_items(
        &self,
        items: &[IndexItem],
        frecency: &FrecencyStore,
        max_results: usize,
        now_unix: i64,
    ) -> Vec<SearchResult> {
        let mut scored: Vec<(&IndexItem, f64)> = items
            .iter()
            .map(|item| {
                let frec = frecency
                    .entries
                    .get(&item.id)
                    .map(|e| frecency_score(e, now_unix))
                    .unwrap_or(0.0);
                (item, frec)
            })
            .collect();

        if scored.len() > max_results {
            scored.select_nth_unstable_by(max_results, |a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.0.title.cmp(&b.0.title))
            });
            scored.truncate(max_results);
        }

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.title.cmp(&b.0.title))
        });

        scored
            .into_iter()
            .map(|(item, score)| to_search_result(item, score))
            .collect()
    }
}

fn to_search_result(item: &IndexItem, score: f64) -> SearchResult {
    SearchResult {
        id: item.id.clone(),
        title: item.title.clone(),
        subtitle: item.subtitle.clone(),
        kind: item.kind,
        icon_path: item.icon_key.clone(),
        secondary_hint: secondary_hint_for_kind(item.kind),
        score,
        autocomplete_text: Some(item.title.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frecency::record_selection;
    use crate::model::{ItemKind, INDEX_SNAPSHOT_VERSION};
    use std::time::Instant;

    fn make_item(id: &str, title: &str) -> IndexItem {
        IndexItem {
            id: id.to_string(),
            title: title.to_string(),
            subtitle: format!("C:\\Apps\\{id}.exe"),
            keywords: Vec::new(),
            kind: ItemKind::App,
            target: format!("C:\\Apps\\{id}.exe"),
            icon_key: None,
        }
    }

    #[test]
    fn word_boundary_ranks_vscode_above_scattered_matches() {
        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 100,
            items: vec![
                make_item("canvas", "Canvas Settings"),
                make_item("vscode", "Visual Studio Code"),
                make_item("vstudio", "Video Studio"),
            ],
        };
        let engine = SearchEngine::new(snap);
        let frec = FrecencyStore::default();

        let results = engine.search("vs", &frec, 8, 3, 1_000);
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "vscode");
    }

    #[test]
    fn exact_substring_preferred_over_fuzzy() {
        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 100,
            items: vec![
                make_item("fuzzy", "Not Even Close Pad"),
                make_item("notepad", "Notepad"),
            ],
        };
        let engine = SearchEngine::new(snap);
        let frec = FrecencyStore::default();

        let results = engine.search("note", &frec, 8, 3, 1_000);
        assert_eq!(results[0].id, "notepad");
    }

    #[test]
    fn prefix_memory_boosts_item_to_first_after_threshold() {
        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 100,
            items: vec![
                make_item("code", "Visual Studio Code"),
                make_item("vim", "Vim Editor"),
            ],
        };
        let engine = SearchEngine::new(snap);
        let mut frec = FrecencyStore::default();

        // Initially "Visual Studio Code" should win for "v" or "vi"
        for _ in 0..3 {
            record_selection(&mut frec, "v", "vim", 1_000);
        }

        let results = engine.search("v", &frec, 8, 3, 1_000);
        assert_eq!(results[0].id, "vim");
    }

    #[test]
    fn search_2000_items_under_2ms() {
        let mut items = Vec::with_capacity(2_000);
        for i in 0..2_000 {
            items.push(make_item(&format!("app_{i}"), &format!("Application Number {i}")));
        }
        items[1_500].title = "Visual Studio Code".to_string();

        let snap = IndexSnapshot {
            version: INDEX_SNAPSHOT_VERSION,
            built_at_unix: 100,
            items,
        };
        let engine = SearchEngine::new(snap);
        let frec = FrecencyStore::default();

        // Warm up
        let _ = engine.search("visual", &frec, 8, 3, 1_000);

        let start = Instant::now();
        let results = engine.search("visual", &frec, 8, 3, 1_000);
        let elapsed = start.elapsed();

        assert_eq!(results[0].title, "Visual Studio Code");
        assert!(
            elapsed.as_secs_f64() * 1000.0 < 2.0,
            "search took {:?}, expected < 2ms",
            elapsed
        );
    }
}
