# ION Sense interface system

This is the **final** ION Sense design contract. Future work consumes these tokens; do not introduce new values, weights, radii, easings, or colors without updating the system and this file.

## Product character

ION Sense is a quiet desktop awareness utility. The interface should feel approximately 50% Apple / macOS, 30% ION identity, 20% Iron Man / JARVIS. When idle, almost nothing moves. When something happens, ION wakes up, expresses itself briefly, then settles back down. Future technology should look effortless, not theatrical.

## Core tokens

### Color

- Background: `#030605`
- Surface: `rgba(8, 14, 10, 0.86)`
- Surface raised: `rgba(11, 19, 14, 0.92)`
- Hairline: `rgba(207, 244, 216, 0.09)`
- Active hairline: `rgba(69, 238, 134, 0.32)`
- Accent: `#45EE86`
- Accent bright: `#65F59B`
- Accent RGB: `69, 238, 134`
- Text primary: `#ECF4EE`
- Text secondary: `rgba(220, 232, 224, 0.66)`
- Text tertiary: `rgba(190, 205, 196, 0.42)`

### Type scale (px, no in-betweens)

`10 / 11 / 12 / 13 / 15 / 17 / 22 / 28`

### Font weight (4)

- `400` body
- `500` medium
- `600` semibold
- `700` bold

Display headlines use 600. UI labels use 500 or 600.

### Letter spacing (4)

- `-0.01em` display (22px+)
- `0` body
- `0.06em` UI labels
- `0.18em` eyebrow / uppercase tracking

### Spacing (9)

`4 / 8 / 12 / 16 / 20 / 24 / 32 / 40 / 48` — applied as padding, margin, and gap. Never reach for an in-between value.

### Radius (4)

- `6px` small controls (toggle thumb, close button, I-glyph, signal dot)
- `10px` inputs, buttons, settings cards
- `16px` panel shell, large surfaces
- `999px` pills, status badges

The HUD's alert panel keeps its asymmetric pill (`138px 62px 62px 138px`) as a deliberate identity mark.

### Typography roles

| Role | Size | Weight | Tracking | Color |
|---|---|---|---|---|
| Eyebrow / uppercase label | 11 | 500 | 0.18em | text tertiary (or accent for ION) |
| UI label / table cell | 12 | 500 | 0.06em | text secondary |
| Body | 13 | 400 | 0 | text secondary |
| Display headline | 22 | 600 | -0.01em | text primary |
| Small print / hint | 11 | 400 | 0 | text tertiary |

## Motion language

### Curves (3)

- `cubic-bezier(.2, .7, .2, 1)` — control spring (`--ion-spring`). Buttons, toggles, inputs, save success.
- `cubic-bezier(.25, .8, .25, 1)` — spatial spring (`--ion-spatial`). Panel entrance, view transitions, alert entrance.
- `linear` — dwell. The 5s alert progress bar.

No other curves.

### Durations (4)

- `140ms` micro (`--ion-motion-micro`)
- `200ms` control (`--ion-motion-ctrl`)
- `280ms` panel (`--ion-motion-panel`)
- `360ms` spatial (`--ion-motion-spatial`)

The alert progress dwell is `5000ms` linear.

### Reduced motion

`@media (prefers-reduced-motion: reduce)` collapses all motion to `1ms` or a `140ms linear` fade. The badge arc, panel breath, and visualizer loops are disabled. The HUD's two-phase presentation contract is preserved.

## Elevation (3 levels)

- **level-0**: background only (`var(--ion-bg)`)
- **level-1**: surface card — `1px solid var(--ion-border)` + `1px inset top highlight` + `box-shadow: 0 1px 2px rgba(0,0,0,0.4)`
- **level-2**: panel shell or popover — level-1 + `0 18px 36px rgba(0,0,0,0.28)` outer shadow + `backdrop-filter: blur(14px) saturate(112%)`

No multi-shadow filter chains. No `filter: drop-shadow` on glow. No `box-shadow` on body text. No neon halos on inputs (use a 1px ION border + native outline for focus).

## ION core / brand badge

The badge is a 220px circular core composed of:

- One hairline outer ring (1px, 0.5 opacity, no animation).
- One short arc (1.5px stroke, `--ion-green-bright`, rotating -10° → 10° → -10° over 5s with `--ion-spatial`).
- Three small indicator dots (2x2, `--ion-green`, no animation).
- The central I-glyph (2.5px stroke, `--ion-green-bright`, no animation).

Idle motion is restricted to the arc. No particle clouds. No continuous 360° rotation. No bouncing assembly. No radar-sweep conic gradient.

## Alert HUD

- Entrance: `360ms` spatial spring — `opacity 0→1, translateY 5px→0, scale 0.985→1`.
- Exit: `200ms` control spring — `opacity 1→0, translateY 0→-3px, scale 1→0.992`.
- Severity: `--severity-border 0.18 / 0.32 / 0.55` for info / warning / critical.
- Idle motion: a 6s opacity breath on the panel inner highlight. Nothing else.
- Voice + waveform + visualizer live in the JS and continue to read the CSS as it expects.

## Settings popover

- Window: 390 × 680.
- Window entrance: `320ms` spatial spring on the shell only. No per-child stagger.
- View transitions: simple opacity crossfade with a 3px translateY. No scale, no translateX.
- Live dot: 2.4s opacity breathe, 0.55↔1. No scale wobble, no glow.
- Toggles: thumb `translateX(16px)` over `200ms var(--ion-spring)`, no overshoot, no halo.
- Save success: `700ms` `data-state="saved"` label swap to a CSS-drawn check + "SAVED", then revert. No radial flash, no bounce.
- Inputs: 1px `var(--ion-border)`, focus = solid `--ion` border + native outline. No box-shadow halo.
- Scrollbar: 6px thumb at `rgba(255,255,255,0.06)`, transparent track, brief `--ion-rgb` 0.32 tint on hover.
- Detector enable/disable: `data-detector-state="disabled"` opacity 0.55. No sliding, no overshoot bounce.

## Performance

- Target 60 FPS. Animate `transform` and `opacity` only.
- Avoid continuously animating: `blur`, `box-shadow`, `background-filter`, large gradients.
- For glow: use pseudo-elements and animate opacity. Do not animate `box-shadow`, `filter`, or `backdrop-filter`.
- Most of the interface is black/dark neutral. Most pixels do not move.

## Forbidden patterns

These are explicitly removed from the codebase and must not return:

- Spinning circles, continuous 360° rotation, radar sweeps, scan lines.
- Strong pulsing, bounce easing, elastic easing, overshoot easings.
- Giant green borders, glowing every card, nested card-in-card.
- Excessively rounded pills on non-pill elements.
- Random grid backgrounds, particle clouds, fake data, hexagons.
- RGB-looking bloom, long stagger delays, giant gradients, floating decorations.
- Per-character text reveal, per-element entrance choreography on the alert HUD or settings window.
- Radial flash on save success, hover translateY, rotate on hover.
- Drop-shadow filter chains on the badge, waveform, or panel.

## Skill notes

For future ION Sense work, the relevant design references are:

- `impeccable` — design critique and audit.
- `motion` (motion.dev) — spring generation, performance audit, and the visual transition editor.
- `21st-ui-build / 21st-ui-review / 21st-ui-explore` — design reference library.
- `ui-ux-pro-max` — interaction quality, accessibility, UX heuristics.
- `frontend-design` — art direction, typography, spacing, component composition.

These skills are installed at `~/.claude/skills/` and in the local `.agents/skills/` and `skills/` directories. They are tool installations for the local agent harness, not part of the product, and are not committed to the repository.
