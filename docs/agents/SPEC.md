# SPEC: Launchify Feature Expansion

**Source Blueprint:** `docs/plans/PLAN-feature-expansion.md`

## Requirements
- **R1 (Power):** Add `empty_recycle_bin`, `hibernate`, and `signout` to `win32/executor.rs` and `catalog.rs` (`builtin_catalog()`).
- **R2 (Task Killer):** `kill <filter>` lists running processes (PID, name, RAM) and `Enter` terminates the selected PID via Win32.
- **R3 (Web & Bangs):** Default bangs (`g`, `yt`, `gh`, `ddg`, `wiki`) + user-defined `web_shortcuts` in `config.json`; `<bang> <query>` opens percent-encoded URL.
- **R4 (Live Currency):** Fetch USD-base rates from `open.er-api.com/v6/latest/USD` on startup (if stale >1h) and on-demand; persist to `%LOCALAPPDATA%\Launchify\currency_rates.json` for offline use.
- **R5 (Clipboard History):** Background watcher records text clipboard changes (max 50, deduplicated, persisted to `clipboard.json`); `cb <filter>` searches history and `Enter` copies selection back to clipboard.
