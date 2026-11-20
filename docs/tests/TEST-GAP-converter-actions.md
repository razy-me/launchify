# Test Gap Summary: Converter, Hook & Commands

## Scope
Inspected `src-tauri/src/converter/mod.rs`, `src-tauri/src/win32/hook.rs`, and `src-tauri/src/commands.rs` for untested critical paths, error branches, and boundary conditions.

## Tests Added
- `src-tauri/src/win32/hook.rs`
  - `solo_rshift_tap_triggers_launcher`: verifies `VK_RSHIFT_CODE` solo press/release returns `HookDecision::Trigger`.
  - `rshift_combo_passes_through_without_trigger`: verifies `VK_RSHIFT_CODE` + another key returns `HookDecision::PassThrough`.
- `src-tauri/src/converter/mod.rs`
  - `convert_file_missing_file_returns_err`: verifies `convert_file` on a nonexistent path returns `Err`.
  - `convert_file_unsupported_extension_returns_err`: verifies `convert_file` on an unsupported extension returns `Err`.
  - `supported_targets_handles_uppercase_extension`: verifies `supported_targets("PNG")` normalizes case and returns expected image/pdf targets.

## Verification
- `cargo check --manifest-path src-tauri/Cargo.toml`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --no-run`: compiled test binary (see run output).
