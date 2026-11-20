use std::collections::HashMap;

use crate::inline_tools::math::format_number;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitCategory {
    Length,
    Mass,
    Temperature,
    Data,
    Time,
}

fn lookup_unit(unit: &str) -> Option<(UnitCategory, f64, &'static str)> {
    match unit.to_ascii_lowercase().as_str() {
        // Length (base: meter)
        "mm" | "millimeter" | "millimeters" => Some((UnitCategory::Length, 0.001, "mm")),
        "cm" | "centimeter" | "centimeters" => Some((UnitCategory::Length, 0.01, "cm")),
        "m" | "meter" | "meters" => Some((UnitCategory::Length, 1.0, "m")),
        "km" | "kilometer" | "kilometers" => Some((UnitCategory::Length, 1000.0, "km")),
        "in" | "inch" | "inches" => Some((UnitCategory::Length, 0.0254, "in")),
        "ft" | "foot" | "feet" => Some((UnitCategory::Length, 0.3048, "ft")),
        "yd" | "yard" | "yards" => Some((UnitCategory::Length, 0.9144, "yd")),
        "mi" | "mile" | "miles" => Some((UnitCategory::Length, 1609.344, "mi")),

        // Mass (base: gram)
        "mg" | "milligram" | "milligrams" => Some((UnitCategory::Mass, 0.001, "mg")),
        "g" | "gram" | "grams" => Some((UnitCategory::Mass, 1.0, "g")),
        "kg" | "kilogram" | "kilograms" => Some((UnitCategory::Mass, 1000.0, "kg")),
        "t" | "tonne" | "tonnes" => Some((UnitCategory::Mass, 1_000_000.0, "t")),
        "oz" | "ounce" | "ounces" => Some((UnitCategory::Mass, 28.349523125, "oz")),
        "lb" | "lbs" | "pound" | "pounds" => Some((UnitCategory::Mass, 453.59237, "lb")),

        // Temperature (handled specially)
        "c" | "celsius" | "degc" => Some((UnitCategory::Temperature, 1.0, "°C")),
        "f" | "fahrenheit" | "degf" => Some((UnitCategory::Temperature, 2.0, "°F")),
        "k" | "kelvin" => Some((UnitCategory::Temperature, 3.0, "K")),

        // Data (base: byte)
        "b" | "byte" | "bytes" => Some((UnitCategory::Data, 1.0, "B")),
        "kb" => Some((UnitCategory::Data, 1_000.0, "KB")),
        "mb" => Some((UnitCategory::Data, 1_000_000.0, "MB")),
        "gb" => Some((UnitCategory::Data, 1_000_000_000.0, "GB")),
        "tb" => Some((UnitCategory::Data, 1_000_000_000_000.0, "TB")),
        "kib" => Some((UnitCategory::Data, 1024.0, "KiB")),
        "mib" => Some((UnitCategory::Data, 1024.0 * 1024.0, "MiB")),
        "gib" => Some((UnitCategory::Data, 1024.0 * 1024.0 * 1024.0, "GiB")),
        "tib" => Some((UnitCategory::Data, 1024.0 * 1024.0 * 1024.0 * 1024.0, "TiB")),

        // Time (base: second)
        "ms" | "millisecond" | "milliseconds" => Some((UnitCategory::Time, 0.001, "ms")),
        "s" | "sec" | "secs" | "second" | "seconds" => Some((UnitCategory::Time, 1.0, "s")),
        "min" | "mins" | "minute" | "minutes" => Some((UnitCategory::Time, 60.0, "min")),
        "h" | "hr" | "hrs" | "hour" | "hours" => Some((UnitCategory::Time, 3600.0, "h")),
        "d" | "day" | "days" => Some((UnitCategory::Time, 86400.0, "d")),

        _ => None,
    }
}

fn convert_temperature(val: f64, from_label: &str, to_label: &str) -> f64 {
    let celsius = match from_label {
        "°C" => val,
        "°F" => (val - 32.0) * 5.0 / 9.0,
        "K" => val - 273.15,
        _ => val,
    };
    match to_label {
        "°C" => celsius,
        "°F" => celsius * 9.0 / 5.0 + 32.0,
        "K" => celsius + 273.15,
        _ => celsius,
    }
}

fn default_currency_rate(code: &str) -> Option<f64> {
    match code {
        "USD" => Some(1.0),
        "EUR" => Some(0.92),
        "GBP" => Some(0.79),
        "JPY" => Some(150.0),
        "CHF" => Some(0.88),
        "CAD" => Some(1.36),
        "AUD" => Some(1.52),
        "CNY" => Some(7.24),
        _ => None,
    }
}

fn lookup_currency_rate(code: &str, rates: &HashMap<String, f64>) -> Option<(String, f64)> {
    let upper = code.to_ascii_uppercase();
    if let Some(&r) = rates.get(&upper) {
        if r > 0.0 && r.is_finite() {
            return Some((upper, r));
        }
    }
    default_currency_rate(&upper).map(|r| (upper, r))
}

/// Parses `<value> <from> [to|in|->] <to>` and returns `(formatted_display, copy_value, kind_label)`.
pub fn try_convert(input: &str, rates: &HashMap<String, f64>) -> Option<(String, String, String)> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }

    let val: f64 = parts[0].parse().ok()?;
    if !val.is_finite() {
        return None;
    }

    let (from_unit, to_unit) = if parts.len() == 4 {
        let sep = parts[2].to_ascii_lowercase();
        if sep != "to" && sep != "in" && sep != "->" {
            return None;
        }
        (parts[1], parts[3])
    } else {
        (parts[1], parts[2])
    };

    if let (Some((cat_a, factor_a, label_a)), Some((cat_b, factor_b, label_b))) =
        (lookup_unit(from_unit), lookup_unit(to_unit))
    {
        if cat_a != cat_b {
            return None;
        }
        let result = if cat_a == UnitCategory::Temperature {
            convert_temperature(val, label_a, label_b)
        } else {
            val * factor_a / factor_b
        };
        if !result.is_finite() {
            return None;
        }
        let num_str = format_number(result);
        let display = format!("{num_str} {label_b}");
        return Some((display, num_str, "Unit".to_string()));
    }

    if let (Some((_, rate_a)), Some((code_b, rate_b))) = (
        lookup_currency_rate(from_unit, rates),
        lookup_currency_rate(to_unit, rates),
    ) {
        // Rates are USD-based: 1 USD = rate_x X
        let result = val / rate_a * rate_b;
        if !result.is_finite() {
            return None;
        }
        let num_str = format!("{result:.2}");
        let display = format!("{num_str} {code_b}");
        return Some((display, num_str, "Currency (offline)".to_string()));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_and_temperature_conversions() {
        let rates = HashMap::new();
        let (disp, copy, kind) = try_convert("100 km to mi", &rates).unwrap();
        assert_eq!(kind, "Unit");
        assert!(disp.ends_with(" mi"));
        let miles: f64 = copy.parse().unwrap();
        assert!((miles - 62.1371192237).abs() < 1e-4);

        let (disp_temp, copy_temp, _) = try_convert("100 c in f", &rates).unwrap();
        assert_eq!(disp_temp, "212 °F");
        assert_eq!(copy_temp, "212");
    }

    #[test]
    fn offline_currency_conversion() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.0);
        rates.insert("EUR".to_string(), 0.90);

        let (disp, copy, kind) = try_convert("100 usd to eur", &rates).unwrap();
        assert_eq!(disp, "90.00 EUR");
        assert_eq!(copy, "90.00");
        assert_eq!(kind, "Currency (offline)");
    }

    #[test]
    fn incompatible_units_return_none() {
        let rates = HashMap::new();
        assert!(try_convert("100 km to kg", &rates).is_none());
    }

    #[test]
    fn overflow_returns_none() {
        let rates = HashMap::new();
        assert!(try_convert("1e308 tb to b", &rates).is_none());
        assert!(try_convert("1e308 usd to jpy", &rates).is_none());
    }
}
