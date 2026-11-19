use crate::model::{FrecencyEntry, FrecencyStore};

/// Half-life for frecency time decay (7 days in seconds).
const FRECENCY_HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 60.0 * 60.0;

/// Base weight applied to the frequency-decay product.
const FRECENCY_WEIGHT: f64 = 120.0;

/// Strong score boost applied when a `(prefix, item_id)` pair reaches `threshold`.
const PREFIX_MEMORY_BASE_BOOST: f64 = 1_000_000.0;

/// Normalize a query prefix for consistent prefix-memory lookup.
pub fn normalize_prefix(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Record a user selection of `item_id` for `query_prefix` at `now_unix`.
pub fn record_selection(
    store: &mut FrecencyStore,
    query_prefix: &str,
    item_id: &str,
    now_unix: i64,
) {
    let entry = store
        .entries
        .entry(item_id.to_string())
        .or_insert(FrecencyEntry {
            count: 0,
            last_used_unix: now_unix,
        });
    entry.count = entry.count.saturating_add(1);
    entry.last_used_unix = now_unix;

    let norm_prefix = normalize_prefix(query_prefix);
    if !norm_prefix.is_empty() {
        let prefix_map = store.prefix_memory.entry(norm_prefix).or_default();
        let count = prefix_map.entry(item_id.to_string()).or_insert(0);
        *count = count.saturating_add(1);
    }
}

/// Compute time-decayed frecency score for an entry at `now_unix`.
pub fn frecency_score(entry: &FrecencyEntry, now_unix: i64) -> f64 {
    if entry.count == 0 {
        return 0.0;
    }
    let age_secs = (now_unix.saturating_sub(entry.last_used_unix)).max(0) as f64;
    let decay = 0.5_f64.powf(age_secs / FRECENCY_HALF_LIFE_SECS);
    (entry.count as f64).ln_1p() * FRECENCY_WEIGHT * decay
}

/// Compute the prefix-memory boost for `(query_prefix, item_id)` if it meets `threshold`.
pub fn prefix_memory_boost(
    store: &FrecencyStore,
    query_prefix: &str,
    item_id: &str,
    threshold: u32,
) -> f64 {
    if threshold == 0 {
        return 0.0;
    }
    let norm_prefix = normalize_prefix(query_prefix);
    if norm_prefix.is_empty() {
        return 0.0;
    }
    let count = store
        .prefix_memory
        .get(&norm_prefix)
        .and_then(|m| m.get(item_id))
        .copied()
        .unwrap_or(0);

    if count >= threshold {
        PREFIX_MEMORY_BASE_BOOST + (count as f64) * 100.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_selection_updates_frecency_and_prefix_memory() {
        let mut store = FrecencyStore::default();
        record_selection(&mut store, "  VS ", "vscode", 1_000);
        record_selection(&mut store, "vs", "vscode", 2_000);

        let entry = store.entries.get("vscode").expect("entry should exist");
        assert_eq!(entry.count, 2);
        assert_eq!(entry.last_used_unix, 2_000);

        let pm_count = store
            .prefix_memory
            .get("vs")
            .and_then(|m| m.get("vscode"))
            .copied()
            .unwrap_or(0);
        assert_eq!(pm_count, 2);
    }

    #[test]
    fn frecency_decays_over_time() {
        let entry = FrecencyEntry {
            count: 5,
            last_used_unix: 1_000_000,
        };
        let fresh = frecency_score(&entry, 1_000_000);
        let one_week_later = frecency_score(&entry, 1_000_000 + 7 * 24 * 60 * 60);
        let two_weeks_later = frecency_score(&entry, 1_000_000 + 14 * 24 * 60 * 60);

        assert!(fresh > 0.0);
        assert!((one_week_later - fresh * 0.5).abs() < 1e-6);
        assert!((two_weeks_later - fresh * 0.25).abs() < 1e-6);
    }

    #[test]
    fn prefix_memory_boost_activates_at_threshold() {
        let mut store = FrecencyStore::default();
        record_selection(&mut store, "term", "wt", 100);
        record_selection(&mut store, "term", "wt", 200);
        assert_eq!(prefix_memory_boost(&store, "term", "wt", 3), 0.0);

        record_selection(&mut store, "term", "wt", 300);
        let boost = prefix_memory_boost(&store, "term", "wt", 3);
        assert!(boost >= PREFIX_MEMORY_BASE_BOOST);
    }
}
