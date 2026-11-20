# Review Findings — F01 Frontend (UI, State, Keyboard, IPC)

### [MAJOR] [OPEN] F01-01 Stale `index` closure on reused DOM rows executes wrong item on click
- **Status**: OPEN
- **Where**: `src/renderer.ts:393-397` (`src/renderer.ts:147-153`)
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User types `"ch"` so item X renders at index `3` (`createResultRow` binds `mousedown` closing over `index = 3`). User types `"r"` (`"chr"`), re-ranking item X to index `0`. `renderState` reuses the existing DOM node and updates `existing.dataset.index = "0"`, then user clicks item X at index `0` -> `onSelectAndExecute(3, secondary)` fires, executing `state.results[3]` (a different item) or nothing if fewer than 4 results exist.
- **Why**: `createResultRow` captures the initial `index` parameter in its `mousedown` listener closure, whereas FLIP row reuse in `renderState` only updates `existing.dataset.index` and never rebinds the listener.
- **Fix**:
```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -391,9 +391,10 @@ function createResultRow(
   }
 
   row.addEventListener("mousedown", (e) => {
     e.preventDefault();
+    const currentIndex = Number(row.dataset.index);
     const secondary = e.shiftKey || e.ctrlKey;
-    onSelectAndExecute(index, secondary);
+    onSelectAndExecute(currentIndex, secondary);
   });
 
   return row;
```

### [MAJOR] [OPEN] F01-02 Double-execution and state mutation via keyboard during close/launch-fade window
- **Status**: OPEN
- **Where**: `src/keyboard.ts:24-33` (`src/main.ts:55-88`, `src/main.ts:100-140`)
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User selects an app or shell command and presses `Enter` twice quickly (or holds `Enter` past key-repeat), or presses `Enter` in the action menu and taps `Enter` again during the 650ms `is-launch-fading` animation -> `state.visible` is `false`, but `onKeyDown` still dispatches `onExecute` and `executeSelected` does not check `state.visible`, causing the item/command to be executed multiple times.
- **Why**: `#launcher.is-launch-fading` only disables pointer events (`pointer-events: none`), while the window remains focused for 650ms and neither `onKeyDown` nor `executeSelected`/`executeWithAction` guards on `state.visible`.
- **Fix**:
```diff
--- a/src/keyboard.ts
+++ b/src/keyboard.ts
@@ -22,6 +22,9 @@ export function attachKeyboardFlow(
   let altDownAlone = false;
 
   const onKeyDown = (e: KeyboardEvent): void => {
+    if (!state.visible) {
+      return;
+    }
     if (
       ((e.key === "Alt" && e.location === 1) || (e.key === "Shift" && e.location === 2)) &&
       !e.repeat
```

### [MAJOR] [OPEN] F01-03 Inactivity timer auto-hides launcher during active Convert mode
- **Status**: OPEN
- **Where**: `src/main.ts:180-189`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User enters `Convert` mode and either spends >15 seconds choosing a file in the native OS file picker (`pickConvertFile`) or runs an `ffmpeg`/`pandoc` conversion that takes >15 seconds without pressing keys in the WebView -> `inactivityTimer` fires and calls `hide()`, hiding the launcher and clearing Convert state via `onLauncherHidden`.
- **Why**: `onWindowBlur` in `src/keyboard.ts:142` explicitly exempts `state.mode === "Convert"` so file picking, drag-and-drop, and conversions are not interrupted, but `resetInactivityTimer` in `src/main.ts` has no such exemption.
- **Fix**:
```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -180,9 +180,12 @@ function bootstrap(): void {
   const resetInactivityTimer = (): void => {
     clearInactivityTimer();
-    if (!state.visible) {
+    if (!state.visible || state.mode === "Convert") {
       return;
     }
     inactivityTimer = window.setTimeout(() => {
       inactivityTimer = null;
+      if (state.mode === "Convert") {
+        return;
+      }
       hide();
     }, INACTIVITY_TIMEOUT_MS);
   };
```

### [MINOR] [OPEN] F01-04 Entering Convert mode leaves `.action-menu` orphaned and window stuck at 896px width
- **Status**: OPEN
- **Where**: `src/renderer.ts:79-102`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User opens the action menu on a result (`ArrowRight`, widening the window to `896px` and appending `.action-menu` to `document.body`), then types `convert` into the search input -> `renderState` takes the `state.mode === "Convert"` early return at line 101, skipping `.action-menu` cleanup (line 195) and leaving `lastWidthPx` at `896` instead of `680`.
- **Why**: The `Convert` branch returns early before the shared action-menu cleanup and calls `resizeLauncher(targetHeight)` without resetting `width` or `lastWidthPx`.
- **Fix**:
```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -80,6 +80,8 @@ export function renderState(
     refs.launcher.classList.remove("has-results");
     refs.launcher.classList.add("has-convert");
     refs.resultsList.hidden = true;
     refs.resultsList.classList.remove("animate-enter");
+    document.body.querySelector(".action-menu")?.remove();
+    refs.resultsList.style.minHeight = "";
 
     const panel = ensureConvertPanel(refs.launcher);
     panel.hidden = false;
@@ -92,9 +94,11 @@ export function renderState(
     }
 
     const targetHeight = 380;
-    if (targetHeight !== state.lastHeightPx) {
+    const targetWidth = 680;
+    if (targetHeight !== state.lastHeightPx || targetWidth !== lastWidthPx) {
       state.lastHeightPx = targetHeight;
-      void resizeLauncher(targetHeight).catch(() => {
+      lastWidthPx = targetWidth;
+      void resizeLauncher(targetHeight, targetWidth).catch(() => {
         // Ignore resize errors when running in browser preview without Tauri
       });
     }
```

### [MINOR] [OPEN] F01-05 `onAutocomplete` race condition overwrites newer input or runs after hide
- **Status**: OPEN
- **Where**: `src/main.ts:228-241`
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User presses `Tab` to autocomplete and immediately types another character or presses `Escape` to hide the launcher before `autocompleteQuery` resolves -> the `.then()` callback overwrites `refs.input.value` with the stale completion and invokes `runQuery(completed)` even though the input changed or the launcher is hidden.
- **Why**: Unlike `runQuery` (which checks `seq !== state.querySeq`), `onAutocomplete` does not verify that `state.visible` is still true and that `refs.input.value` / `state.querySeq` have not changed since the IPC call was issued.
- **Fix**:
```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -227,10 +227,15 @@ function bootstrap(): void {
     },
     onAutocomplete: () => {
       const selected = state.results[state.selectedIndex];
-      void autocompleteQuery(refs.input.value, selected ? selected.title : null)
+      const rawAtRequest = refs.input.value;
+      const seqAtRequest = state.querySeq;
+      void autocompleteQuery(rawAtRequest, selected ? selected.title : null)
         .then((completed) => {
+          if (!state.visible || refs.input.value !== rawAtRequest || state.querySeq !== seqAtRequest) {
+            return;
+          }
           if (completed !== null && completed !== refs.input.value) {
             refs.input.value = completed;
             refs.input.setSelectionRange(completed.length, completed.length);
             void runQuery(completed);
           }
```

### [MINOR] [OPEN] F01-06 "Reveal in Explorer" button clipped off-screen by `overflow: hidden` on `.convert-status`
- **Status**: OPEN
- **Where**: `src/styles.css:416-426` (`src/renderer.ts:528-541`)
- **Source**: `r-review` (2026-11-20)
- **Confidence**: HIGH
- **Category**: bug
- **Scenario**: User converts a file whose output path is longer than ~55 characters (typical on Windows, e.g. `C:\Users\<name>\Documents\...\output.png`) -> `Saved: <path> (<bytes> bytes) ` fills the single line and pushes the appended `.convert-reveal-btn` past the right edge, where `.convert-status`'s `white-space: nowrap; overflow: hidden; text-overflow: ellipsis;` clips it completely out of view.
- **Why**: `.convert-status` truncates all inline content as a single non-wrapping line instead of using a flex layout where the text truncates and the button has `flex-shrink: 0`.
- **Fix**:
```diff
--- a/src/styles.css
+++ b/src/styles.css
@@ -416,13 +416,15 @@
 .convert-status {
+  display: flex;
+  align-items: center;
+  gap: 8px;
   font-family: var(--font-mono);
   font-size: 12px;
   padding: 8px 10px;
   border-radius: 7px;
   background: rgba(255, 255, 255, 0.04);
   color: var(--text-secondary);
-  white-space: nowrap;
-  overflow: hidden;
-  text-overflow: ellipsis;
+}
+.convert-status-text {
+  flex: 1;
+  min-width: 0;
+  white-space: nowrap;
+  overflow: hidden;
+  text-overflow: ellipsis;
+}
+.convert-reveal-btn {
+  flex-shrink: 0;
 }
```
(Wrap the status message in a `<span class="convert-status-text">` inside `setConvertStatus` before appending `revealBtn`.)

### [MINOR] [OPEN] F01-07 Reused result rows retain `is-launched`/`is-green` classes when launcher reopens during fade
- **Status**: OPEN
- **Where**: `src/renderer.ts:147-153` (`src/main.ts:66`, `src/main.ts:288-298`)
- **Source**: `r-review` (2026-11-20)
- **Confidence**: MEDIUM
- **Category**: bug
- **Scenario**: User launches an item (adding `is-launched is-green` to its `.result-row`) and immediately re-triggers the launcher hotkey within 650ms before `hideLauncher()` / `onLauncherHidden` runs -> `onLauncherShown` runs `runQuery("")` while `has-results` is still set, so `renderState` reuses the existing `.result-row` DOM node without stripping `is-launched` and `is-green`, leaving the row highlighted green.
- **Why**: `renderState` only removes `is-entering` when reusing an existing row, not `is-launched` or `is-green`.
- **Fix**:
```diff
--- a/src/renderer.ts
+++ b/src/renderer.ts
@@ -148,7 +148,7 @@ export function renderState(
       if (existing) {
         existing.dataset.index = String(index);
         existing.classList.toggle("is-selected", isSelected);
-        existing.classList.remove("is-entering");
+        existing.classList.remove("is-entering", "is-launched", "is-green");
         existing.setAttribute("aria-selected", isSelected ? "true" : "false");
         fragment.appendChild(existing);
```
