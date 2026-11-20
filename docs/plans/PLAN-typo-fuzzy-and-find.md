# Plan: Tippfehler-tolerante Fuzzy-Suche & `find:` / `f:` Dateisuche

**Goal:** Launchify um echte Tippfehler-tolerante Fuzzy-Suche (z. B. `"Dsicord"` -> `"Discord"`) und einen `find:`/`f:`-Modus für zuletzt bearbeitete/erstellte Dateien (inkl. Extension-Filter wie `f: .exe` und Fuzzy-Suche wie `find: launchify`) erweitern.

## 1. Requirements
- **R1 (Typo-Toleranz):** `SearchEngine::search` nutzt bei fehlendem `nucleo`-Subsequence-Treffer (oder als sekundären Score) eine **Damerau-Levenshtein-Distanz** (Transpositionen wie `si` <-> `is` zählen als 1 Edit), sodass `"Dsicord"` zuverlässig `"Discord"` findet.
- **R2 (`find:` / `f:` Prefix):** `commands.rs::parse_mode` erkennt `find:` und `f:` (case-insensitive) als `QueryMode::FindFiles(rest)`.
- **R3 (Recency-Dateisuche):** Ein eigener schneller Scanner/Cache für Benutzerordner (`Desktop`, `Downloads`, `Documents`, `C:\Tools` / konfigurierte Pfade) liefert Dateien sortiert nach `max(modified, created)` absteigend.
- **R4 (Extension- & Fuzzy-Filter):**
  - `f:` (leer) -> Top-8 zuletzt geänderte/erstellte Dateien.
  - `f: .exe` (beginnt mit `.` oder `*.`) -> zuletzt geänderte Dateien mit passender Endung.
  - `find: launchify` -> Fuzzy/Typo-Match auf Dateinamen, gewichtet mit Aktualität.

## 2. Non-Goals
- Keine Volltext-Indizierung von Dateiinhalten, keine systemweite `C:\`-Durchsuchung (zu langsam ohne Everything-Service; Fokus auf User-Ordner + `indexed_folders`).

## 3. Architecture & Contracts
- **`src-tauri/src/search/ranking.rs`:**
  - `pub fn damerau_levenshtein(a: &str, b: &str) -> usize` (auf `chars().map(|c| c.to_ascii_lowercase())`).
  - `pub fn typo_fuzzy_score(query: &str, title: &str) -> Option<u32>` (Prüfung auf Gesamttitel & einzelne Wörter; max Edit-Distanz `<= 2` bzw. `<= len/3`).
- **`src-tauri/src/search/engine.rs`:**
  - In `search()`: Wenn `nucleo` `None` liefert, `typo_fuzzy_score(q, &item.title)` prüfen und mit kleinem Basis-Score + Frecency einreihen.
- **`src-tauri/src/find_files.rs` (neu):**
  - `pub fn search_recent_files(query: &str, roots: &[PathBuf], limit: usize) -> Vec<QueryMatch>`
  - Durchläuft die konfigurierten/Standard-User-Ordner (flach + begrenzte Tiefe z. B. max depth 4, ohne `node_modules`/`.git`/`target`), sammelt `(PathBuf, SystemTime)` und filtert/sortiert nach Extension (`.ext`) oder Fuzzy/Typo-Score + Recency.
- **`src-tauri/src/commands.rs`:**
  - `QueryMode::FindFiles(&str)` in `parse_mode` für `find:` / `f:` ergänzen.
  - Treffer als `LauncherItem` mit `id: format!("file:{}", path.display())` (oder direkt über `execute` -> `OpenPath`) zurückgeben; `commands::execute` unterstützt `file:<path>` zum Öffnen der Datei.

## 4. Work Packages
1. **WP-01 — Typo-Tolerante Fuzzy-Suche (`search/ranking.rs`, `search/engine.rs`)**
   - `damerau_levenshtein` + `typo_fuzzy_score` implementieren und in `SearchEngine::search` integrieren.
   - Unit-Test: `"Dsicord"` findet `"Discord"`.
2. **WP-02 — `find:` / `f:` Dateisuche (`find_files.rs`, `lib.rs`, `commands.rs`)**
   - `find_files::search_recent_files` mit Extension-Filter (`f: .exe`), Recency-Sortierung und Fuzzy/Typo-Filter (`find: launchify`).
   - `QueryMode::FindFiles` in `commands.rs` verdrahten + `file:` Execution.
   - Unit-Tests für `.exe`-Filter, Recency-Sortierung und Fuzzy-Match.

## 5. Verification Plan
- `cargo check --manifest-path src-tauri/Cargo.toml` & `npm run build`
- Release-Build testen: `"Dsicord"` -> zeigt `Discord`; `"f: .exe"` -> zeigt neueste `.exe`-Dateien; `"find: launchify"` -> findet Launchify-Dateien.
