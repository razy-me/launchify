# Plan: Extended File Converter Formats (Video, Audio, Docs, Data)
**Date**: 2026-11-20 | **Mode**: `r-plan-it` (Compact) | **Status**: DRAFT

## 1. Goal & Requirements
- **Outcome**: Expand `converter/mod.rs` to support a much broader matrix of Video, Audio, Image, Data, Text, and Document conversions (pure-Rust out-of-the-box + `ffmpeg` / `pandoc` CLI fallbacks).
- **Requirements**:
  - `R1`: **Video & Audio (`ffmpeg`)**: Support `mp4`, `mkv`, `webm`, `mov`, `avi`, `flv`, `wmv`, `m4v` ↔ each other, Video → `gif` (animated GIF) & Video → Audio (`mp3`, `wav`, `flac`, `ogg`, `aac`, `m4a`, `opus`, `wma` with `-vn`), plus Audio ↔ Audio.
  - `R2`: **Extended Images (pure Rust `image`)**: Add `tga`, `qoi`, `pnm` (`ppm`/`pgm`/`pbm`) to the existing image matrix (`png`, `jpg`, `jpeg`, `webp`, `bmp`, `gif`, `ico`, `tiff`).
  - `R3`: **Extended Data & Text (pure Rust)**: Add `tsv` (tab-separated) to `json`/`yaml`/`toml`/`csv`; add `html` → `txt` (tag stripping), `md` → `txt`, and `txt` ↔ `url` (percent-encoding).
  - `R4`: **PDF & Documents**:
    - **Pure Rust / CLI**: Image (`png`, `jpg`, `webp`, `bmp`, `tiff`) → `pdf` (pure Rust); `pdf` → `txt` (`pdf-extract` or `pdftotext`); `pdf` → `png`/`jpg` (`pdftoppm`/`magick`).
    - **Documents (`pandoc` / `soffice` on `PATH`)**: `docx`, `odt`, `rtf`, `epub`, `rst`, `tex` ↔ `md`, `html`, `txt`, `pdf`.
  - `R5`: **UI Hint**: Label CLI-backed targets in the dropdown (e.g. `mp3 (ffmpeg)`, `docx (pandoc)`) without breaking `target_format` value.
- **Non-Goals**: Bundling `ffmpeg.exe` or `pandoc.exe` in the installer; multi-file archive/folder batching.
- **Assumptions**: `A1`: `ffmpeg` / `pandoc` are invoked from `PATH` when selected; pure-Rust formats remain zero-dependency at runtime.

## 2. Technical Design & Contracts
- **Chosen Approach**: Keep the single `converter/mod.rs` module and extend `supported_targets(ext)` and `convert_file(source, target_ext)` with categorized format tables (`VIDEO_EXTS`, `AUDIO_EXTS`, `IMAGE_EXTS`, `DATA_EXTS`, `PANDOC_EXTS`) and dedicated helpers (`convert_via_ffmpeg`, `convert_via_pandoc`, `convert_tsv`).
- **Interfaces / Types / Signatures**:
  - `converter/mod.rs`: `supported_targets(ext: &str) -> Vec<&'static str>` (unchanged signature, expanded matrix).
  - `commands.rs`: `get_convert_targets` & `convert_file_cmd` (unchanged signatures).
- **Data / State Flow**: `ext` -> `supported_targets(ext)` -> `renderer.ts` renders `<option value="mp3">MP3 (ffmpeg)</option>` -> `convert_file_cmd(src, "mp3")` -> `convert_via_ffmpeg`.

## 3. File Touchpoints & Work Breakdown
| ID | Action | File / Module | Change Summary | Depends On | Verify / Test Criterion |
|---|---|---|---|---|---|
| WP-01 | EDIT | `src-tauri/Cargo.toml` | Add `percent-encoding = "2"`, `pdf-extract = "0.7"` | - | `cargo check` passes |
| WP-02 | EDIT | `src-tauri/src/converter/mod.rs` | Expand `IMAGE_EXTS`, `VIDEO_EXTS`, `AUDIO_EXTS`, `PANDOC_EXTS`, `PDF` (`img->pdf`, `pdf->txt`, `pdf->img`), `tsv`, `html->txt`, `txt<->url`, `video->gif` + tests | WP-01 | `cargo check` & `clippy` pass |
| WP-03 | EDIT | `src/renderer.ts` | Display `(ffmpeg)` / `(pandoc)` hint in target `<option>` label while keeping raw `value` | WP-02 | `npm.cmd run typecheck` & `build` pass |

## 4. Premortem (Edge Cases, Security, Performance)
- **Callers / Breaking Impact**: None — all IPC and Rust function signatures remain identical.
- **Edge Cases & Error Handling**: Video → Audio uses `-vn` so `ffmpeg` doesn't fail on video streams; `pandoc` missing returns `"pandoc not found on PATH. Install pandoc to convert document files."`.
- **Security & Performance**: Arguments passed via `Command::arg(...)` (no shell interpolation); `spawn_blocking` prevents UI freeze.

## 5. Verification Plan
- **Verification Commands**: `cargo check --manifest-path src-tauri/Cargo.toml`, `npm.cmd run typecheck`, `npm.cmd run build`
- **Key Scenarios**:
  1. `supported_targets("mp4")` includes `mkv`, `webm`, `gif`, `mp3`, `wav`, `aac`, `opus`.
  2. `convert_file("data.tsv", "json")` and `convert_file("page.html", "txt")` produce valid output.
