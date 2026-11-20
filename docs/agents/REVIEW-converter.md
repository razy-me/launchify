# Audit Report — Universal File Converter (`convert`)

**Date:** 2026-11-20
**Status:** MAJOR (1 Major, 1 Minor)
**Scope:** `src-tauri/src/converter/mod.rs`, `src-tauri/src/commands.rs`, `src/renderer.ts`, `src/keyboard.ts`

## Summary

| Requirement | Status | Notes |
|---|---|---|
| `R1` (`convert` prefix mode & inline panel) | **PASS** | `parse_prefix_mode` in `commands.rs` + `ensureConvertPanel` / `resizeLauncher(380)` in `renderer.ts` |
| `R2` (Drag & drop, file picker, typed path) | **PASS** | Tauri `onDragDropEvent` + HTML5 dropzone, `#convert-browse-btn` (`<input type="file">`), `#convert-path-input`, `getConvertTargets` |
| `R3` (Pure-Rust offline conversions) | **PASS** | Images (`image`), Data (`json`/`yaml`/`toml`/`csv`), Text/Doc (`md`→`html`, `txt`↔`base64`/`hex`) |
| `R4` (Optional `ffmpeg` media conversion) | **PASS** | `MEDIA_FORMATS` (`mp4`, `mp3`, `wav`, `webm`, `mkv`, `ogg`, `flac`) + clear `ErrorKind::NotFound` message |
| `R5` (Collision-safe output + status + Reveal) | **MAJOR** | `build_output_path` & `setConvertStatus` implemented, but **"Reveal in Explorer" action is missing** in `renderer.ts` |
| `R6` (Suppress `blur` auto-hide in `Convert`) | **PASS** | `onWindowBlur` in `keyboard.ts` skips `onHide()` when `state.mode === "Convert"` |

## Findings

### MAJOR-01: Missing "Reveal in Explorer" action (`R5`)
- **Location:** `src/renderer.ts` (`runConvert`, `ensureConvertPanel`)
- **Detail:** `R5` requires status feedback **plus** a "Reveal in Explorer" action after conversion. `runConvert()` currently displays `Saved: <output_path> (<bytes> bytes)` via `setConvertStatus`, with no button/link to reveal the output file in Explorer.
- **Fix:** Add a "Reveal in Explorer" button/action in `#convert-status` (or adjacent to it) when `convertFile` succeeds, invoking the existing `RevealInExplorer` / `execute` path or a dedicated reveal command.

### MINOR-01: Converter unit tests not executed at runtime
- **Location:** `docs/agents/TEST-REPORT.md`
- **Detail:** `cargo test --manifest-path src-tauri/Cargo.toml --lib converter` compiled (`--no-run` passed) but failed to execute on Windows with `0xc0000139` (`STATUS_ENTRYPOINT_NOT_FOUND`). Runtime behavior of `converter/mod.rs` is therefore verified by static inspection only.
