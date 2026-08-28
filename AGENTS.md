# AGENTS.md — Ion Sense

Persistent project knowledge for coding agents. Keep it concise; do not turn it into a changelog.

## What Ion Sense is

A tray-resident Windows awareness utility (Tauri 2 + Rust + static HTML/JS frontend, no bundler).
Detectors watch system events (battery, temperature, downloads, IMAP email, Discord bot messages)
and surface them as a temporary green HUD overlay instead of conventional notifications.
Part of the larger ION ecosystem (ION = assistant, Ion Sense = passive awareness layer).

## Product philosophy

- Mostly invisible: no permanent UI, no animations while idle.
- Green identity (`#5CF47A` family). Never convert to ION's blue. Severity changes energy and
  cadence, not hue.
- Transparency + restrained blur/glass; no large opaque black backgrounds; glow stays on
  linework/icons, never body text.
- See `design-system/ion-sense/MASTER.md` for tokens, motion timings, and accessibility rules.

## Architecture (verified 2026-08)

```
Detectors (OS threads in src-tauri/src/detectors/)
  battery_low   battery crate, threshold crossing + re-arm, 30s poll
  overheating   sysinfo components + nvidia-smi fallback (Windows), 2 consecutive hot samples
  downloads     notify FS watcher on Downloads dir, temp-extension + stability heuristic
  email         async-imap over rustls, IMAP IDLE or polling, UID cursor, OS-keychain password
  friend_message serenity Discord gateway, bot DMs + allowed guild channels, dedup by message id
        │  try_dispatch
        ▼
EventDispatcher (mpsc capacity 64 + pending counter)  ── detectors never touch Tauri APIs
        ▼
forward_events (lib.rs)  waits for hud_ready, positions HUD, emits "ion-sense://trigger"
        ▼
HUD frontend (frontend/hud-prototype.html)
  __ionSenseStage → hud_present (native show) → __ionSenseReveal → auto-dismiss → hud_idle ack
  severity variables, waveform visualizer, Web Speech voice line, FIFO queue
```

- `src-tauri/src/lib.rs` owns windows (hud + settings), tray (Tauri tray + native Win32 fallback),
  settings popover lifecycle (generation counters, fade, close fallback), autostart.
- `settings.rs` validates/sanitizes/atomically saves JSON settings with `.bak` recovery.
- `credentials.rs` stores IMAP passwords / Discord tokens only in the OS keyring (keyring crate,
  service `com.ionsense.desktop.*`). Never log, never persist elsewhere.
- HUD window: transparent, always-on-top, click-through (`set_ignore_cursor_events`), kept shown
  while idle to avoid a WebView2 white flash; sized to the monitor **work area** (not over taskbar).
- Tauri command ACL lives in `src-tauri/capabilities/*.json` (least privilege, per window).

## Directories

- `src-tauri/src/` — Rust backend (lib.rs, event.rs, dispatcher.rs, settings.rs, credentials.rs, detectors/)
- `frontend/` — shipped UI: `hud-prototype.html` (HUD), `settings.html` (tray popover). Static, no build step.
- `prototype/hud-prototype.html` — byte-identical browser-only copy for `?dev=1` visual testing; keep in sync or drop intentionally.
- `design-system/ion-sense/MASTER.md` — visual/motion/accessibility contract.
- `.tmp/` — gitignored local scratch (screenshots, dev logs, recordings). Never commit.

## Commands

```powershell
pnpm install            # JS deps (pnpm@11). node_modules already present in this repo.
pnpm tauri dev          # dev loop; enables settings-window test events + window.ionSenseTest()
cargo test --manifest-path src-tauri/Cargo.toml     # 21 unit tests
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml
cargo fmt --check --manifest-path src-tauri/Cargo.toml
pnpm tauri build        # NSIS + MSI installers under src-tauri/target/release/bundle/
```

Note: on this machine cargo lives in `%USERPROFILE%\.cargo\bin` (not on Git Bash PATH by default);
pnpm is not on PATH but `node_modules/.bin/tauri` works.

## Conventions

- Rust 2024 edition, rust-version 1.88. Tauri deps are exact-pinned (`=x.y.z`) on purpose.
- Detectors: plain OS threads (or single-threaded tokio runtime for async ones), take
  `(Settings, EventDispatcher, Arc<AtomicBool> stop)`, poll with `wait_until_stopped`, never
  reference Tauri. Handle queue-full by dropping with an eprintln, never by blocking.
- Events are the typed contract in `event.rs` (`type`/`message`/`severity`/`timestamp`,
  snake_case serde). HUD validates with `normalizeEvent` and rejects unknown types.
- Settings changes must go through `sanitized()` + `validate()`; restart detectors on save.
- Frontend is plain ES2019+ in IIFEs, `"use strict"`, no dependencies, no framework.
- All error paths eprintln with the `Ion Sense` prefix; no panics on the settings/tray command path.

## Do NOT break

- The two-phase HUD presentation contract (`__ionSenseStage`/`hud_present`/`__ionSenseReveal`)
  and the `hud_idle` timestamp ack — Rust depends on them to sequence the FIFO.
- The transparent-window behavior: never hide/show the HUD native window per alert; never paint
  an opaque body background in the HUD window.
- OS-keychain-only storage of IMAP/Discord secrets.
- Green visual identity and the `MASTER.md` motion/reduced-motion rules.
- The detector shutdown contract: `DetectorRuntime::drop` signals `stop` and reaps workers off
  the Tauri command thread (network detectors may be inside long timeouts).
- `package_delivered` stays contract-only (no detector yet) — intentional.

## Known debt (as of 2026-08)

- `frontend/*.html` and `prototype/hud-prototype.html` each stack 2–3 generations of CSS override
  blocks (older token systems cascade beneath newer ones). Consolidate carefully; the final
  cascade is what ships.
- Downloads hint text in settings.html mentions only `.crdownload`/`.part`; the detector now also
  accepts `.partial`/`.download` and stable final-name creations.
- `event-meta` renders 7.5–8.5px text at small breakpoints, below the MASTER.md 8.5px floor.
- No JS typecheck/lint exists (no bundler); HUD/settings JS is untyped by design.
