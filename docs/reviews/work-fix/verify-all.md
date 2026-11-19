# Combined Fix Verification (`445df9b4096ae48efb89eef50ae93ea535b29e36..HEAD`)

**Status:** VERIFIED_WITH_MINOR_NOTE  
**Checks:** `npx.cmd tsc --noEmit` (pass), `cargo check --manifest-path src-tauri/Cargo.toml` (pass), `cargo test --manifest-path src-tauri/Cargo.toml` (exit `0xc0000139` `STATUS_ENTRYPOINT_NOT_FOUND` at test binary startup).

## Verified Findings
- **F1-01, F1-02, F1-04** (`src-tauri/src/win32/executor.rs`, `src-tauri/src/win32/switcher.rs`): Fixed. PowerShell commands use Base64 UTF-16LE `-EncodedCommand`, `wt.exe` uses `--` boundary, `GlobalFree` is called on `GlobalLock`/`SetClipboardData` failure, `DwmGetWindowAttribute` error is checked, and `AttachThreadInput` + `BringWindowToTop` + `SetForegroundWindow` is used. Unit test `wt_args_include_double_dash_and_quoted_encoded_command` added.
- **F2-01, F2-02/X-contract-01, F2-03/X-contract-03** (`src-tauri/src/commands.rs`, `src/ipc.ts`): Fixed. Unknown `item_id` in `execute` now returns `Err` instead of falling back to `execute_target`; `ipc.ts` parameter names match `commands.rs` (`raw`, `action`, `topItemTitle`, `height`).
- **F3-01, F3-02, F4-01, F4-03** (`src-tauri/src/indexer/scanner.rs`, `src-tauri/src/icons.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/path_nav.rs`): Fixed with regression tests (`scan_lnk_dir_initializes_icon_key_to_none`, `extract_png_for_target_balances_com_initialization`, `write_atomic_overwrites_existing_file`, `format_autocomplete_windows_drive_path_has_no_leading_slash`).
- **F5-02, F5-03** (`src/main.ts`, `src/keyboard.ts`): Fixed. `executeTimer` is tracked and cleared on `onLauncherShown`; `onWindowBlur` defers `onHide()` via `requestAnimationFrame` + `!document.hasFocus()`.

## Minor Issue Found
- **Caller mismatch (`src/main.ts:137`)**: `autocompleteQuery(refs.input.value, selected ? selected.id : null)` passes `selected.id` instead of `selected.title` to `topItemTitle`.
