use crate::model::ItemKind;

/// Compute deterministic bonus for exact match, prefix match, word-boundary acronym/start,
/// or exact substring match on `title`.
pub fn title_match_bonus(title: &str, query: &str) -> f64 {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 0.0;
    }
    let t = title.to_lowercase();

    if t == q {
        return 5_000.0;
    }
    if t.starts_with(&q) {
        return 2_500.0;
    }
    if is_word_boundary_match(&t, &q) {
        return 1_500.0;
    }
    if t.contains(&q) {
        return 800.0;
    }
    0.0
}

/// Returns true if `query` (lowercased) matches the leading characters of consecutive words
/// in `title` (lowercased), e.g., `"vs"` matching `"visual studio code"`.
fn is_word_boundary_match(title_lower: &str, query_lower: &str) -> bool {
    let q_chars: Vec<char> = query_lower.chars().collect();
    if q_chars.is_empty() {
        return false;
    }

    let word_starts: Vec<char> = title_lower
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|w| w.chars().next())
        .collect();

    if word_starts.len() < q_chars.len() {
        return false;
    }

    // Check if query chars match a contiguous or ordered subsequence of word-start chars.
    let mut qi = 0;
    for wc in word_starts {
        if wc == q_chars[qi] {
            qi += 1;
            if qi == q_chars.len() {
                return true;
            }
        }
    }
    false
}

/// Optimal String Alignment (restricted Damerau-Levenshtein) distance between two strings (case-insensitive).
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().map(|c| c.to_ascii_lowercase()).collect();
    let b_chars: Vec<char> = b.chars().map(|c| c.to_ascii_lowercase()).collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=n {
        for j in 1..=m {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            let mut val = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1
                && j > 1
                && a_chars[i - 1] == b_chars[j - 2]
                && a_chars[i - 2] == b_chars[j - 1]
            {
                val = val.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = val;
        }
    }
    d[n][m]
}

/// Returns a fallback fuzzy score when `query` has typos/transpositions relative to `title`
/// (e.g. `"Dsicord"` -> `"Discord"`). Returns `None` for queries shorter than 3 chars.
pub fn typo_fuzzy_score(query: &str, title: &str) -> Option<f64> {
    let q = query.trim();
    let q_len = q.chars().count();
    if q_len < 3 {
        return None;
    }

    let max_edits = if q_len <= 5 { 1 } else { 2 };
    let mut best_dist: Option<usize> = None;

    let title_len = title.chars().count();
    if title_len.abs_diff(q_len) <= max_edits {
        let d_full = damerau_levenshtein(q, title);
        if d_full <= max_edits {
            best_dist = Some(d_full);
        }
    }

    for word in title.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let w_len = word.chars().count();
        if w_len.abs_diff(q_len) > max_edits {
            continue;
        }
        let d_word = damerau_levenshtein(q, word);
        if d_word <= max_edits {
            best_dist = Some(best_dist.map_or(d_word, |prev| prev.min(d_word)));
        }
    }

    best_dist.map(|dist| match dist {
        0 => 400.0,
        1 => 250.0,
        _ => 120.0,
    })
}

/// Return a short secondary action hint for the item kind, if applicable.
pub fn secondary_hint_for_kind(kind: ItemKind) -> Option<String> {
    match kind {
        ItemKind::App | ItemKind::StoreApp => None,
        ItemKind::File | ItemKind::Folder => Some("Open containing folder".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_prefix_and_word_boundary_bonuses() {
        assert!(title_match_bonus("Visual Studio Code", "visual studio code") > 4_000.0);
        assert!(title_match_bonus("Visual Studio Code", "visual") > 2_000.0);
        assert!(title_match_bonus("Visual Studio Code", "vs") >= 1_500.0);
        assert!(title_match_bonus("Visual Studio Code", "studio") >= 800.0);
        assert_eq!(title_match_bonus("Canvas Settings", "vs"), 0.0);
    }

    #[test]
    fn typo_fuzzy_matches_transpositions_and_typos() {
        assert!(typo_fuzzy_score("Dsicord", "Discord").is_some());
        assert!(typo_fuzzy_score("Vsiual", "Visual Studio Code").is_some());
        assert!(typo_fuzzy_score("xyz", "Discord").is_none());
    }

    #[test]
    fn typo_fuzzy_skips_large_length_mismatch() {
        let long_query = "a".repeat(1000);
        assert!(typo_fuzzy_score(&long_query, "Discord").is_none());
        let long_title = "b".repeat(1000);
        assert!(typo_fuzzy_score("abc", &long_title).is_none());
    }
}
