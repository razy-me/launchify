# Fix Ledger: `f: mebo` does not surface `meboard.exe` in Top 8

## Symptom
- **Expected:** Searching `f: mebo` after executing `meboard.exe` 7+ times should surface `meboard.exe` in the Top 8 results (frecency / prefix memory boost).
- **Actual:** `meboard.exe` does not appear in the Top 8 results for `f: mebo`.
- **Trigger:** `f: mebo` (FindFiles mode).
- **Frequency:** Always.

## Root Cause Analysis
1. **Hypothesis 1 (VERIFIED):** `commands.rs` calls `find_files::search_recent_files(fq, &roots, cfg.max_results)` directly without passing `frecency` or `prefix_memory_threshold`!
2. **Hypothesis 2:** `find_files::search_recent_files` sorts only by `modified_unix` (recency) and `name`, completely ignoring `frecency` / `prefix_memory_boost`.
