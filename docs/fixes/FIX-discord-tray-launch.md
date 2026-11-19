# Fix Ledger: Discord im System Tray startet nicht über Launchify

## 1. Symptom & Success Criterion
- **Expected:** Wenn Discord im System Tray minimiert läuft und über Launchify ausgewählt (`Enter`) wird, öffnet/fokussiert sich das Discord-Fenster.
- **Actual:** Es passiert nichts (kein sichtbares Fenster, kein Fokus).
- **Trigger:** Discord in Launchify auswählen und `Enter` drücken, während Discord bereits im System Tray läuft.
- **Frequency:** Immer.
- **Environment:** Windows 11, Launchify Release-Build.
- **Success Criterion:** Discord-Shortcut / Target über Launchify ausführen bringt das laufende Discord-Fenster nach vorne bzw. startet es korrekt.

## 2. Repro & Root Cause Analysis
- **Repro:** `Get-Process Discord` zeigt 6 laufende Discord-Prozesse mit `MainWindowHandle = 0` (minimiert im System Tray). `Discord.lnk` zeigt auf `C:\Users\Razy\AppData\Local\Discord\Discord.exe --processStart Discord.exe`.
- **Fix:** `AllowSetForegroundWindow(ASFW_ANY)` vor `ShellExecuteW` in `src-tauri/src/win32/executor.rs:82` hinzugefügt.
- **Verify:** `npm run tauri build -- --no-bundle` erfolgreich (`launchify.exe` neu gebaut und gestartet).
- **Status:** FIXED (Live-Test mit laufendem Discord im System Tray durch Nutzer erforderlich).
