---
title: Ideas
type: meta
---

# Ideas

Backlog of the user's ideas as they come up, with first thoughts. Not
scheduled; picked up after the feel-and-look review ([[roadmap]]). Newest
last.

## [2026-10-14] Music generated from the landscape

Background music derived from what you fly over.

- Orbit sonification: the orbit of the point under the crosshair as
  waveform or melody (periodic points loop, chaotic ones sound noisy).
  CodeParade's "Sounds of the Mandelbrot Set" did this and got about 4M
  views ([[synthesis/fractal-app-market-research]], audience notes).
- Periods as harmony: periods of nearby minibrots
  ([[techniques/reference-selection]]) as chords or overtones; approaching
  a period-p minibrot tunes in a matching harmony.
- Landscape as score ([[decisions/landscape-mode]]): terrain height under
  the camera → pitch/register, canyon roughness → rhythm density, depth →
  slow key changes.
- Motion as tempo: flight speed sets the pace.
- Risk: raw sonification sounds like noise; everything must be quantised to
  scales with smooth transitions — a generative ambient engine fed by the
  fractal. Implementation: Rust audio (cpal) in the shared core, so every
  platform gets it.

## Cinematic effects for autopilot and fly-to (2026-10-14)

User: "When auto-zooming you may also choose to add more effects like that the
view is turning while zooming in. Maybe the color palette could change etc."

- Rotation while zooming: needs a rotation angle in the viewport (pixel → c
  mapping in the kernel, reprojection, pan directions, verify tools). Rotation
  is free per pixel; the work is plumbing everything through.
- Palette changes with depth: slow hue drift or cross-fade between looks per
  e-fold (depth tint and colour cycling exist already but cycling only runs at
  rest).
- Further candidates: gentle palette pulse to the zoom rate, slow cycling
  while flying, a vignette/glow at high speed, music-driven effects (see the
  music idea above).
- Must stay switchable (off for "pure" exploration) and work in recorded
  flight videos.
- Planned after the autopilot and control center land (they touch the same
  Navigator and UI).
- **Status 2026-10-15: built** (pilot track PJ): view rotation end to end, spin
  tied to the zoom rate, OKLab hue and phase drift with depth, slow cycling while
  flying, all off by default; together with the journey mode (planned flights
  from landmark to landmark, seeded). As built: [[decisions/controls]] "Journey",
  "View rotation", "Cinematic effects". Not built: vignette/glow at speed,
  music-driven effects, cross-fading between two palettes (the hue drift gives
  most of the look change for one palette lookup).

## Mini-Mandelbrot atlas and maths moments (2026-10-15)

User asked which unsolved questions the tool could explore. Pictures cannot settle MLC,
density of hyperbolicity or the boundary's area; experimental maths can. Written up as
[[decisions/minibrot-atlas]] (waits for go). Side idea, a "maths moment" in the app: π in
the iteration counts. At `c = -0.75 + iε` the escape count times ε tends to π, and at
`c = 0.25 + ε` the count times √ε does (Boll 1991). **Built 2026-10-15** as the π meter with
places and guided flights: sources, proofs, error orders and our precision measurements in
[[techniques/pi-in-the-mandelbrot-set]], controls in [[decisions/controls]] "π meter".

## Cosmic size scale (2026-10-15)

User: a size comparison of the set with the universe, galaxies, the Sun, Earth, humans,
atoms, nuclei, strings, shown like the mini-map at the bottom left.

- Anchor: the screen. "If this view is your 30 cm screen, the whole set is as big as X."
  Our own arithmetic (sizes unverified, need source pages: NASA, PDG): you at zoom ≈ 6, Earth ≈ 4e7,
  Sun ≈ 5e9, Neptune's orbit ≈ 3e13, a light year ≈ 3e16, the Milky Way ≈ 3e21, the observable
  universe ≈ 3e27.
- Second mode, anchor the set to an object: "if the set were the Earth, this view is an atom".
- The punchline: physics runs out. The observable universe over the Planck length is about
  1e62, so a 1e-3000 view is about 49 universe-to-Planck ladders deep. Past 1e62 the panel
  counts ladders instead of naming objects.
- Display: a log ruler with small icons in the overlay (Rust/Metal, like the mini-map), a
  marker for the current view, one line of text. Also in exported videos; possible depth
  badges for the game ([[decisions/gamification]]).
- Strings are hypothetical (about the Planck length); label them as such.

## Earlier ideas already filed elsewhere

- Landscape mode with altitude as zoom, flight recorder and rewind:
  [[decisions/landscape-mode]].
- Game mechanics (atlas, daily challenge, canyon racing, depth missions,
  journey mode): [[decisions/gamification]].
- Selling on several stores: [[decisions/distribution]].
