pub mod math;
pub mod units;
pub mod utils;

use std::collections::HashMap;

use crate::model::InlinePreview;

/// Evaluates an inline tool query (math, units, offline currency, Hex/RGB, timestamps, UUID).
///
/// When `force_calc` is `true` (e.g. `=` prefix), any valid expression is evaluated.
/// When `force_calc` is `false`, only unambiguous inline patterns are auto-detected.
pub fn evaluate(
    query: &str,
    force_calc: bool,
    rates: &HashMap<String, f64>,
) -> Option<InlinePreview> {
    let trimmed = query.trim();
    let (expr, forced) = if let Some(rest) = trimmed.strip_prefix('=') {
        (rest.trim(), true)
    } else {
        (trimmed, force_calc)
    };

    if expr.is_empty() {
        return None;
    }

    if let Some((display, copy_value, kind_label)) = utils::try_eval_util(expr) {
        return Some(InlinePreview {
            display,
            copy_value,
            kind_label,
        });
    }

    if let Some((display, copy_value, kind_label)) = units::try_convert(expr, rates) {
        return Some(InlinePreview {
            display,
            copy_value,
            kind_label,
        });
    }

    if forced || math::looks_like_math_expr(expr) {
        if let Ok(val) = math::eval_math(expr) {
            let formatted = math::format_number(val);
            return Some(InlinePreview {
                display: formatted.clone(),
                copy_value: formatted,
                kind_label: "Calculator".to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detect_and_forced_calc() {
        let rates = HashMap::new();

        let res = evaluate("1920 * 0.75", false, &rates).unwrap();
        assert_eq!(res.display, "1440");
        assert_eq!(res.kind_label, "Calculator");

        let res2 = evaluate("=42", false, &rates).unwrap();
        assert_eq!(res2.display, "42");

        // Plain word or bare number without '=' should not auto-trigger calculator.
        assert!(evaluate("spotify", false, &rates).is_none());
        assert!(evaluate("42", false, &rates).is_none());
    }
}
