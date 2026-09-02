# ION Sense interface system

## Product character

ION Sense is a quiet desktop awareness utility: premium system software first, futuristic instrumentation second. The interface should feel approximately 50% Apple/macOS, 30% ION identity, 20% Iron Man/JARVIS. When idle, almost nothing moves. When something happens, ION wakes up, expresses itself briefly, then settles back down. Future technology should look effortless, not theatrical.

## Core tokens

- Accent: `#45EE86`; bright linework: `#65F59B`; accent RGB: `69, 238, 134`.
- Primary text: `#ECF4EE`; secondary text: `rgba(220, 232, 224, 0.66)`; tertiary text: `rgba(190, 205, 196, 0.42)`.
- Base surfaces: `#030605`, `#050807`, `#070A08`.
- Hairline: `rgba(207, 244, 216, 0.09)`; active hairline: `rgba(69, 238, 134, 0.32)`.
- UI font: Inter (preferred) with Segoe UI Variable Text and Segoe UI fallbacks.
- Display font: Inter (preferred) with Segoe UI Variable Display fallback.
- Spacing rhythm: 4, 8, 12, 16, 20, 24, 32, 40, 48 pixels.
- Control height: 42–46 pixels where space allows; no interactive target below 40 pixels.
- Corner radii: small controls 8–10px, inputs/buttons 10–12px, detector sections 14–16px, main windows 20–28px. Use pills only for status badges and metadata.

## Material and depth

- Dark translucent charcoal surfaces (`rgba(8, 14, 10, 0.86)` and similar).
- 1px hairline border. No glowing thick outlines.
- 1px inset top highlight on raised surfaces.
- Soft outer shadow; no multi-drop-shadow filter chains.
- Subtle backdrop blur (12–14px) on raised surfaces only.
- Optional very subtle green tint near active components (≤ 32% alpha on borders).
- Do not outline every object in bright green. Reserve green for active states, alerts, the ION core, primary actions, the live indicator.

## Motion system

- Micro interaction: 150–160ms.
- Controls: 200ms with `--ion-spring` (cubic-bezier(.2, .7, .2, 1) — no overshoot).
- Panels and view transitions: 200–320ms.
- HUD entrance: 360ms (opacity 0→1, translateY 5px→0, scale 0.985→1).
- HUD exit: 200ms (opacity 1→0, translateY 0→-3px, scale 1→0.992).
- Settings window entrance: 320ms on the shell only (no per-child stagger).
- Save success: 700ms swap to a small check + "SAVED", then revert. No radial flash, no bounce.
- Live indicator: 2.4s opacity 0.55↔1, no scale wobble, no glow.
- Monitor core: ~1% breath on a 6s cycle; one short arc travelling 10–20 degrees. No continuous 360° rotation.
- Toggles: thumb translateX only, 200ms `--ion-spring`, no overshoot, no halo.
- Buttons: press `scale(0.985)` for 100ms. No Y-translate on hover. No elastic bounce.
- Easing: one curve (`--ion-spring: cubic-bezier(.2, .7, .2, 1)`). No overshoot, no `cubic-bezier(.22, 1.16, .36, 1)`, no `cubic-bezier(.2, .85, .3, 1.15)`.
- For glow: use pseudo-elements and animate opacity. Do not animate `box-shadow`, `filter`, or `backdrop-filter`.

## Performance and idle state

- Target 60 FPS. Animate `transform` and `opacity` only.
- Avoid continuously animating: blur, large box shadows, background filters, huge gradients.
- Most of the interface should be black/dark neutral. Most pixels should not move.
- Respect `prefers-reduced-motion: reduce`. All new animations have a reduced-motion fallback.

## Forbidden patterns

- Spinning circles, continuous 360° rotation, radar sweeps, scan lines.
- Strong pulsing, bounce easing, elastic easing.
- Giant green borders, glowing every card, nested card-in-card.
- Excessively rounded pills on non-pill elements.
- Random grid backgrounds, particle clouds, fake data, hexagons.
- RGB-looking bloom, long stagger delays, giant gradients, floating decorations.
- Per-character text reveal, per-element entrance choreography on the alert HUD or settings window.
- Radial flash on save success, click-through-goes-through-redirect, hover translateY, rotate on hover.

## Material and depth (legacy)


- Use smoked black-green glass with restrained radial green reflections.
- Borders stay one physical pixel. Depth comes from inner highlights and low-opacity shadow layers, never from thick neon outlines.
- Glow belongs to active linework, icons, and state dots. Body text does not glow.
- Full-screen edge ambience must remain quieter than the central notification.

## Motion

- Micro interaction: 150ms.
- Controls: 210ms.
- Panels and view transitions: 320–420ms.
- HUD entrance: 1200ms; HUD exit: 800ms.
- Entrances use `cubic-bezier(.16, 1, .3, 1)`; exits use a decisive ease-in.
- Hover feedback should use color, border, and shadow before movement. Avoid card lift and decorative rotation.
- Always provide a reduced-motion path with short fades and no continuous animation.

## Alert intensity

- Keep one green brand palette across `info`, `warning`, and `critical`; severity changes energy, cadence, and contrast—not hue.
- `info` is calm and slow, `warning` is more present, and `critical` may use a restrained status pulse plus a stronger perimeter response.
- Every alert exposes a quiet telemetry row derived from the existing event contract: severity, detector source, and a locale-aware timestamp.
- The five-second settled-state rail communicates remaining dwell time without competing with the event title.
- Continuous HUD motion is limited to slow SVG transforms and opacity on small layers; it pauses under reduced motion and when the document is hidden.

## Interaction and accessibility

- Preserve visible `:focus-visible` rings and keyboard order.
- Labels and helper text remain readable at the compact tray width; avoid text below 8.5px and prefer 10–13px for settings content.
- Detector state is expressed through text plus shape/color, not color alone.
- Unsaved settings use a contextual status message, detector toggles update the overview immediately, and invalid fields retain both an inline outline and a card-level error boundary.
- Iconography uses inline SVG from one rounded 24px stroke family. Do not use emoji as UI icons.
- Settings views remain mounted during transitions so values, focus context, and scroll position are not lost.

## Responsive behavior

- The settings utility uses a narrow portrait window and an internal scroll region; cards stack in one column.
- The HUD is centered, preserves its integrated badge silhouette, and scales at the existing 1180px, 720px, and 480px breakpoints.
- Test common Windows logical viewports representing 100%, 125%, and 150% display scaling. Avoid sub-pixel-dependent borders and geometry.

## HUD 2.0: the ION Sense Core (monitoring view)

- The monitoring centerpiece is an organic energy field, not a gauge: a soft-edged core whose silhouette wobbles ±2% on a 14s cycle, two small radial light fields drifting on 19s/27s cycles inside it, breathing halos, and one orbit arc travelling a full 64s circle.
- No spinning, no radar, no constant mechanical rotation, no giant orb. The I glyph stays steady inside the core as the brand anchor.
- Core states are causal, not decorative: `data-core-state="focus"` follows a real detector restart (save); `"alert"` echoes a fired test event. Energy rises through opacity/edge only, holds briefly, then collapses smoothly back to idle (`frontend/settings.html` `setCoreState`).
- All core animation is gated behind `data-window-state="entering"/"active"` — nothing animates while the window is hidden. Reduced motion freezes the core to a static composition with 140ms state fades.

## HUD 2.0: severity-connected alert core

- The alert badge shares the event's energy: arc, glyph, and orbit luminance derive from `--severity-energy` through opacity only (`.hud-toast[data-severity]`), so the left core reads as part of the event state.
- Resolve choreography: on exit the core's energy contracts a step ahead of the surface — the alert resolves rather than vanishes.
- Motion vocabulary everywhere: hover = scale (≤1.02) + color/border/luminance response (no Y-lifts, no rotation); press = scale(.97–.985); toggles = `--ease-out` translateX, no overshoot bezier; one spring family (`--ease-out` cubic-bezier(.16,1,.3,1)) across all controls.
- Micro-label floor is enforced at 8.5px in the winning CSS layers.

## HUD 2.1: layered material and per-interaction motion

- The Sense Core gains a third membrane (hairline + diagonal sheen), a bottom shadow pool and top light pool for spherical volume, and a blurred elliptical caustic light drifting beneath the membrane (31s). Depth is layered translucency, never glow stacking.
- Core alert state escalates with real event severity via `data-core-severity` (warning tightens glows; critical compresses, sharpens the membrane, double-pulses). Test events map their true severities.
- The alert conduit (core-to-content line) flares once as the alert materializes; the panel shell carries the same light-pool/shadow-pool material as the Core; the ambient screen field is severity-graded on the existing overlay surface.
- Settings uses one continuous grouped surface with hairline rows (macOS System Settings structure); inputs are recessed neutral material that turns ION only on focus; save is a ghost hairline action that strengthens when dirty.
- Motion is per-interaction: `--ease-press` (120-130ms press), `--ease-glow` (luminance), `--ease-settle` (state landings, core, 190ms toggle), entrance spring for arrival, near-linear decisive contraction for dismissal.

## Settings 3.0: flat target visual language (2026-09)

Supersedes the grouped-surface/orb settings material above for the settings utility. The HUD keeps its 2.1 material.

- One muted accent green `#34C759` (Apple system green family) across toggles, section numbers, focus rings, and the live dot; bright linework is reserved for the I-mark gradient. The deep `#2E9E44` save button (hover `#37AF4E`) is the only large green fill.
- Flat near-black canvas `rgba(12, 15, 16, .92)` with a single neutral hairline `rgba(255, 255, 255, .065)`. No cards, no rail, no raised surfaces, no glow stacking in the settings window.
- Hero: topbar I-mark lockup, centered status column, flat breathing I-mark core (19×46px stroke glyph, `i-breathe` 5.6s ±2.5% scale) instead of any 3D/energy orb; four bare metric columns under a green-tinted hairline; bottom wordmark + "Detector settings" pill nav.
- Editor: hairline-separated sections (`rgba(255, 255, 255, .05)`) with green monospace section numbers top-right; iOS-scale 40×24 flat toggles (off track neutral, on track `#34C759`, white knob, no halo); stacked full-width fields with top/bottom hairlines and green focus underline; quiet status line; solid deep-green save.
- Typography stays Segoe UI Variable with 12–13px body; section numbers are the only monospace. No emoji, no icon rows.
- Legacy accent `#45EE86` remains HUD-only; the two palettes must not be mixed within one window.
