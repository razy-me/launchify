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
}
