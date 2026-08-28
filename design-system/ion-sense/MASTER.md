# ION Sense interface system

## Product character

ION Sense is a quiet desktop awareness utility: premium system software first, futuristic instrumentation second. Interfaces stay compact, legible, and calm. Green communicates activity and focus; it is never used as a full-screen decorative fill.

## Core tokens

- Accent: `#43F28A`; bright linework: `#62F69B`; accent RGB: `67, 242, 138`.
- Primary text: `#ECF4EE`; secondary text: `rgba(220, 232, 224, 0.66)`; tertiary text: `rgba(190, 205, 196, 0.42)`.
- Base surfaces: `#030605`, `#040A06`, `#080B0A`.
- Hairline: `rgba(207, 244, 216, 0.09)`; active hairline: `rgba(67, 242, 138, 0.32)`.
- UI font: Inter (preferred) with Segoe UI Variable Text and Segoe UI fallbacks.
- Display font: Inter (preferred) with Segoe UI Variable Display fallback.
- Spacing rhythm: 4, 8, 12, 16, 20, 24, 32, 40, 48 pixels.
- Control height: 42–46 pixels where space allows; no interactive target below 40 pixels.

## Material and depth

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
