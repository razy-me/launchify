# Cross-Cutting Review Findings: Contract & IPC Consistency

**Concern**: `contract-and-ipc`  
**Scope**: Tauri IPC boundary (`src-tauri/src/commands.rs` vs `src/ipc.ts`), data contracts (`src-tauri/src/model.rs` vs `src/types.ts`), payload serialization/deserialization, error handling across the boundary, and configuration handling.

---

### [CRITICAL] [OPEN] X-contract-01 IPC Parameter Name Mismatch in `autocomplete` Causes Complete Feature Failure
- **Status**: OPEN
- **Where**: `src/ipc.ts:20-25` vs `src-tauri/src/commands.rs:254-259`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: When the user presses `Tab` to autocomplete a path or top result, `autocompleteQuery` calls Tauri IPC `invoke("autocomplete", { input, selectedId })`. On the Rust backend, the Tauri command is declared as `pub fn autocomplete(raw: String, top_item_title: Option<String>, state: State<'_, AppState>)`. Because Tauri v2 deserializes invoke arguments by exact parameter name matching from the JSON dictionary, Tauri attempts to find `"raw"` (missing, deserialization fails) and `"top_item_title"` (missing, defaults or fails). The command invocation immediately fails with a deserialization error (`missing field 'raw'`), rejecting the Promise in `ipc.ts` and rendering the Tab autocomplete completely broken.
- **Why**: TypeScript sends `{ input, selectedId }` while Rust expects `{ raw, top_item_title }`. Furthermore, `main.ts:132` passes `selected.id` instead of the selected item's title, violating the intended parameter semantics on the backend.
- **Fix**:
```diff
--- a/src/ipc.ts
+++ b/src/ipc.ts
@@ -20,6 +20,6 @@ export async function executeItem(
 export async function autocompleteQuery(
-  input: string,
-  selectedId: string | null
+  raw: string,
+  topItemTitle: string | null
 ): Promise<string | null> {
-  return invoke<string | null>("autocomplete", { input, selectedId });
+  return invoke<string | null>("autocomplete", { raw, topItemTitle });
 }
```
```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -131,3 +131,3 @@ function bootstrap(): void {
       const selected = state.results[state.selectedIndex];
-      void autocompleteQuery(refs.input.value, selected ? selected.id : null)
+      void autocompleteQuery(refs.input.value, selected ? selected.title : null)
         .then((completed) => {
```

---

### [MAJOR] [OPEN] X-contract-02 Double Serialization and Redundant Base64 Encoding for Search Result Icons
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:140-151` vs `src/ipc.ts:43-55`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: In `commands.rs`, for every search result in `Global` mode, the backend reads `{key}.png` from `%LOCALAPPDATA%/Launchify/icons/`, runs a manual custom Base64 encoder (`encode_base64(&bytes)`), and formats it as a `data:image/png;base64,...` data URL before serializing the payload into JSON. Meanwhile, in `ipc.ts`, `resolveIconUrl` provides fallback logic for `convertFileSrc(iconPath)` assuming `icon_path` can also be a filesystem path. However, in `path_nav.rs` and `commands.rs` (Shell/Window modes), `icon_path` is always set to `None`. For indexed app items, embedding high-resolution PNG data URLs into the `query` IPC response creates substantial payload bloat (e.g. 100-300KB of base64 JSON payload serialized and deserialized across the IPC channel on every keystroke), causing frame drops during rapid typing.
- **Why**: Backend and frontend have an inconsistent icon delivery contract: backend uses inline data URLs while frontend IPC helper has code for Tauri asset URLs (`convertFileSrc`), neither coordinating an efficient caching mechanism.
- **Fix**: Use Tauri's asset protocol or stream icon paths directly with `asset://` / `convertFileSrc`, avoiding repetitive base64 serialization across the IPC boundary on every search query.
```diff
--- a/src-tauri/src/commands.rs
+++ b/src-tauri/src/commands.rs
@@ -143,6 +143,3 @@ pub fn query(raw: String, state: State<'_, AppState>) -> Result<QueryResponse, S
                     let p = icons_dir.join(format!("{key}.png"));
-                    if let Ok(bytes) = std::fs::read(&p) {
-                        let b64 = encode_base64(&bytes);
-                        r.icon_path = Some(format!("data:image/png;base64,{b64}"));
-                    } else {
+                    if p.exists() {
+                        r.icon_path = Some(p.to_string_lossy().into_owned());
+                    } else {
                         r.icon_path = None;
```

---

### [MAJOR] [OPEN] X-contract-03 `__inline__` Item ID Execution Contract Violation Causes Calc/Unit Copy Failure
- **Status**: OPEN
- **Where**: `src/main.ts:79-87` vs `src-tauri/src/commands.rs:163-248`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: When an inline evaluation (math expression, unit conversion, timestamp, etc.) is displayed in `#inline-preview` and there are no search results (e.g., in Calc mode `= 1920*0.75` or pure math `100 eur to usd`), pressing Enter triggers `executeSelected` line 79:
  ```ts
  if (state.inlinePreview) {
    try {
      await executeItem("__inline__", state.query, secondary);
    } catch {}
    hide();
  }
  ```
  However, in `commands.rs:execute`, the backend only handles item IDs with prefixes:
  - `"shell:"`
  - `"win:"`
  - `"power:"`
  - `"copy:"`
  - Or items present in the search snapshot!
  Because `"__inline__"` is not in the index snapshot and does not start with `"copy:"`, the backend falls through to `execute_target("__inline__", LaunchMode::Normal)`, which attempts `ShellExecuteW("open", "__inline__", ...)` and fails with an error (`"ShellExecuteW returned error code 2"`). The inline result is never copied to the clipboard, and the calculation result cannot be pasted.
- **Why**: Frontend expects `execute` to handle inline preview items by passing `"__inline__"`, but backend contract expects either a `"copy:<value>"` prefix or an item matching the snapshot.
- **Fix**:
```diff
--- a/src/main.ts
+++ b/src/main.ts
@@ -80,3 +80,3 @@ function bootstrap(): void {
       try {
-        await executeItem("__inline__", state.query, secondary);
+        await executeItem(`copy:${state.inlinePreview.copy_value}`, state.query, secondary);
       } catch {
```

---

### [MAJOR] [OPEN] X-contract-04 Unhandled Power Action Variants Cause IPC Failures
- **Status**: OPEN
- **Where**: `src-tauri/src/catalog.rs:82-83` vs `src-tauri/src/win32/executor.rs:182-195`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: In `catalog.rs`, built-in system power commands are registered with targets `power:lock`, `power:sleep`, `power:restart`, `power:shutdown`, `power:signout`, and `power:hibernate`. When a user selects "Sign Out" (`power:signout`) or "Hibernate" (`power:hibernate`), `commands.rs:205` strips the prefix and invokes `win32::executor::system_power_action(power_cmd)`. In `executor.rs:182-195`:
  ```rust
  match action {
      "lock" => ...
      "sleep" => ...
      "restart" => ...
      "shutdown" => ...
      other => Err(format!("Unknown power action: {other}")),
  }
  ```
  `system_power_action` returns `Err("Unknown power action: signout")` or `Err("Unknown power action: hibernate")`. This causes the Tauri IPC call `execute` to reject with an error, failing to sign out or hibernate the PC.
- **Why**: Contract discrepancy between catalog definitions and executor match arms.
- **Fix**:
```diff
--- a/src-tauri/src/win32/executor.rs
+++ b/src-tauri/src/win32/executor.rs
@@ -192,2 +192,8 @@ mod win_impl {
             "shutdown" => shell_execute("open", "shutdown.exe", Some("/s /t 0"), SW_HIDE),
+            "signout" => shell_execute("open", "shutdown.exe", Some("/l"), SW_HIDE),
+            "hibernate" => shell_execute(
+                "open",
+                "rundll32.exe",
+                Some("powrprof.dll,SetSuspendState 1,1,0"),
+                SW_HIDE,
+            ),
             other => Err(format!("Unknown power action: {other}")),
```

---

### [MINOR] [OPEN] X-contract-05 Dead IPC Command `collapse_to_row` Not Bound in Frontend Contract
- **Status**: OPEN
- **Where**: `src-tauri/src/commands.rs:278-290`, `src-tauri/src/lib.rs:50` vs `src/ipc.ts`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: `collapse_to_row(y_offset: i32, height: u32, app: AppHandle)` was implemented in `commands.rs` and registered in `tauri::generate_handler![]` in `src-tauri/src/lib.rs:50` to support smooth in-place row launching. However, no corresponding wrapper function exists in `src/ipc.ts` and `collapse_to_row` is never called by any frontend flow in `src/main.ts` or `src/renderer.ts`.
- **Why**: Incomplete feature integration leaving dead IPC surface area exposed on the backend.
- **Fix**: Either wire `collapse_to_row` into `src/ipc.ts` and `src/main.ts:executeSelected` to animate launcher collapse, or remove the unused command from the backend handler.

---

### [MINOR] [OPEN] X-contract-06 Deserialization Tagging Mismatch Risk in `ExecuteOutcome`
- **Status**: OPEN
- **Where**: `src-tauri/src/model.rs:104-109` vs `src/types.ts:39-42`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: HIGH
- **Category**: contract
- **Scenario**: In `src-tauri/src/model.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(tag = "type", content = "value")]
  pub enum ExecuteOutcome {
      Launched,
      CopiedToClipboard { value: String },
      WindowFocused,
  }
  ```
  Because `#[serde(tag = "type", content = "value")]` is used:
  - For `Launched`, serde serializes `{ "type": "Launched" }`.
  - For `WindowFocused`, serde serializes `{ "type": "WindowFocused" }`.
  - For `CopiedToClipboard { value }`, with both `tag` and `content` specified on a struct variant, serde generates `{ "type": "CopiedToClipboard", "value": { "value": "..." } }` (nested under content key `"value"`), whereas `src/types.ts` defines:
    ```ts
    export type ExecuteOutcome =
      | { type: "Launched" }
      | { type: "CopiedToClipboard"; value: string }
      | { type: "WindowFocused" };
    ```
  This expects `{ type: "CopiedToClipboard", value: string }` without the nested object. Any frontend consumer inspecting `outcome.value` would receive `undefined` or an `[object Object]` rather than the string.
- **Why**: Serde adjacently tagged enum (`tag = "...", content = "..."`) nests struct variant fields into the content field.
- **Fix**: Change enum tagging in `src-tauri/src/model.rs` to internally tagged `#[serde(tag = "type")]`.
```diff
--- a/src-tauri/src/model.rs
+++ b/src-tauri/src/model.rs
@@ -104,3 +104,3 @@ pub struct QueryResponse {
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
-#[serde(tag = "type", content = "value")]
+#[serde(tag = "type")]
 pub enum ExecuteOutcome {
```

---

### [ENHANCEMENT] [OPEN] X-contract-07 Asymmetric Configuration Contract: Backend Config Inaccessible to Frontend
- **Status**: OPEN
- **Where**: `src-tauri/src/config.rs:31-40` vs `src/state.ts:3-7`
- **Source**: `r-review` (2026-11-19)
- **Confidence**: MEDIUM
- **Category**: contract
- **Scenario**: `LaunchifyConfig` contains `max_results` (default 8, configurable 1-8). However, there is no IPC command to fetch the active configuration. The frontend hardcodes `MAX_VISIBLE_RESULTS = 8`, `ROW_HEIGHT_PX = 48`, and `computeTargetHeight` assuming 8 is always the maximum. If the user configures `max_results = 4` in `config.json`, the backend returns up to 4 items, but the frontend layout and window sizing logic have no knowledge of configuration changes, preventing dynamic adjustment of UI density or settings.
- **Why**: Absence of a `get_config` / `listen("config://updated")` IPC contract between frontend and backend.
- **Fix**: Expose a `get_config` Tauri command and emit a config change event so frontend can synchronize display limits and settings dynamically.
