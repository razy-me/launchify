# DECISIONS (append-only)

## D-001 Tauri v2 + Vanilla TypeScript instead of Electron or React/Svelte (WP-00, Planner)
- Decision: Use Rust + Tauri v2 with Vanilla TypeScript and minimalist CSS (no UI framework).
- Alternatives considered: Electron + React; Tauri v2 + Solid/Svelte; pure Win32/Direct2D UI.
- Reason / trade-offs: Spec explicitly mandates Rust + Tauri v2 + Vanilla TS for ~15–25 MB idle RAM and `< 10 ms` warm-state show latency. With at most 8 visible rows, direct DOM rendering in Vanilla TS avoids framework runtime overhead and keeps bundle size tiny.

## D-002 bincode for index.bin + atomic JSON for config.json & frecency.json (WP-01, Planner)
- Decision: Persist the compiled index snapshot as binary `index.bin` via `bincode` and store `config.json` and `frecency.json` as atomic-write JSON files in `%LOCALAPPDATA%\Launchify\`.
- Alternatives considered: SQLite (`rusqlite`) for both index and frecency; TOML for config.
- Reason / trade-offs: `bincode` deserializes thousands of `IndexItem` records in `< 2 ms` at startup (R22). JSON for `config.json` and `frecency.json` avoids native SQLite DLL/build complexity while remaining human-editable (A3, A5). Atomic write-to-temp-then-rename prevents corruption on unexpected shutdown.

## D-003 Offline static currency rates & custom recursive-descent math parser (WP-02, Planner)
- Decision: Implement a small zero-dependency recursive-descent math parser and an offline static currency/unit conversion table (user-overridable in `config.json`).
- Alternatives considered: Fetching live exchange rates via HTTP; pulling heavy CAS crates.
- Reason / trade-offs: Strictly honors R17 (zero network requests) and keeps compile times and binary size minimal while covering all required arithmetic, unit, currency, Hex/RGB, timestamp, and UUID operations.

## D-004 Dedicated WH_KEYBOARD_LL thread with pure state-machine core (WP-05, Planner)
- Decision: Run `WH_KEYBOARD_LL` on a dedicated OS thread with its own Win32 message loop and extract the solo-`VK_LWIN` detection logic into a pure Rust state machine.
- Alternatives considered: Running hook on the Tauri main thread; using `RegisterHotKey`.
- Reason / trade-offs: `RegisterHotKey` cannot intercept solo `VK_LWIN` without breaking `Win+*` combos. A dedicated thread guarantees `< 0.5 ms` hook callback latency so Windows never unhooks it, and isolating the state machine allows deterministic unit testing of `Win+E`, `Win+L`, `Win+Shift+S`, `Win+D`, and solo `Win` tap.

## D-005 Disjoint parallel execution of WP-06 (Backend Wiring) and WP-07 (Frontend) (WP-06/WP-07, Planner)
- Decision: Freeze the Tauri IPC command & DTO contract in `TODO.md` so WP-06 (`src-tauri/src/{app,commands,tray,lib}.rs`) and WP-07 (`src/**`, `index.html`) can be implemented in parallel in Group D.
- Alternatives considered: Serializing WP-07 strictly after WP-06.
- Reason / trade-offs: Zero file ownership overlap and faster Phase 2 delivery; Phase 3 integration testing verifies the live Rust <-> TS contract end-to-end.
