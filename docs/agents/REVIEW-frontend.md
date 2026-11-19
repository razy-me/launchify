# REVIEW frontend — reviewer: fresh agent
Scope: `index.html`, `src/main.ts`, `src/types.ts`, `src/ipc.ts`, `src/state.ts`, `src/renderer.ts`, `src/keyboard.ts`, `src/styles.css`

| # | Sev | Category (bug/security/perf/maint/spec) | File:Line | Finding | Recommendation |
|---|---|---|---|---|---|
| 1 | MINOR | bug | `src/main.ts:130-133` | `state.visible` is initialized to `true` and `renderVisibility(refs, true)` is called on startup ("Initial visible state for dev/preview"), even though in Tauri warm-state the window starts hidden (`SW_HIDE`). | Gate the initial `state.visible = true` behind `!("__TAURI_INTERNALS__" in window)` so Tauri production/warm-state starts with `visible = false`. |
| 2 | MINOR | bug | `src/main.ts:112-125` | `onAutocomplete` does not check `querySeq` (or whether `refs.input.value` changed) before applying the async `autocompleteQuery` result, creating a race if the user types while `Tab` completion is in flight. | Capture `const seq = state.querySeq` (or `const before = refs.input.value`) before `autocompleteQuery` and abort if it changed before resolving. |
| 3 | MINOR | bug | `src/keyboard.ts:17-59` | `onKeyDown` does not check `e.isComposing`, so pressing `Enter` or `Escape` during IME composition triggers launcher execution/hide. | Add `if (e.isComposing) return;` at the top of `onKeyDown`. |
| 4 | ENHANCEMENT | maint | `src/renderer.ts:119-123` | Result rows have `role="option"` and `aria-selected`, but lack `id` attributes paired with `aria-activedescendant` on `#search-input`. | Assign `row.id = \`result-${index}\`` and update `refs.input.setAttribute("aria-activedescendant", ...)` in `updateSelectionOnly`. |

## Challenged Decisions (D-xxx): agree / disagree + reason
- **D-001 (Tauri v2 + Vanilla TypeScript instead of Electron or React/Svelte):** **Agree.** With at most 8 result rows, direct DOM manipulation via `DocumentFragment` and `replaceChildren()` keeps the bundle minimal, avoids framework runtime overhead, and meets R4 (`< 90 ms` animation, zero idle loops/timers).
- **D-005 (Disjoint parallel execution of WP-06 and WP-07):** **Agree.** `src/types.ts` and `src/ipc.ts` cleanly mirror the backend IPC contract; `npm.cmd run typecheck` (`tsc --noEmit`) passes with zero errors.

## Missing / partially met requirements
- None. R1–R4, R7–R8, R10, and R23 are all met in the frontend implementation.

## Overall assessment
Clean, minimal, and spec-compliant Vanilla TypeScript frontend. Zero XSS/DOM injection risks (all dynamic strings use `textContent`), zero idle CSS `@keyframes` or JS timers (R4), and full keyboard flow support (R7, R8). `tsc --noEmit` passes with 0 errors. Only 3 minor edge-case bugs and 1 accessibility enhancement were found.
