# Feature-Roadmap & Ausbauplan — Launchify

**Goal:** Launchify um alltagsnahe, schnelle Zero-Bloat-Power-Features erweitern, ohne den minimalistischen Windows-11-Core oder die Latenz zu belasten.

## 1. Requirements (Priorisiert)
- **R1 — Clipboard History (`cb` / `Alt+V` oder Prefix):** Lokale In-Memory/JSON-Historie der letzten 30 kopierten Texte (via `AddClipboardFormatListener`), per Fuzzy-Suche durchsuchbar und mit `Enter` direkt einfügbar/kopierbar.
- **R2 — Web Search & Custom Bangs (`g `, `yt `, `gh `, `w `):** Konfigurierbare Such-Shortcuts in `config.json` (z. B. `g <query>` -> Google, `gh <query>` -> GitHub), die ohne Netzwerk-Calls im Launcher direkt die URL im Standardbrowser öffnen.
- **R3 — Quick Snippets / Textbausteine (`;email`, `;addr`):** Kurze statische oder dynamische Snippets (`{date}`, `{time}`, `{uuid}`) aus `config.json`, die bei `Enter` in die Zwischenablage kopiert werden.
- **R4 — Konfigurierbarer Hotkey & Settings-UI (`config.json` + IPC):** `get_config` / `save_config` IPC + kleines Inline-Settings-Panel (Trigger-Taste wählen: Linkes Alt, Rechte Strg, Alt+Space; `max_results`, Autostart-Toggle), statt Code neu zu kompilieren.
- **R5 — System-Quick-Actions & Media/Volume (`vol 50`, `mute`, `darkmode`, `recycle`):** Direkte Windows-Steuerung ohne externe Tools (Lautstärke, Mute, Papierkorb leeren, Dark/Light-Mode-Toggle).
- **R6 — App-Kontextmenü (`Shift+Enter` / `Ctrl+Enter`):** Sekundäre Aktionen auf Suchtreffern (Als Admin starten, Dateipfad im Explorer öffnen, Pfad kopieren) mit kleinem Footer-Hint.

## 2. Non-Goals
- Keine Cloud-Sync, keine Telemetrie, kein Plugin-Marketplace mit schwerer JS-Runtime, keine Hintergrund-Web-Requests.

## 3. Work Packages

| WP | Feature | Actions & Affected Files | Dependencies | Acceptance Criteria |
|---|---|---|---|---|
| **WP-01** | Web Bangs & Snippets | `EDIT` `config.rs`, `commands.rs`, `search/engine.rs` | — | `g rust` öffnet Google-Suche; `;date` kopiert aktuelles Datum; konfigurierbar über `config.json`. |
| **WP-02** | App-Kontextmenü (`Shift+Enter` / `Ctrl+O`) | `EDIT` `commands.rs`, `keyboard.ts`, `renderer.ts` | — | `Shift+Enter` öffnet als Admin, `Ctrl+Shift+C` kopiert Zielpfad, `Ctrl+E` öffnet Ordner im Explorer. |
| **WP-03** | System-Quick-Actions | `EDIT` `catalog.rs`, `win32/executor.rs`, `commands.rs` | — | `mute`, `recycle` (Papierkorb leeren), `signout`, `hibernate` funktionieren zuverlässig. |
| **WP-04** | Clipboard History (`cb `) | `ADD` `win32/clipboard.rs`; `EDIT` `app.rs`, `commands.rs` | — | Letzte 30 Text-Copies lokal gespeichert, mit `cb <text>` durchsuchbar, `Enter` kopiert Eintrag. |
| **WP-05** | Hotkey & Config via IPC | `EDIT` `config.rs`, `win32/hook.rs`, `commands.rs`, `ipc.ts` | — | `get_config` / `update_config` IPC; Hotkey-Wechsel ohne Neukompilieren möglich. |

## 4. Verification Plan
- `npm run build` (`tsc --noEmit && vite build`) & `cargo check --manifest-path src-tauri/Cargo.toml` nach jedem WP.
- Manuelle Tests pro WP: Bangs im Browser öffnen, `Shift+Enter` Admin-Prompt, `cb ` zeigt Clipboard-Einträge, `config.json`-Änderung greift live.
