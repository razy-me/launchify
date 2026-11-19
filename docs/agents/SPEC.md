# SPEC — Launchify

## Original Prompt (verbatim)
> ## 1. Visuelles Konzept & Ästhetik (AMOLED- & Mica-gerecht)
>
> Der Launcher soll sich wie ein nativer, extrem ruhiger Bestandteil von Windows 11 anfühlen – ohne bunte Gaming-RGB-Akzente, aber auch ohne macOS-Kopie.
>
> * **Geometrie & Platzierung:**
> * Ein kompaktes, schwebendes Pill- bzw. Rounded-Rectangle-Panel (`width: ~680px`, `border-radius: 14px`), horizontal zentriert und vertikal im oberen Drittel (**ca. 28 % vom oberen Bildschirmrand**) des jeweils aktiven Monitors verankert.
> * Im Ruhezustand (leere Eingabezeile) zeigt das Panel **nur die reine Suchleiste** (`height: ~56px`) – keine vorgeschlagenen Apps, keine Werbe-Kacheln, keine visuelle Unruhe. Erst mit dem ersten Zeichen klappt die Ergebnisliste darunter nahtlos aus (maximal 7–8 sichtbare Zeilen, um Scroll-Ballast zu vermeiden).
>
>
> * **Materialität & Farben:**
> * Nativer Windows-11-**Mica-** oder feiner **Acrylic-Blur-Hintergrund** mit einer hauchdünnen `1px` Innenkontur (`rgba(255, 255, 255, 0.08)` im Dark Mode), damit sich das Fenster auf einem dunklen AMOLED-Display klar vom Hintergrund abhebt.
> * Monochrom-basierte Typografie (z. B. *Segoe UI Variable* für Namen, *JetBrains Mono* für Pfade, Rechenergebnisse und Tastenkürzel). Ein einziger, entsättigter Akzent-Balken (`2px` links am selektierten Eintrag) zeigt den Fokus statt greller Vollflächen.
>
>
> * **Mikro-Animationen:**
> * Öffnen/Schließen in **unter 90 ms** (minimaler Scale-Fade von `0.98` auf `1.0`), im versteckten Zustand **null** laufende CSS-Keyframes oder Render-Loops.
>
>
>
> ---
>
> ## 2. Handhabung & Tastatur-Flow (Vertieft)
>
> Jede Interaktion muss ohne Griff zur Maus in unter zwei Sekunden erledigt sein:
>
> * **Aktivierung & Schließen:**
> * **Einzelnes Drücken & Loslassen der linken `Win`-Taste** öffnet und fokussiert den Launcher sofort. Alle Kombinationen (`Win + E`, `Win + L`, `Win + Shift + S`, `Win + D`) bleiben vollständig bei Windows.
> * **`Esc`:** Erste Stufe leert die Suchzeile (falls Text drinsteht); zweite Stufe (oder `Esc` bei leerer Zeile / Klick außerhalb) blendet das Fenster aus und gibt den Fokus exakt an das zuvor aktive Fenster zurück.
>
>
> * **Navigation & Aktionen:**
> * `↑` / `↓` (sowie `Ctrl + J` / `Ctrl + K`): Auswahl bewegen (mit Wrap-Around von unten nach oben).
> * `Enter`: Primäraktion ausführen (App starten, Datei öffnen, Rechenergebnis in die Zwischenablage kopieren).
> * `Shift + Enter` oder `Ctrl + Enter`: Sekundäraktion (z. B. *Als Administrator ausführen* bei Apps, *Im Explorer/Terminal öffnen* bei Ordnern, *Pfad kopieren* bei Dateien).
> * `Tab`: Autovervollständigung des Top-Treffers in die Suchzeile, um z. B. direkt Argumente oder Unterpfade weiterzutippen.
>
>
> * **Optionale Prefix-Modi (kein Zwang, nur Turbo-Schalter):**
> * Ohne Prefix wird global über alle relevanten Quellen gesucht. Wer gezielt filtern will, nutzt ein einzelnes Zeichen am Anfang:
> * `>` – Direkter PowerShell-/CMD-Befehl (ohne sichtbares Konsolenfenster oder wahlweise im Windows Terminal).
> * `=` – Sofort-Taschenrechner & Einheiten-/Währungsumrechner (Ergebnis erscheint live rechts in der Eingabezeile, `Enter` kopiert den Wert).
> * `/` – Direkte Pfad-Navigation im Dateisystem mit `Tab`-Completion.
> * `@` – Wechsel zu bereits offenen Fenstern (Instant Window Switcher).
>
>
>
> ---
>
> ## 3. Suchumfang: Was rein soll – und was gezielt draußen bleibt
>
> Ein Launcher wird dann träge und unübersichtlich, wenn er versucht, eine vollwertige Festplattensuche zu sein.
>
> ### Was gesucht wird (Hohe Signalstärke)
>
> 1. **Installierte Apps & System-Tools:** Alle Einträge aus `shell:AppsFolder` (klassische `.exe`, Store-/UWP-Apps, PWAs) sowie Startmenü-Verknüpfungen (`.lnk`), inklusive synonymer Suchbegriffe (z. B. „Audio“ findet auch *Soundeinstellungen* oder *GoXLR*).
> 2. **Windows-Systemeinstellungen & Power-Aktionen:** Direkte Deep-Links (`ms-settings:bluetooth`, `ms-settings:display`, `ms-settings:sound`) sowie Systembefehle (*Sperren, Ruhezustand, Neustart, Papierkorb leeren*).
> 3. **Kuratierte Arbeitsordner & Projekte:** Nur explizit freigegebene Verzeichnisse (z. B. Entwicklungsprojekte, Dokumente, Skripte) – primär Ordnernamen und Top-Level-Dateien.
> 4. **Eigene Workflows & Web-Shortcuts:** Eigene `.bat`/`.ps1`-Skripte, lokale Tools oder schnelle Such-Kürzel (`gh <repo>`, `yt <suche>`), die direkt den Standardbrowser öffnen.
> 5. **Inline-Tools (ohne externe API):** Taschenrechner (`1920 * 0.75`), Hex/RGB-Umrechnung, Zeitstempel/Zeitzonen und UUID-Generator.
>
> ### Was gezielt NICHT gesucht wird (Null Ballast)
>
> * **Keine Web-Suche / Bing-Vorschläge:** Die Suchleiste schickt niemals Tastatureingaben ins Internet und mischt keine Web-Treffer unter lokale Apps.
> * **Kein Volltext-Datei-Indexing:** Der Launcher durchsucht niemals den Inhalt von PDFs, Logs oder Quellcode-Dateien.
> * **Keine System- & Build-Verzeichnisse:** `C:\Windows`, `Program Files` (Rohdaten), `AppData` (außer Startmenü), `node_modules`, `.git`, `target`, `dist`, `__pycache__` und temporäre Caches sind auf Kernel-Watcher-Ebene komplett ausgeschlossen.
> * **Keine Medien-Bibliotheken:** Keine einzelnen Bilder, Audio-Snippets oder Video-Rohdateien – dafür ist der Explorer da.
>
> ---
>
> ## 4. Ranking-Logik: Warum der richtige Treffer auf Platz 1 steht
>
> * **Fuzzy-Matching mit Wortgrenzen-Bonus:** Treffer am Wortanfang (`V` + `S` → **V**isual **S**tudio Code) oder exakte Teilstrings werden deutlich höher bewertet als verstreute Buchstaben mitten im Wort.
> * **Frecency-Score (Frequency + Recency):** Jede ausgeführte Aktion aktualisiert lokal Zähler und Zeitstempel. Kürzlich und häufig genutzte Apps steigen automatisch auf.
> * **Prefix-Memory (Lernende Kurzschlüsse):** Merkt sich das Tool, dass du bei der Eingabe `"sp"` dreimal *Spotify* gewählt hast, bekommt *Spotify* für `"sp"` einen festen Score-Boost und landet sofort auf Platz 1.
>
> ---
>
> ## 5. Ressourcenschonung & Laptop-Effizienz (Zero Idle Load)
>
> Damit der Launcher auf dem Galaxy Book keinerlei spürbare Last, Abwärme oder Akkudrain erzeugt:
>
> * **Warm-State-Fenster:** Der Prozess läuft als stiller Hintergrund-Dienst; das Fenster existiert bereits im Speicher (`SW_HIDE`) und wird beim Hotkey nur eingeblendet (`< 10 ms` Reaktionszeit).
> * **Reines Event-Driven Indexing (0,0 % CPU im Leerlauf):**
> * Beim Start wird ein binärer Index-Snapshot (`index.bin`) in **unter 2 ms** in den RAM geladen.
> * Danach überwachen `ReadDirectoryChangesW` (Startmenü/Projektordner) und `SHChangeNotifyRegister` (`shell:AppsFolder`) Änderungen rein passiv über Windows-Kernel-Events.
> * Erst wenn tatsächlich eine App installiert oder gelöscht wird, wacht ein Worker-Thread mit niedrigster Priorität (`THREAD_MODE_BACKGROUND_BEGIN` / EcoQoS auf den E-Cores) auf, wartet 1,5 Sekunden Debounce ab und aktualisiert gezielt nur diesen einen Eintrag.
>
>
> * **Lokaler Icon-Cache:** App-Icons werden genau einmal beim Indexieren extrahiert, als kleine PNG/WebP-Blobs auf Disk gecacht und beim Start asynchron geladen – niemals live während des Tippens.
> * **WebView-Ruhezustand:** Solange das Fenster versteckt ist, wird kein Rendering getriggert; der RAM-Bedarf bleibt konstant im Bereich von **~15–25 MB**.
>
> ---
>
> ## 6. Tech-Stack & Systemarchitektur
>
> ### Tech-Stack
>
> * **Core & OS-Layer:** **Rust** + **Tauri v2** (nutzt die in Windows 11 integrierte WebView2-Runtime ohne eigenen Browser-Overhead).
> * **Win32-Integration:** `windows` Crate (offizielle Microsoft Win32/COM-Bindings für `WH_KEYBOARD_LL`, `ShellExecuteExW`, `IShellItem`, `GetForegroundWindow`, `MonitorFromPoint`).
> * **Window-Material:** `window-vibrancy` für nativen Windows-11-Mica-/Acrylic-Effekt + `WS_EX_TOOLWINDOW` (kein Eintrag in Taskleiste oder `Alt + Tab`).
> * **Such-Engine:** `nucleo` (extrem schneller, lock-freier Fuzzy-Matcher aus dem Helix-Editor) + `arc-swap` für atomare Index-Updates ohne UI-Blockade.
> * **Persistenz:** Kompakte Binärdatei (`bincode`) für den App-/Icon-Cache + schlankes JSON/SQLite für Frecency-Stats und eigene Shortcuts.
> * **Frontend:** Vanilla TypeScript + minimalistisches CSS (kein schweres UI-Framework nötig, da nur maximal 8 Zeilen gerendert werden).
>
> ### Architektur-Zusammenspiel (4 entkoppelte Schichten)
>
> 1. **Dedizierter Hook-Thread (OS-Ebene):**
> * Ein eigener, winziger Rust-Thread mit eigenem Win32-Message-Loop hält den `WH_KEYBOARD_LL`-Hook.
> * Er prüft ausschließlich, ob `VK_LWIN` allein gedrückt und wieder losgelassen wurde, unterdrückt dann das Windows-Startmenü und schickt ein Signal über einen Channel an den Window-Manager. Dadurch blockiert der Hook niemals länger als `< 0,5 ms` und wird von Windows selbst nach dem Standby nie gekappt.
>
>
> 2. **Window- & Fokus-Controller:**
> * Empfängt das Toggle-Signal, speichert das aktuelle `HWND` (`GetForegroundWindow`), berechnet die exakte Position und DPI-Skalierung des Monitors unter dem Mauszeiger (`MonitorFromPoint`), blendet das Tauri-Fenster ein und erzwingt sauber den Eingabefokus.
>
>
> 3. **Lock-Free Search & Frecency Engine:**
> * Nimmt jeden Tastenanschlag aus dem Frontend entgegen, filtert den aktuellen In-Memory-Index per `nucleo`, verrechnet den Frecency-Boost und gibt in **< 2 ms** die Top-8-Treffer zurück.
>
>
> 4. **Background Indexer & Executor:**
> * Lauscht im Hintergrund auf Kernel-Events (`ReadDirectoryChangesW` / `SHChangeNotify`), baut bei Änderungen im Hintergrund einen neuen Index-Snapshot und tauscht ihn per `ArcSwap` atomar aus. Beim Ausführen eines Treffers startet er Programme sauber über `ShellExecuteExW`.

---

## Requirements

### Visual & UI
- **R1:** Floating frameless panel (`width: 680px`, `border-radius: 14px`) horizontally centered and vertically anchored at ~28% from top of the active monitor (determined via cursor position `MonitorFromPoint` with DPI scaling). [functional]
- **R2:** Idle state (empty query) displays only the search bar (`height: ~56px`) with zero suggested apps, zero tiles, and zero visual clutter. Upon first character input, the results list expands smoothly below, capped at 7–8 visible rows max. [functional]
- **R3:** Native Windows 11 Mica/Acrylic blur via `window-vibrancy` with `1px` inner border (`rgba(255, 255, 255, 0.08)` in dark mode), `WS_EX_TOOLWINDOW` (hidden from taskbar and `Alt+Tab`), monochrome typography (`Segoe UI Variable` for names, `JetBrains Mono` for paths/results/shortcuts), and a single desaturated `2px` left accent bar for focus indication. [functional]
- **R4:** Open/close micro-animation under 90 ms (subtle scale-fade `0.98` -> `1.0`). Zero running CSS keyframes, timers, or render loops while hidden. [non-functional: performance]

### Activation, Focus & Keyboard Flow
- **R5:** Dedicated low-level keyboard hook thread (`WH_KEYBOARD_LL` with its own Win32 message loop) detects solo `VK_LWIN` press-and-release, suppresses the Windows Start Menu, and sends a toggle signal via channel in `< 0.5 ms` without blocking. All `Win+*` combos (`Win+E`, `Win+L`, `Win+Shift+S`, `Win+D`, etc.) pass through unmodified to Windows. [functional]
- **R6:** Window & Focus Controller saves the previously active `HWND` (`GetForegroundWindow`) before showing the launcher, positions it on the active monitor, focuses the input cleanly, and on hide restores focus to the saved `HWND`. [functional]
- **R7:** `Esc` two-stage behavior: if input is non-empty, first `Esc` clears input and collapses results; second `Esc` (or `Esc` on empty input, or focus loss / click outside) hides the window and restores previous window focus. [functional]
- **R8:** Keyboard navigation & actions: `↑`/`↓` and `Ctrl+J`/`Ctrl+K` move selection with wrap-around; `Enter` executes primary action; `Shift+Enter` or `Ctrl+Enter` executes secondary action (Run as Admin for apps, Open in Explorer/Terminal for folders, Copy Path for files); `Tab` autocompletes top/selected match into the input bar for arguments or subpaths. [functional]

### Prefix Modes
- **R9:** `>` prefix mode executes PowerShell/CMD commands (hidden console by default, or in Windows Terminal via secondary action). [functional]
- **R10:** `=` prefix mode (and inline auto-detection without prefix) evaluates math expressions (`1920 * 0.75`), unit/currency conversions (offline static rates/factors), Hex/RGB conversions, timestamps/timezones, and UUID generation; live result displayed right-aligned in the input bar, `Enter` copies to clipboard. [functional]
- **R11:** `/` prefix mode provides direct filesystem path navigation with `Tab` completion. [functional]
- **R12:** `@` prefix mode lists currently open top-level windows and switches/focuses the selected window on `Enter` (Instant Window Switcher). [functional]

### Search Scope & Exclusions
- **R13:** Indexes installed apps from `shell:AppsFolder` (`.exe`, Store/UWP, PWAs) and Start Menu `.lnk` shortcuts, supporting synonym/keyword aliases (e.g., "Audio" matches Sound Settings). [functional]
- **R14:** Built-in Windows System Settings deep-links (`ms-settings:bluetooth`, `ms-settings:display`, `ms-settings:sound`, etc.) and Power/System actions (Lock, Sleep/Hibernate, Restart, Empty Recycle Bin). [functional]
- **R15:** Curated workspace/project folders (user-configured in config JSON): indexes folder names and top-level files only. [functional]
- **R16:** Custom workflows & web shortcuts (user-configured `.bat`/`.ps1` scripts, local tools, and keyword URL templates like `gh <repo>`, `yt <query>` opening in default browser). [functional]
- **R17:** Strict exclusions: zero network/web search requests on keystrokes; zero full-text file content indexing; hard-excluded directories (`C:\Windows`, raw `Program Files`, `AppData` except Start Menu, `node_modules`, `.git`, `target`, `dist`, `__pycache__`, temp caches); no media library files (images/audio/video raw files). [non-functional: privacy & performance]

### Ranking & Persistence
- **R18:** Lock-free search engine using `nucleo` (fuzzy matching with word-boundary bonus and exact substring preference) over an `ArcSwap`-managed in-memory index, returning top 8 results in `< 2 ms`. [non-functional: performance]
- **R19:** Frecency scoring (frequency + time-decayed recency) persisted locally in JSON/SQLite; every execution updates counts and timestamps. [functional]
- **R20:** Prefix-Memory: tracks `(query_prefix, item_id)` selections; once a prefix-item pair reaches threshold (e.g., 3 selections), applies a strong score boost so it ranks #1 for that prefix. [functional]

### Resource Efficiency & Architecture
- **R21:** Warm-state background process (`SW_HIDE` when hidden, `< 10 ms` show latency) with system tray icon for settings/reload/quit. [non-functional: performance]
- **R22:** Event-driven indexing with 0.0% idle CPU: loads binary snapshot (`index.bin` via `bincode`) in `< 2 ms` at startup; watches Start Menu & curated folders via `ReadDirectoryChangesW` and `shell:AppsFolder` via `SHChangeNotifyRegister`; debounces changes by 1.5 s on a low-priority background thread (`THREAD_MODE_BACKGROUND_BEGIN` / EcoQoS) and atomically swaps index via `ArcSwap`. [non-functional: performance]
- **R23:** Local icon cache: extracts app icons once during indexing, caches as small PNG files on disk, loaded asynchronously — never extracted live during typing. [non-functional: performance]
- **R24:** Clean execution via `ShellExecuteExW` (supporting normal launch, `runas` elevation, Explorer/Terminal opening, and URI/deep-link launch). [functional]
- **R25:** Structured logging, graceful shutdown (unhooking `WH_KEYBOARD_LL` cleanly on exit), config validation with sensible defaults, and strict lint/typecheck/test hygiene (`cargo clippy`, `cargo test`, `tsc --noEmit`). [non-functional: quality & security]

---

## Assumptions
- **A1:** Target OS is Windows 11 (x86_64) with WebView2 Runtime pre-installed; Win32-specific modules will be gated behind `#[cfg(windows)]` with clean trait abstractions so core search, ranking, inline-tools, config, and persistence unit tests run fast and deterministically.
- **A2:** Currency conversion in `=` mode uses an offline static rate table (configurable in user config) to strictly honor R17 (zero network requests).
- **A3:** Persistence uses `bincode` for `index.bin` (fast `< 2 ms` load) and JSON files (`frecency.json`, `config.json`) in `%LOCALAPPDATA%\Launchify\` for human-editable config and lightweight atomic writes (avoiding native SQLite DLL overhead).
- **A4:** Frontend uses Vanilla TypeScript + Vite + minimalist CSS (zero UI frameworks) communicating with Tauri v2 backend via typed `invoke` commands and Tauri events.
- **A5:** A minimal system tray icon (`tauri::tray`) is included so the user can quit, open `config.json`, or trigger a manual re-index since the window is `WS_EX_TOOLWINDOW` (no taskbar entry).

## Out of Scope
- Cross-platform support (macOS / Linux)
- Live internet/Bing search or online currency API fetching
- Full-text file content indexing
- Plugin marketplace or third-party extension runtime
- GUI settings window (configuration is via clean, documented `config.json` opened from tray)
