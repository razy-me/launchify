# SPEC — Universal File Converter (`convert`)

## Original Request
> eine "convert" funktion, dass ich in die Suchleiste "convert" eingebe und wir einen File-Converter haben, wo ich alles mögliche in alles mögliche konvertieren kann

## Requirements
- `R1`: Typing `convert` (case-insensitive) or `convert <file_path>` activates `PrefixMode::Convert` and renders an inline converter panel inside the launcher window (auto-resizing height).
- `R2`: Support drag-and-drop, native file picker, or typed path input, auto-detecting source format by extension.
- `R3`: Pure-Rust offline conversions:
  - **Images**: `png`, `jpg`/`jpeg`, `webp`, `bmp`, `gif`, `ico`, `tiff` (via `image` crate).
  - **Data**: `json` ↔ `yaml` ↔ `toml` ↔ `csv` (array of flat objects for CSV).
  - **Text/Doc**: `md` → `html` (via `pulldown-cmark`), `txt` ↔ `base64` / `hex`.
- `R4`: Optional CLI-backed media conversion (`mp4`, `mp3`, `wav`, `webm`, `mkv`, `ogg`, `flac`) via `ffmpeg` on `PATH`, returning a clear error if `ffmpeg` is not installed.
- `R5`: Output saved alongside source file (`<stem>.<target_ext>`, or `<stem>_converted.<target_ext>` on collision) with status feedback + "Reveal in Explorer" action.
- `R6`: Suppress `blur` auto-hide while `PrefixMode::Convert` is active so Drag & Drop from Explorer and native file-picker dialogs do not close the launcher.

## Non-Goals
- Cloud/online conversion APIs (zero-network security rule).
- Bundled `ffmpeg` binary.
- Batch folder conversion in v1.

## Assumptions
- `A1`: Users have `ffmpeg` on `PATH` if they want audio/video conversion; image/data/text conversions work 100% out-of-the-box.
