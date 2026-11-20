# Code Review Findings: File Converter (`r-review` Tier 2)

**Scope:** `src-tauri/src/converter/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/model.rs`, `src-tauri/src/lib.rs`, `src/types.ts`, `src/ipc.ts`, `src/renderer.ts`, `src/keyboard.ts`, `src/styles.css`

---

## Summary

- **CRITICAL:** 0
- **MAJOR:** 3
- **MINOR:** 6
- **ENHANCEMENT:** 2

---

## MAJOR

### 1. [FIXED] `magick` fallback fails on multi-page PDFs (`src-tauri/src/converter/mod.rs:606-622, 696-698`)
When `pdftoppm` is not on `PATH`, `convert_pdf` falls back to:
```rust
Command::new("magick").arg(source).arg(out_path)
```
For a multi-page PDF, ImageMagick writes `stem-0.png`, `stem-1.png`, etc., instead of `out_path` (`stem.png`). Immediately afterward, `fs::metadata(&out_path)` at line 696 fails with `NotFound` ("Failed to stat output ...").
**Fix:** Pass `format!("{}[0]", source.display())` (or `source` with `[0]` appended to `OsString`) to `magick` so it renders only the first page to `out_path`, matching `pdftoppm -singlefile`.

### 2. [FIXED] Concurrent `Enter` / submit race in `runConvert` (`src/renderer.ts:401-435`, `src/keyboard.ts:48-50`)
`runConvert()` does not guard against re-entry when a conversion is already in flight. Pressing `Enter` twice quickly (or clicking Convert while `launchify:convert-submit` fires) invokes `convertFile()` concurrently for the same source/target.
**Fix:** Add an `isConverting` flag or check `if (convertSubmitBtnEl.disabled) return;` at the top of `runConvert()`.

### 3. [FIXED] `runConvert` `finally` block can re-enable submit button in invalid state (`src/renderer.ts:432-434`)
In `runConvert()`, `finally { convertSubmitBtnEl.disabled = false; }` unconditionally re-enables the Convert button. If the user edits `pathInput` to an invalid/empty path while a conversion is running, `refreshConvertTargets` disables `convertSubmitBtnEl`, and `runConvert`'s `finally` block re-enables it.
**Fix:** Only re-enable `convertSubmitBtnEl` if `convertTargetSelectEl.value` is non-empty and `!convertTargetSelectEl.disabled`.

---

## MINOR

### 1. [FIXED] `pick_convert_file` early return skips restoring `always_on_top` (`src-tauri/src/commands.rs:395, 429-430`)
`window.set_always_on_top(false)` is called before `spawn_blocking`, and `window.set_always_on_top(true)` is called after `.await.map_err(...)?`. If `spawn_blocking` returns `Err`, the `?` operator returns early and leaves the launcher window without `always_on_top(true)`.
**Fix:** Restore `window.set_always_on_top(true)` before propagating the error.

### 2. [FIXED] Dead `pandocFormats` check in `refreshConvertTargets` (`src/renderer.ts:365, 372-373`)
`pandocFormats` lists source extensions (`docx`, `odt`, `rtf`, `epub`, `rst`, `tex`), which never appear in `targets` (the target formats for those sources are `md`, `html`, `txt`, `pdf`). As a result, `pandocFormats.has(fmt)` is always `false` and the `(pandoc)` label is never shown.
**Fix:** Check whether the *source* extension is in `pandocFormats` (or `target === "pdf"` for `md`/`html`) when labeling targets.

### 3. `convert_image_to_pdf` loads entire uncompressed RGB image into memory (`src-tauri/src/converter/mod.rs:143-199`)
`img.to_rgb8().into_raw()` plus `Vec::with_capacity(raw.len() + 1024)` allocates `2 * width * height * 3` bytes in RAM for uncompressed PDF embedding. Large images (e.g., 8000×8000) will spike memory (~384 MB) and produce huge PDFs.
**Fix:** Consider JPEG/Flate compression or documenting/capping dimensions.

### 4. [FIXED] `Enter` key in `keyboard.ts` submits even when focus is on `Reveal in Explorer` or `targetSelect` (`src/keyboard.ts:46-51`)
Pressing `Enter` while `state.mode === "Convert"` always dispatches `launchify:convert-submit`, even if the user tabbed onto the `Reveal in Explorer` button.

### 5. `build_output_path` TOCTOU race (`src-tauri/src/converter/mod.rs:103-125`)
`build_output_path` checks `Path::exists()` before writing, which has a standard TOCTOU window if two conversions target the same output path concurrently.

### 6. External CLI tools receive raw `source`/`out_path` without `--` (`src-tauri/src/converter/mod.rs:437-440, 571-573, 594-598, 631-636`)
No shell is invoked (so no command injection), but a relative filename starting with `-` (e.g., `-test.md`) could be parsed as an option flag by `pandoc`, `pdftotext`, or `pdftoppm`.

---

## ENHANCEMENT

1. **Multi-page PDF rendering (`src-tauri/src/converter/mod.rs:591-600`):** `pdftoppm -singlefile` only renders page 1 of a PDF to PNG/JPG. Consider documenting this in the UI or supporting page selection.
2. **HTML entity decoding (`src-tauri/src/converter/mod.rs:420-426`):** `strip_html_to_text` only decodes 6 named entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`, `&nbsp;`); `&apos;` and numeric character references (`&#...;`, `&#x...;`) are left literal.
