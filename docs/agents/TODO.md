# TODO — Extended File Converter Formats
**Plan Status:** APPROVED 2026-11-20 (from existing blueprint `docs/plans/PLAN-converter-formats.md`)

## Work Packages
- [x] **WP-01**: `src-tauri/Cargo.toml` — Add `percent-encoding = "2"`, `pdf-extract = "0.7"`
- [x] **WP-02**: `src-tauri/src/converter/mod.rs` — Expand `IMAGE_EXTS`, `VIDEO_EXTS`, `AUDIO_EXTS`, `PANDOC_EXTS`, `PDF` (`img->pdf`, `pdf->txt`, `pdf->img`), `tsv`, `html->txt`, `txt<->url`, `video->gif` + unit tests
- [x] **WP-03**: `src/renderer.ts` — Display `(ffmpeg)` / `(pandoc)` / `(pdftoppm)` hint in target `<option>` label while keeping raw `value`
