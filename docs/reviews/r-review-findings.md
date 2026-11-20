# Code Review: Result Actions Dropdown

Scope: `src-tauri/src/model.rs`, `src-tauri/src/commands.rs`, `src/types.ts`, `src/ipc.ts`, `src/state.ts`, `src/renderer.ts`, `src/keyboard.ts`, `src/main.ts`, `src/styles.css`.

## Summary
- **CRITICAL**: 0
- **MAJOR**: 2
- **MINOR**: 3
- **ENHANCEMENT**: 1

---

## MAJOR

### 1. [FIXED] `.action-menu` is clipped when there are 1–2 results
- **Files**: `src/renderer.ts:253-255`, `src/styles.css:176-182`, `src/state.ts:47-53`
- **Details**: `renderActionMenu` appends `.action-menu` inside `#results-list` (`overflow-y: auto`), and the window height is sized strictly to `computeTargetHeight(state.results.length)` (`56 + 9 + n * 48`). When `results.length` is 1 or 2, `#results-list` is only `56px` or `104px` tall, while a 4-item `.action-menu` is ~`134px` tall, clipping the bottom items behind a scrollbar.
- **Fix**: Either expand the window height temporarily while `actionMenuOpen` is true to `Math.max(targetHeight, ...)` or render `.action-menu` outside `#results-list` / ensure minimum height when the menu is open.

### 2. [FIXED] `ActionKind::RunAsAdmin` is ignored for `ShellCommand` (`shell:` items)
- **Files**: `src-tauri/src/commands.rs:215-222`, `src/renderer.ts:206-213`
- **Details**: `getActionsForItem` offers `"Run as Administrator"` (`ActionKind::RunAsAdmin`) for `ItemKind::ShellCommand`. However, in `commands.rs::execute`, the `item_id.strip_prefix("shell:")` branch only checks `secondary = action == ActionKind::Secondary` and always uses `LaunchMode::ShellHidden` or `LaunchMode::ShellTerminal`, ignoring `ActionKind::RunAsAdmin`.
- **Fix**: Handle `ActionKind::RunAsAdmin` in the `shell:` branch using `LaunchMode::Elevated` (or remove `RunAsAdmin` from `ShellCommand` in `getActionsForItem` if elevated shell commands are unsupported).

---

## MINOR

### 3. [FIXED] `renderState` does not clean up `.action-menu` when results update
- **Files**: `src/renderer.ts` (`renderState`), `src/main.ts:30`
- **Details**: `runQuery` resets `state.actionMenuOpen = false`, but `renderState` only reconciles `.result-row` elements and `#selection-indicator`. If `state.results.length > 0`, any existing `.action-menu` node in `#results-list` is not removed by `renderState`.
- **Fix**: Remove `.action-menu` in `renderState` when `!state.actionMenuOpen` (or call `renderActionMenu` at the end of `renderState`).

### 4. [FIXED] No click-outside / blur dismissal for `.action-menu`
- **Files**: `src/main.ts`, `src/keyboard.ts:36-65`
- **Details**: While `state.actionMenuOpen` is true, `keyboard.ts` swallows all keys other than `ArrowUp`/`ArrowDown`/`Ctrl+J`/`Ctrl+K`/`Enter`/`ArrowLeft`/`Escape`. Clicking on another result row or elsewhere in the launcher does not close the menu.
- **Fix**: Close `actionMenuOpen` on `mousedown` outside `.action-menu` (or when `handleSelectAndExecute` runs).

### 5. [FIXED] Missing ARIA menuitem attributes
- **Files**: `src/renderer.ts:257-269`
- **Details**: `.action-menu` has `role="menu"`, but `.action-menu-item` buttons lack `role="menuitem"` and `aria-selected` (or `aria-activedescendant` on the input).
- **Fix**: Set `btn.setAttribute("role", "menuitem")` and `btn.setAttribute("aria-selected", String(idx === state.actionIndex))`.

---

## ENHANCEMENT

### 6. Visual affordance for `ArrowRight` action menu
- **Files**: `src/renderer.ts`, `src/styles.css`
- **Details**: There is no subtle hint on the selected row indicating that `ArrowRight` (at the end of the input) opens the actions menu.
- **Fix**: Consider adding a subtle `→ Actions` hint on the active row's right side when `results.length > 0`.
