---
name: RapidMaker
description: "Cybercore interface for 3D print quotes and human contact."
colors:
  bg: "#0b100c"
  surface: "#141d15"
  ink: "#edf4e9"
  muted: "#a7b5a2"
  accent: "#b5ee4b"
  accent-hover: "#d0ff78"
  on-accent: "#111a08"
  soft: "#202e1d"
  line: "#3a4a34"
  danger: "#ffc2b8"
  danger-bg: "#3d2523"
  disabled-bg: "#2d3828"
  disabled-text: "#a2b199"
  control-line: "#758469"
  light-bg: "#f0f3e9"
  light-surface: "#fafcf5"
  light-ink: "#172114"
  light-muted: "#536047"
  light-accent: "#375f13"
  light-accent-hover: "#294b0c"
  light-on-accent: "#ffffff"
  light-soft: "#e0e8d5"
  light-control-line: "#758269"
  light-line: "#b8c5a8"
  light-danger: "#a23328"
  light-danger-bg: "#fff0ea"
  light-disabled-bg: "#dce3d2"
  light-disabled-text: "#5f6e50"
typography:
  display:
    fontFamily: "Oxanium Variable, sans-serif"
    fontSize: "clamp(60px,6.9vw,96px)"
    fontWeight: 650
    lineHeight: 0.98
    letterSpacing: "-.04em"
  headline:
    fontFamily: "Oxanium Variable, sans-serif"
    fontSize: "clamp(27px,3vw,38px)"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-.025em"
  title:
    fontFamily: "Oxanium Variable, sans-serif"
    fontSize: "19px"
    fontWeight: 650
    lineHeight: 1.3
    letterSpacing: "-.02em"
  body:
    fontFamily: "Manrope Variable, sans-serif"
    fontSize: "15px"
    lineHeight: 1.6
  label:
    fontFamily: "Manrope Variable, sans-serif"
    fontSize: "12px"
    fontWeight: 700
  button:
    fontFamily: "Manrope Variable, sans-serif"
    fontSize: "13px"
    fontWeight: 700
    lineHeight: 1.4
rounded:
  control: "3px"
  card: "6px"
spacing:
  base: "8px"
  field-gap: "16px"
  card-padding: "28px"
  section: "64px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "11px 18px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "11px 18px"
  button-text:
    textColor: "{colors.accent}"
    padding: "5px 0"
  button-disabled:
    backgroundColor: "{colors.disabled-bg}"
    textColor: "{colors.disabled-text}"
  field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "10px 12px"
  card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.card}"
    padding: "{spacing.card-padding}"
  material-chip:
    backgroundColor: "{colors.soft}"
    rounded: "{rounded.control}"
    padding: "6px 10px"
---
# Design System: RapidMaker

## Production chamber
The user pinned cybercore and rejected the former native selects, static colors, 2D model appearance and flat PA/TPU samples. The replacement is a linked production chamber and configurator. Dark graphite, lime signals, Oxanium display lettering, Manrope body, the existing SVG logo and semantic theme tokens remain authoritative. Alternate light mode applies to the entire page.

Taste settings for this revision: DESIGN_VARIANCE 8, MOTION_INTENSITY 8, VISUAL_DENSITY 5. Own-world aesthetic using Svelte, Tailwind/native CSS and accessible native radio semantics. No external design system is claimed.

## Layout and controls
The headline and introduction precede a two-column desktop workstation: sticky model scene left, configurator right. Below 760px there is one scene, embedded above the material controls and sticky while configuring. The material rack scrolls horizontally on mobile. Main panels are square; ordinary controls retain the 3px radius. Circular pigment samples identify physical color, not generic cards.

Material cassettes use native radio inputs with visible labels, arrow-key operation, focus outlines and selected checkmarks. The selected cassette lifts 5px in 240ms. Pigment and quality/strength selection indicators translate in 280ms using the existing strong ease-out. Color names and synthetic prices use the adapted Transitions.dev soft-text recipe; material descriptions wrap normally and update immediately to avoid long-text animation overflow. Keyboard input suppresses control transition motion. Reduced motion disables position transitions.

Quality has Standard and Dokładny. Strength has Lekka, Standard and Wzmocniona, with visible material-density diagrams and a caveat about geometry, material and print direction. These presets affect synthetic quote fixtures and the contact summary; they are not measured load ratings.

## Models and motion
Native WebGL renders locally generated, beveled triangle meshes with directional illumination, fill, rim, surface-layer shading, depth testing and a soft illustrative ground shadow. Mapbox earcut triangulates faces with holes. No uploaded geometry is parsed and no Three.js is used. Models are representative concepts, not production CAD.

- PLA: a revolved chess figurine.
- PETG: a toothed gear with center bore, hub and six openings.
- ASA: an open enclosure with mounting posts.
- PA: a rounded three-socket bracket based on the user's supplied photograph; deep bosses, stepped bores and a recessed central web.
- TPU: a hollow tire with geometric diagonal tread.

GSAP authors a 1.3s initial layer reveal, a 0.6s material-change reveal, a 0.65s color sweep, a quality texture change and a short compression/recovery for TPU. Rotation takes 55s, capped at 30fps/DPR1.5. Pause, replay and manual rotate are exposed. Manual rotation pauses automatic rotation. Offscreen and hidden tabs suspend the ongoing scene. Reduced motion renders complete static geometry and permits instantaneous manual rotation. Without WebGL, a text fallback keeps the configurator usable.

## Product truth and boundaries
Prices are explicitly synthetic and files remain local metadata. Latest-response guards prevent old quote requests from replacing new choices; the contact button stays disabled while recalculating or invalid. Contact moves focus to its heading. Map and company details remain from the supplied business sources. No boat photo, fabricated customer proof, account, checkout, live upload, or production deployment. Current development is local only at port 5173.

## Verification
Run svelte-check and Vite build, verify quote fixtures including all strength presets, keyboard radio groups, desktop/mobile layouts, scene visibility and real states. Device performance and screen-reader use require physical-device validation before production. Do not claim numerical frame-rate performance merely from the drawing cap.
