# TOFIX — WP-02 (`src-tauri/src/converter/mod.rs`)

## Findings

1. **[MAJOR] Potential panic on non-ASCII input in `decode_hex`** (`src-tauri/src/converter/mod.rs:234-245`)
   - `decode_hex` filters out whitespace (`!c.is_whitespace()`) and then slices `&cleaned[i..i + 2]` by byte index.
   - If the input `.hex` file contains multi-byte UTF-8 characters (e.g., `"aéb"`), `i..i + 2` can land in the middle of a UTF-8 codepoint and panic at runtime instead of returning `Err(...)`.
   - **Fix:** Validate `cleaned.is_ascii()` (or iterate over `cleaned.as_bytes().chunks_exact(2)` after checking `is_ascii()` and `len() % 2 == 0`).

2. **[MINOR] `build_output_path` overwrites `_converted` on repeated conversions** (`src-tauri/src/converter/mod.rs:79-90`)
   - `build_output_path` checks `!primary.exists()` and falls back to `{stem}_converted.{target_ext}`, but does not check whether `{stem}_converted.{target_ext}` already exists. Running the same conversion twice silently overwrites the previous `_converted` file.
   - **Fix:** Loop with a counter (`{stem}_converted.{target_ext}`, `{stem}_converted_2.{target_ext}`, ...) until `!candidate.exists()`.
