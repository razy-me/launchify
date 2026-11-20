# Plan: Universal File Converter (`convert`)
**Date**: 2026-11-20 | **Mode**: `r-plan-it` (Compact) | **Status**: DRAFT

## 1. Goal & Requirements
- **Outcome**: Typing `convert` (or `convert <path>`) in Launchify's search bar opens an inline File Converter panel supporting local, offline conversion across images, structured data, and optional media via `ffmpeg`.
- **Requirements**:
  - `R1`: Typing `convert` (case-insensitive) or `convert <file_path>` activates `PrefixMode::Convert` and renders an inline converter panel inside the launcher window (auto-resizing height).
  - `R2`: Support drag-and-drop, native file picker (`tauri-plugin-dialog` or Win32 file dialog), or typed path input, auto-detecting source format by extension.
  - `R3`: Pure-Rust offline conversions:
    - **Images**: `png`, `jpg`/`jpeg`, `webp`, `bmp`, `gif`, `ico`, `tiff` (via `image` crate).
    - **Data**: `json` ↔ `yaml` ↔ `toml` ↔ `csv` (array of flat objects for CSV).
    - **Text/Doc**: `md` → `html` (basic/pulldown-cmark), `txt` ↔ `base64` / `hex`.
  - `R4`: Optional CLI-backed media conversion (`mp4`, `mp3`, `wav`, `webm`, `mkv`, `ogg`, `flac`) via `ffmpeg` on `PATH`, returning a clear error if `ffmpeg` is not installed.
  - `R5`: Output saved alongside source file (`<stem>.<target_ext>`, or `<stem>_converted.<target_ext>` on collision) with status feedback + "Reveal in Explorer" action.
- **Non-Goals**: Cloud/online conversion APIs (violates zero-network security rule); bundled `ffmpeg` binary; batch folder conversion in v1.
- **Assumptions**: `A1`: Users have `ffmpeg` on `PATH` if they want audio/video conversion; image/data/text conversions work 100% out-of-the-box.

## 2. Technical Design & Contracts
- **Chosen Approach**: Add `PrefixMode::Convert` in `commands.rs` / `model.rs` and a dedicated `converter/` Rust module. Keep the launcher's single-window pill UI by mounting a lightweight inline converter view in `renderer.ts` when `mode === "Convert"`, using existing `resizeLauncher` IPC.
- **Interfaces / Types / Signatures**:
  - `model.rs`: Add `Convert` to `PrefixMode`; add `ConvertRequest { source_path: String, target_format: String }`, `ConvertResult { output_path: String, bytes_written: u64 }`.
  - `converter/mod.rs`:
    - `supported_targets(ext: &str) -> Vec<&'static str>`
    - `convert_file(source: &Path, target_ext: &str) -> Result<ConvertResult, String>`
  - `commands.rs`:
    - `#[tauri::command] fn get_convert_targets(source_path: String) -> Result<Vec<String>, String>`
    - `#[tauri::command] fn convert_file_cmd(source_path: String, target_format: String) -> Result<ConvertResult, String>`
- **Data / State Flow**: `User types "convert"` -> `query("convert")` returns `mode: Convert` -> `renderer.ts` shows converter panel & calls `resizeLauncher(380)` -> `User drops/picks file` -> `get_convert_targets` populates target dropdown -> `User clicks Convert` -> `convert_file_cmd` writes output -> UI shows path + Reveal button.

## 3. File Touchpoints & Work Breakdown
| ID | Action | File / Module | Change Summary | Depends On | Verify / Test Criterion |
|---|---|---|---|---|---|
| WP-01 | EDIT | `src-tauri/Cargo.toml` | Add `image`, `serde_yaml`, `toml`, `csv`, `pulldown-cmark`, `base64` | - | `cargo check` passes |
| WP-02 | ADD | `src-tauri/src/converter/mod.rs` | Implement format registry, image/data/text converters, `ffmpeg` fallback, collision-safe output path | WP-01 | `cargo test converter` passes |
| WP-03 | EDIT | `src-tauri/src/model.rs`, `commands.rs`, `lib.rs` | Add `PrefixMode::Convert`, `ConvertResult`, `get_convert_targets`, `convert_file_cmd` | WP-02 | `cargo test` passes |
| WP-04 | EDIT | `src/types.ts`, `src/ipc.ts` | Mirror `Convert` mode, `ConvertResult`, and IPC wrappers | WP-03 | `npm.cmd run typecheck` passes |
| WP-05 | EDIT | `src/renderer.ts`, `src/styles.css`, `src/keyboard.ts` | Render inline converter panel (dropzone, file input, target select, convert button, result); suppress `blur` auto-hide while in `Convert` mode | WP-04 | `npm.cmd run build` passes |

## 4. Premortem (Edge Cases, Security, Performance)
- **Callers / Breaking Impact**: `PrefixMode` exhaustive matches in `commands.rs` and `src/renderer.ts` must handle `Convert`.
- **Edge Cases & Error Handling**: Suppress `blur` auto-hide while `Convert` mode is active (so Drag & Drop from Explorer and native file-picker dialogs don't close the launcher); unsupported extension -> empty target list + clear hint; corrupt/invalid input -> descriptive `Err(String)`; `ffmpeg` missing -> explicit install hint; output collision -> `_converted` suffix.
- **Security & Performance**: Canonicalize `source_path` and reject non-file/missing paths; run `convert_file_cmd` on `tauri::async_runtime::spawn_blocking` so UI never freezes.

## 5. Verification Plan
- **Verification Commands**: `cargo test --manifest-path src-tauri/Cargo.toml`, `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`, `npm.cmd run typecheck`, `npm.cmd run build`
- **Key Scenarios**:
  1. `convert_file("test.png", "webp")` produces valid `test.webp`.
  2. `convert_file("data.json", "yaml")` produces valid `data.yaml` and round-trips back to JSON.
  3. Typing `convert` in search bar opens inline converter panel; `Escape` returns to idle pill.
