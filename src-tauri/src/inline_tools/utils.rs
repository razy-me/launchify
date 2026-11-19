use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Evaluates utility expressions: Hex/RGB colors, UUID v4, and Unix/ISO timestamps.
pub fn try_eval_util(input: &str) -> Option<(String, String, String)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(res) = try_color(trimmed) {
        return Some(res);
    }
    if let Some(res) = try_uuid(trimmed) {
        return Some(res);
    }
    if let Some(res) = try_timestamp(trimmed) {
        return Some(res);
    }

    None
}

fn try_color(input: &str) -> Option<(String, String, String)> {
    if let Some(hex) = input.strip_prefix('#') {
        let (r, g, b) = if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        } else if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            (r, g, b)
        } else {
            return None;
        };
        let rgb = format!("rgb({r}, {g}, {b})");
        return Some((rgb.clone(), rgb, "Color".to_string()));
    }

    let lower = input.to_ascii_lowercase();
    if let Some(inner) = lower.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            let hex = format!("#{r:02X}{g:02X}{b:02X}");
            return Some((hex.clone(), hex, "Color".to_string()));
        }
    }

    None
}

fn try_uuid(input: &str) -> Option<(String, String, String)> {
    let lower = input.to_ascii_lowercase();
    if lower == "uuid" || lower == "uuid4" || lower == "guid" {
        let uuid = generate_uuid_v4();
        return Some((uuid.clone(), uuid, "UUID v4".to_string()));
    }
    None
}

/// Generates a random RFC 4122 version 4 UUID using std hash/time entropy.
pub fn generate_uuid_v4() -> String {
    let h1 = RandomState::new().build_hasher().finish();
    let h2 = RandomState::new().build_hasher().finish();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let hi = h1 ^ nanos.rotate_left(17);
    let lo = h2 ^ nanos.rotate_right(13);

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hi.to_le_bytes());
    bytes[8..].copy_from_slice(&lo.to_le_bytes());

    // Set version 4 (0100) and variant 1 (10xx)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

fn try_timestamp(input: &str) -> Option<(String, String, String)> {
    let lower = input.to_ascii_lowercase();
    if lower == "now" || lower == "unix" || lower == "timestamp" {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;
        let iso = format_unix_utc(secs);
        let display = format!("{secs} ({iso})");
        return Some((display, secs.to_string(), "Timestamp".to_string()));
    }

    // 10-digit (seconds) or 13-digit (milliseconds) Unix timestamp
    if (input.len() == 10 || input.len() == 13) && input.chars().all(|c| c.is_ascii_digit()) {
        let raw: i64 = input.parse().ok()?;
        let secs = if input.len() == 13 { raw / 1000 } else { raw };
        let iso = format_unix_utc(secs);
        return Some((iso.clone(), iso, "UTC Timestamp".to_string()));
    }

    // ISO date: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ
    if let Some(secs) = parse_iso_utc(input) {
        let s = secs.to_string();
        return Some((s.clone(), s, "Unix Timestamp".to_string()));
    }

    None
}

/// Converts Unix epoch seconds to `YYYY-MM-DD HH:MM:SS UTC` using Howard Hinnant's algorithm.
pub fn format_unix_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02} UTC")
}

fn parse_iso_utc(input: &str) -> Option<i64> {
    let s = input.trim().trim_end_matches('Z');
    let (date_part, time_part) = if let Some((d, t)) = s.split_once('T') {
        (d, Some(t))
    } else if let Some((d, t)) = s.split_once(' ') {
        (d, Some(t))
    } else {
        (s, None)
    };

    let d_parts: Vec<&str> = date_part.split('-').collect();
    if d_parts.len() != 3 || d_parts[0].len() != 4 {
        return None;
    }
    let year: i64 = d_parts[0].parse().ok()?;
    let month: i64 = d_parts[1].parse().ok()?;
    let day: i64 = d_parts[2].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (hour, minute, second) = if let Some(tp) = time_part {
        let t_parts: Vec<&str> = tp.split(':').collect();
        if t_parts.len() != 3 {
            return None;
        }
        let h: i64 = t_parts[0].parse().ok()?;
        let m: i64 = t_parts[1].parse().ok()?;
        let sec: i64 = t_parts[2].parse().ok()?;
        if !(0..=23).contains(&h) || !(0..=59).contains(&m) || !(0..=59).contains(&sec) {
            return None;
        }
        (h, m, sec)
    } else {
        (0, 0, 0)
    };

    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_rgb_conversions() {
        let (disp, copy, kind) = try_eval_util("#ff8800").unwrap();
        assert_eq!(disp, "rgb(255, 136, 0)");
        assert_eq!(copy, "rgb(255, 136, 0)");
        assert_eq!(kind, "Color");

        let (disp2, _, _) = try_eval_util("rgb(255, 136, 0)").unwrap();
        assert_eq!(disp2, "#FF8800");
    }

    #[test]
    fn uuid_v4_format() {
        let (disp, _, kind) = try_eval_util("uuid").unwrap();
        assert_eq!(kind, "UUID v4");
        assert_eq!(disp.len(), 36);
        assert_eq!(&disp[14..15], "4");
        let variant = u8::from_str_radix(&disp[19..20], 16).unwrap();
        assert!((8..=11).contains(&variant));
    }

    #[test]
    fn unix_and_iso_timestamps() {
        let (disp, _, kind) = try_eval_util("1700000000").unwrap();
        assert_eq!(kind, "UTC Timestamp");
        assert_eq!(disp, "2023-11-14 22:13:20 UTC");

        let (unix_str, _, _) = try_eval_util("2023-11-14T22:13:20Z").unwrap();
        assert_eq!(unix_str, "1700000000");
    }
}
