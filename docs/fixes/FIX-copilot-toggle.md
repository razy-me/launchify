# Fix Ledger: Copilot-Taste führt beim 2. Drücken native Funktion aus statt Launchify zu schließen

**Mode:** `finally-fix-this` (round 1)

## 1. Tried List & History
1. **Attempt 1 (`app.rs`):** `is_visible` Toggle in `launchify-hook-listener` eingebaut (`hide_launcher` wenn `window.is_visible()`, sonst `show_launcher`).
   - *Result:* Symptom besteht laut Nutzer weiterhin (2. Drücken der Copilot-Taste löst native Copilot-Funktion aus).
2. **Attempt 2 (`hook.rs`):** `SwallowOnly` für `VK_F23` KeyUp ergänzt (`copilot_down` State-Flag).
   - *Result:* Symptom besteht laut Nutzer weiterhin.

## 2. Established Facts & Ruled Out
- **Fact (VERIFIED):** 1. Drücken der Copilot-Taste öffnet Launchify normal (Hook empfängt `Win+Shift+F23` und schluckt `F23`).
- **Fact (VERIFIED):** 2. Drücken (während Launchify offen/fokussiert ist) löst die native Copilot-Funktion aus.

## 3. Ground Truth Instrumentation & Root Cause
- **Ground Truth (`ground-truth.log`):**
  1. `F23` DOWN und `F23` UP wurden beim 2. Druck zwar geschluckt (`TriggerSoloWin` -> `SwallowOnly`), **aber**:
  2. `inject_mask_key()` sendete `0xFF` (ungültiger/reserved Sentinel-Keycode) an Launchify (`WS_EX_TOOLWINDOW`), und **unmittelbar danach** (während `VK_LWIN` noch gedrückt war!) rief `app.rs` `hide_launcher()` auf -> Fokuswechsel auf das vorherige Fenster (`restore_foreground_window`).
  3. Erst nach dem Fokuswechsel kam `VK_LWIN` UP an -> Windows sah ein unmaskiertes `LWIN` UP im neuen Fensterkontext und öffnete die Windows-Suche / Startmenü.
  4. Zusätzlich feuerte gedrückt halten (Auto-Repeat) `TriggerSoloWin` mehrfach hintereinander (Zeilen 138–159).

## 4. Fix Applied (Round 1)
- `src-tauri/src/win32/hook.rs`:
  - `VK_MASK_DUMMY` von `0xFF` auf `0xE8` (Standard Windows unassigned mask key) geändert.
  - Trigger auf `VK_LWIN` Release (`HookDecision::TriggerOnWinRelease`) verschoben, damit **kein Fokuswechsel zwischen `LWIN` DOWN und `LWIN` UP** stattfindet.
  - Doppelte Maskierung (`inject_mask_key()` bei `F23` DOWN und unmittelbar vor `LWIN` UP).
  - Auto-Repeat-Schutz (`!self.copilot_down`) ergänzt.
  - Temporäre Log-Instrumentierung entfernt.
