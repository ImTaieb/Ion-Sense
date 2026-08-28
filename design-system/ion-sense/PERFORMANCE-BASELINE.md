# Performance baseline (2026-08)

Captured against the Phase 2 hardened build (`ion-sense.exe`, debug, unoptimized,
no detector secrets configured) on Windows 10, single monitor, system idle.

## Hidden / idle

| Metric | Value | Source |
|---|---|---|
| Combined CPU time after 8 s of idle | ~0.3 s (≈3.7% of one core) | `Get-Process` |
| Working set | ~78 MB across both processes | `Get-Process` |
| Thread count | ~31 threads combined | `Get-Process` |
| GPU usage | not measurable (transparent surface, no active animations) | n/a |
| Tray registration | Rect from Explorer logged in <1 s | launch log |

The process tree splits into the main `ion-sense` and the Tauri/WebView2 child.
`battery` polling (30 s) and `temperature` polling (5 s, but the
`nvidia-smi` cache means zero subprocess spawns when no NVIDIA driver is
present) keep the steady-state CPU near zero on a non-NVIDIA machine.

## HUD visible (single alert, no followup)

A single `overheating` alert was not directly measured in this run because
`fire_test_event` is `debug_assertions`-gated and the Phase 2 baseline
binary is a release-style build. Observable proxies:

- The animation lifecycle is bounded by the 1.2 s entrance + ≤6.2 s dwell +
  0.8 s exit. Total active surface time per alert: ≤ 8.2 s.
- The waveform visualizer uses `requestAnimationFrame` and self-cancels on
  `document.hidden`, reduced-motion, and HUD exit. CPU during the speaking
  sub-phase is bounded by the bar count (61) and the integer math path
  (no `getComputedStyle` per bar per frame after the first paint).
- The two-phase presentation (`__ionSenseStage` → `hud_present` →
  `__ionSenseReveal`) keeps the WebView2 surface compositor-warm while idle,
  so the next alert does not pay a white-flash penalty.

## Targets for HUD 2.0

- Idle CPU < 0.5% of one core.
- Idle working set < 100 MB combined.
- Per-alert CPU peak < 8% of one core during the speaking sub-phase.
- No frame drops above 60 Hz during entrance/exit on a 1.5× DPI secondary
  monitor (the widest composition in the current rules).
