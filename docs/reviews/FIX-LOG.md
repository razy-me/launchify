# Fix Log
Source: docs/reviews/r-review-findings.md | Date: 2026-11-20 | Scope: CRITICAL+MAJOR+MINOR+ENHANCEMENT | Base: 261ccee4dc28754c1db99351c92e90041f5405cc

| FIXED | REJECTED | STALE | INVALID | DEFERRED |
|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 0 |

## Findings
| ID | Sev | Where | Triage | Cluster | Status | Evidence / reason | Commit | Test |
|---|---|---|---|---|---|---|---|---|
| F02-01 | CRITICAL | `src-tauri/src/commands.rs` | FIX | C2 | FIXED | Used `.get(..5)` / `.get(..2)` in `extract_find_rest` | pending | `extract_find_rest_handles_non_ascii_safely` |
| F02-02 | CRITICAL | `src-tauri/src/storage.rs` | FIX | C2 | FIXED | Used `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING \| MOVEFILE_WRITE_THROUGH` on Windows | pending | `write_atomic_overwrites_existing_file` |
| F02-03 | MAJOR | `src-tauri/src/commands.rs` | FIX | C2 | FIXED | Handled `ItemKind::Workflow` (args, `run_in_terminal`) and `ItemKind::WebShortcut` (`{query}` substitution) | pending | `workflow_command_and_web_shortcut_formatting` |
| F02-04 | MAJOR | `src-tauri/src/commands.rs` | FIX | C2 | FIXED | Set `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8` in `pick_convert_file` | pending | — |
| F02-05 | MAJOR | `src-tauri/src/storage.rs` | FIX | C2 | FIXED | Added `with_limit(64 * 1024 * 1024)` to `bincode::DefaultOptions` in `load_index` | pending | `load_index_rejects_oversized_corrupt_length_prefix` |
| F02-06 | MAJOR | `src-tauri/src/tray.rs` | FIX | C2 | FIXED | Replaced `std::thread::spawn` with `state.watch_tx.try_send(())` | pending | — |
| F02-07 | MINOR | `src-tauri/src/commands.rs` | FIX | C2 | FIXED | Moved frecency recording after execution and excluded `__inline__` and `copy:` | pending | — |
| F02-08 | MINOR | `src-tauri/src/icons.rs` | FIX | C2 | FIXED | Capped `bmWidth`/`bmHeight` at 1024 and used `checked_mul` | pending | — |
| F02-09 | ENHANCEMENT | `src-tauri/src/commands.rs` | FIX | C2 | FIXED | Scoped `frecency` lock before icon I/O and used `base64` crate | pending | — |
| F01-01 | MAJOR | `src/renderer.ts:393-397` | FIX | C1 | FIXED | Read `row.dataset.index` on `mousedown` | `c885dbc` | `npm run build` |
| F01-02 | MAJOR | `src/keyboard.ts:24-33` | FIX | C1 | FIXED | Guard `onKeyDown`, `executeSelected`, `executeWithAction` on `state.visible` | `c885dbc` | `npm run build` |
| F01-03 | MAJOR | `src/main.ts:180-189` | FIX | C1 | FIXED | Exempt `Convert` mode in `resetInactivityTimer` | `c885dbc` | `npm run build` |
| F01-04 | MINOR | `src/renderer.ts:79-102` | FIX | C1 | FIXED | Remove `.action-menu`, clear `minHeight`, reset width to 680 in `Convert` | `c885dbc` | `npm run build` |
| F01-05 | MINOR | `src/main.ts:228-241` | FIX | C1 | FIXED | Guard `onAutocomplete` on `visible`, `rawAtRequest`, `seqAtRequest` | `c885dbc` | `npm run build` |
| F01-06 | MINOR | `src/styles.css:416-426` | FIX | C1 | FIXED | Flex `.convert-status` with `.convert-status-text` ellipsis and `flex-shrink: 0` button | `c885dbc` | `npm run build` |
| F01-07 | MINOR | `src/renderer.ts:147-153` | FIX | C1 | FIXED | Strip `is-launched` and `is-green` on reused rows | `c885dbc` | `npm run build` |

## Clusters
| Cluster | Findings | Files | Order | Parallel group |
|---|---|---|---|---|
| C1 | F01-01..F01-07 | `src/renderer.ts`, `src/keyboard.ts`, `src/main.ts`, `src/styles.css` | 1 | A |
| C2 | F02-01..F02-09 | `src-tauri/src/commands.rs`, `src-tauri/src/storage.rs`, `src-tauri/src/catalog.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/icons.rs` | 1 | A |
| C3 | F03-01..F03-04 | `src-tauri/src/indexer/mod.rs`, `src-tauri/src/search/ranking.rs`, `src-tauri/src/path_nav.rs`, `src-tauri/src/indexer/watcher.rs` | 1 | FIXED |
| C4 | F04-01..F04-09 | `src-tauri/src/inline_tools/math.rs`, `src-tauri/src/converter/mod.rs`, `src-tauri/src/inline_tools/units.rs`, `src-tauri/src/inline_tools/utils.rs` | 1 | A |
| C5 | F05-01..F05-06 | `src-tauri/Cargo.toml`, `src-tauri/src/win32/executor.rs`, `src-tauri/src/win32/hook.rs`, `src-tauri/src/win32/switcher.rs` | 2 | B |

## Deferred — Decisions Needed

## Final Gate
build/lint/typecheck/test: pending | follow-up review suggested: pending
