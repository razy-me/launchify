# TOFIX: WP-03

## Findings

### [CRITICAL] UTF-8 byte-boundary panic in prefix slicing (`src-tauri/src/commands.rs`)
- **Location**: `src-tauri/src/commands.rs` (`parse_prefix_mode` and `query`)
- **Issue**:
  - `parse_prefix_mode` checks `trimmed.len() >= 8` and slices `trimmed[..8]` / `trimmed[8..]`.
  - `query` checks `trimmed_raw.len() >= 5` / `>= 2` and slices `trimmed_raw[..5]` / `trimmed_raw[5..]` and `trimmed_raw[..2]` / `trimmed_raw[2..]`.
  - In Rust, `str::len()` returns byte length, and slicing `&s[..n]` panics if `n` is not a UTF-8 character boundary. Any user query containing multi-byte UTF-8 characters straddling byte 2, 5, or 8 (e.g., `€`, umlauts, or emoji) will crash the backend with a panic.
- **Fix**:
  Use `str::get(..n)` (e.g., `if trimmed.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("convert "))`) and `trimmed.get(8..)` (or `is_char_boundary`) instead of unchecked byte-index slicing. Add unit tests with multi-byte UTF-8 inputs (e.g., `"€"`, `"äöü12345"`) to `commands::tests`.
