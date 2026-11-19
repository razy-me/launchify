# Review Findings Fraction F5: Frontend UI, State, Renderer & Styles

Scope: `src/main.ts`, `src/ipc.ts`, `src/keyboard.ts`, `src/renderer.ts`, `src/state.ts`, `src/types.ts`, `src/styles.css`, `index.html`

---

### [MAJOR] [OPEN] F5-01 Click/mousedown on results list executes wrong item when list is scrolled
- **Status**: OPEN
- **Where**: `src/renderer.ts:85-91`, `src/renderer.ts:113-121`, `src/styles.css:140-159`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: When user moves selection or lists more than 8 results (or in custom window sizes / scrollable states) where `#results-list` has `overflow-y: auto`, `#selection-indicator` has `position: absolute; top: 0; transform: translateY(...)` inside the scrolling container. Furthermore, in `updateSelectionOnly`, clicking or navigating to an item scrolls the list, but `#selection-indicator` transform is calculated purely as `selectedIndex * 48px` without accounting for `scrollTop`, causing visual desynchronization. More critically, clicking a row on mousedown (`e.preventDefault()`) invokes `onSelectAndExecute(index, secondary)` using the closure-captured `index` from the rendered list without verifying if the result list was replaced or if `state.results[index]` corresponds to current state during rapid typing/queries.
- **Why**: The absolute positioning of `#selection-indicator` relies on strict static index layout without handling potential scroll offsets if `#results-list` expands beyond max-height, and `createResultRow` holds index closures that can desynchronize if `state.results` changes mid-event.
- **Fix**: Ensure indicator position accounts for row geometry or attach relative selection styling directly to rows, and ensure row clicks resolve item by ID or validate bounds against current state.

```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -86,8 +86,9 @@ export function renderState(
     const indicator = document.createElement("div");
     indicator.id = "selection-indicator";
     if (state.selectedIndex >= 0) {
-      indicator.style.transform = `translateY(${state.selectedIndex * 48}px)`;
+      indicator.style.transform = `translateY(${state.selectedIndex * ROW_HEIGHT_PX}px)`;
       indicator.style.opacity = "1";
     } else {
```

---

### [MAJOR] [OPEN] F5-02 Race condition in launch fade animation can leave launcher hidden or state corrupted on re-open
- **Status**: OPEN
- **Where**: `src/main.ts:67-75`, `src/main.ts:153-176`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: concurrency
- **Scenario**: User executes an item with Enter. `executeSelected` sets `state.visible = false` and schedules a timeout for 650ms to call `renderVisibility(refs, false)` and `hideLauncher()`. If the user immediately presses the global hotkey (Win+Shift+F23) at 300ms, `onLauncherShown` fires. `onLauncherShown` sets `state.visible = true`, clears input, and starts query animation. However, the pending 650ms `setTimeout` from `executeSelected` has a simple check `if (!state.visible)`. While `state.visible` was set to `true` at 300ms, if the user pressed Escape or launched an item again, or if a race occurs with blur/hidden transitions, multiple timers overlap without any cleanup/cancellation ID (`launchFadeTimeoutId`). The pending timeout calls `refs.launcher.classList.remove("is-launch-fading", "is-closing")` and `hideLauncher()`, abruptly closing the launcher in the middle of the user's new session.
- **Why**: Timeouts for UI transitions (`650ms` in `executeSelected`, `185ms` in `hide()`, `120ms` in `onLauncherShown`) are unmanaged fire-and-forget timers that are never tracked or cleared when the launcher changes state or is re-opened.
- **Fix**: Track active timeout IDs and cancel them whenever `onLauncherShown`, `hide()`, or `executeSelected` is invoked.

```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -21,6 +21,14 @@ function bootstrap(): void {
   const refs = getDomRefs();
   const state = createInitialState();
+  let launchTimer: number | null = null;
+  let hideTimer: number | null = null;
+  let queryTimer: number | null = null;
+
+  const cancelTimers = (): void => {
+    if (launchTimer !== null) { clearTimeout(launchTimer); launchTimer = null; }
+    if (hideTimer !== null) { clearTimeout(hideTimer); hideTimer = null; }
+    if (queryTimer !== null) { clearTimeout(queryTimer); queryTimer = null; }
+  };
 
   const runQuery = async (rawInput: string): Promise<void> => {
@@ -67,7 +75,8 @@ function bootstrap(): void {
-      window.setTimeout(() => {
+      cancelTimers();
+      launchTimer = window.setTimeout(() => {
+        launchTimer = null;
         if (!state.visible) {
           refs.launcher.classList.remove("is-launch-fading", "is-closing");
           renderVisibility(refs, false);
```

---

### [MAJOR] [OPEN] F5-03 Window blur event triggers `hideLauncher` during initial window focus or alt-tab interactions
- **Status**: OPEN
- **Where**: `src/keyboard.ts:61-63`, `src/keyboard.ts:66`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: In Tauri on Windows, when the launcher window is shown or transitioning focus via `apply_toolwindow_style` and `SetForegroundWindow`, or when interacting with native context menus or IME candidates, `window.blur` fires in the webview. In `keyboard.ts:61-63`:
  ```ts
  const onWindowBlur = (): void => {
    handlers.onHide();
  };
  ```
  `handlers.onHide()` immediately marks `state.visible = false` and invokes `hideLauncher()` via Tauri IPC. If an IME composition or transient focus handoff causes a momentary blur event, the launcher unexpectedly closes while the user is typing in Japanese, Chinese, or using an input method editor.
- **Why**: Listening directly to `window.blur` without checking `document.hasFocus()` or ignoring transient blur events caused by IME / webview focus initialization leads to premature dismissal.
- **Fix**: Verify document focus before initiating hide, or debounce blur events with a short microtask/requestAnimationFrame check.

```diff
--- a/src/keyboard.ts
+++ b/src/keyboard.ts
@@ -61,3 +61,7 @@ export function attachKeyboardFlow(
   const onWindowBlur = (): void => {
-    handlers.onHide();
+    requestAnimationFrame(() => {
+      if (!document.hasFocus()) {
+        handlers.onHide();
+      }
+    });
   };
```

---

### [MINOR] [OPEN] F5-04 Missing active-descendant or ARIA relationship between search input and selected result item
- **Status**: OPEN
- **Where**: `index.html:12-26`, `src/renderer.ts:140-145`, `src/renderer.ts:123-132`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: In `index.html`, `#search-input` defines `role="combobox"` (or `aria-autocomplete="list"` and `aria-controls="results-list"`), and `#results-list` has `role="listbox"`. When navigating through results via Up/Down arrows (`src/keyboard.ts`), `aria-activedescendant` is never updated on `#search-input`, and the result row DOM elements do not have unique `id` attributes (e.g. `result-option-${index}`). Screen readers and accessibility tools cannot determine which option in the listbox is currently active.
- **Why**: `updateSelectionOnly` updates `aria-selected` and `.is-selected` on `.result-row`, but fails to assign an ID to each option or bind `aria-activedescendant` on `#search-input`.
- **Fix**: Assign `id="result-item-${index}"` to each result row and update `refs.input.setAttribute("aria-activedescendant", isSelected ? id : "")`.

```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -124,7 +124,10 @@ export function updateSelectionOnly(refs: DomRefs, selectedIndex: number): void
   rows.forEach((row, i) => {
     const isSelected = i === selectedIndex;
     row.classList.toggle("is-selected", isSelected);
     row.setAttribute("aria-selected", isSelected ? "true" : "false");
+    if (isSelected) {
+      refs.input.setAttribute("aria-activedescendant", row.id);
+    }
   });
+  if (selectedIndex < 0) {
+    refs.input.removeAttribute("aria-activedescendant");
+  }
```

---

### [MINOR] [OPEN] F5-05 `executeSelected` ignores execution failures and fails to inform the user
- **Status**: OPEN
- **Where**: `src/main.ts:63-65`, `src/main.ts:79-87`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: robustness
- **Scenario**: When executing an item via Enter or mouse click, `executeItem(selected.id, state.query, secondary)` is invoked as a floating promise with `.catch(() => {})`. If execution fails on the backend (e.g., file not found, permission denied, command error), the UI has already started fading out (`is-launch-fading`), turns green, and closes the window 650ms later. The user sees a green "success" launch animation even when the command completely failed to execute, with zero visual indication or recovery path.
- **Why**: The execution feedback animation is applied optimistically before the backend IPC response returns, and errors are completely discarded in empty catch handlers.
- **Fix**: Await or handle `executeItem` result; if an error occurs, revert the launch-fading animation and highlight the row in an error state instead of fading out.

```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -63,3 +63,7 @@ function bootstrap(): void {
-      void executeItem(selected.id, state.query, secondary).catch(() => {
-        // Backend logs execution errors; keep UI responsive
-      });
+      void executeItem(selected.id, state.query, secondary)
+        .catch((err) => {
+          row?.classList.remove("is-green");
+          row?.classList.add("is-error");
+          refs.launcher.classList.remove("is-launch-fading");
+        });
```

---

### [MINOR] [OPEN] F5-06 Unhandled promise rejections on window resize IPC calls
- **Status**: OPEN
- **Where**: `src/renderer.ts:104-109`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: robustness
- **Scenario**: When typing fast, `renderState` is called on every query response. `state.lastHeightPx` is updated synchronously (`state.lastHeightPx = targetHeight`), but `resizeLauncher(targetHeight)` is an asynchronous IPC call. Because `lastHeightPx` is immediately mutated before the IPC call succeeds, if `resizeLauncher` fails or is throttled/dropped by Tauri window message queue, subsequent calls with the same target height will skip calling `resizeLauncher` because `targetHeight === state.lastHeightPx`, leaving the window at the wrong physical dimensions.
- **Why**: Optimistic state mutation of `lastHeightPx` without synchronization against actual window resize completion.
- **Fix**: Only set `state.lastHeightPx` after successful resize or reset `state.lastHeightPx` on resize error.

```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -104,7 +104,9 @@ export function renderState(
   const targetHeight = computeTargetHeight(state.results.length);
   if (targetHeight !== state.lastHeightPx) {
-    state.lastHeightPx = targetHeight;
-    void resizeLauncher(targetHeight).catch(() => {
+    void resizeLauncher(targetHeight).then(() => {
+      state.lastHeightPx = targetHeight;
+    }).catch(() => {
       // Ignore resize errors when running in browser preview without Tauri
     });
   }
```

---

### [ENHANCEMENT] [OPEN] F5-07 `input` event is unthrottled and spams Tauri IPC on rapid keystrokes
- **Status**: OPEN
- **Where**: `src/main.ts:118-120`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: performance
- **Scenario**: User rapidly types or pastes a query (or holds backspace). An IPC message `query` is dispatched to the Tauri backend on every single input event without debouncing or animation-frame throttling. Even though `seq !== state.querySeq` discards older responses in TypeScript, the backend thread pool and search engine still evaluate all intermediate queries, causing CPU churn and IPC serialization overhead.
- **Why**: Direct binding of `input` event to async backend query without micro-debounce (e.g. 10-15ms) or checking for composition events.
- **Fix**: Add a lightweight debounce or requestAnimationFrame batching for `runQuery` during rapid input.

```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -118,3 +118,7 @@ function bootstrap(): void {
-  refs.input.addEventListener("input", () => {
-    void runQuery(refs.input.value);
-  });
+  let inputDebounce: number | null = null;
+  refs.input.addEventListener("input", () => {
+    if (inputDebounce !== null) clearTimeout(inputDebounce);
+    inputDebounce = window.setTimeout(() => {
+      void runQuery(refs.input.value);
+    }, 12);
+  });
```

---

### [ENHANCEMENT] [OPEN] F5-08 Replace full DOM reconstruction (`replaceChildren`) with keyed reconciliation
- **Status**: OPEN
- **Where**: `src/renderer.ts:82-101`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: performance
- **Scenario**: Every time a search query completes, `refs.resultsList.replaceChildren()` destroys all result row DOM nodes and recreates them from scratch, along with new event listeners and image elements. When search results only change slightly or reorder (e.g. typing one character that filters from 8 items to 6 items), all image loads and DOM layout nodes are discarded, causing avoidable GC pressure and image reloading.
- **Why**: Complete recreation of rows instead of reusing existing row elements.
- **Fix**: Reuse existing `.result-row` elements in the DOM tree where possible by updating text content and image `src` instead of clearing children entirely.
