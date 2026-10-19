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
  views (unverified).
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
  badges for a game mode.
- Strings are hypothetical (about the Planck length); label them as such.
- **Status 2026-10-15: built** (off by default, key C): screen and set-as-object anchors
  (physical), light-crossing times and probe energies, the unexplored band, big numbers and
  ladders past the Planck length, and a relative "zoom range = cosmos" metaphor that also travels
  back in cosmic time. As built: [[decisions/controls]] "Cosmic size scale". Not built: icons
  (coloured dots instead), depth badges, the panel in exported videos (overlays never enter
  exports).

## How big is big? (2026-10-15)

User: fascinated by googol, googolplex, Graham's number, TREE(3); how does our 10^3000 compare?
Direct comparison is hopeless, so compare the kind of size: one dial per Knuth up-arrow, as a
gear train after Ganson's "Machine with Concrete".

- **Status 2026-10-15: built, then removed the same day.** User: "It's an own window, which
  is not like the rest; way too much text; not sure if people understand it." Panel, window,
  menu item, control-center button, the cosmic scale's link and the Rust maths are gone. The
  convention, numbers and sources stay in [[synthesis/how-big-is-big]] for achievements later.

## Money ladder for the relative mode (2026-10-15)

User: a third ladder next to size and time: 1 cent at home (zero has no place on a log scale)
up to world GDP at the 1e3000 limit, with milestones (coffee, salary, car, house, jackpot, big
company, a country's GDP) and "the richest person in the world" as a role, not a name. Every
value carries a year and a source (Forbes / World Bank / IMF); the richest person's figure
goes stale fastest. Only 16 decades, so it needs the relative mode's milestone spacing. Build
after the size/time relative mode has been tried.

## 3D worlds: Mandelbox first, as a separate mode (2026-10-19)

User asked which other fractal types almondbread could support, including the 3D Mandelbulb and
Mandelbox once considered. Researched in [[synthesis/other-fractal-types]] §2c; plan page
[[plans/3d-worlds]].

- **Verdict: not recommended now.** If wanted, a separate "3D worlds" mode with its own renderer
  and controls, Mandelbox first (conformal folds, scalar DE, architectural spaces to fly
  through), then the Mandelbulb. XL: two to four months of agent work.
- No perturbation scheme for 3D fractals is published (searched 2026-10-19: fractalforums,
  mathr, Mandelbulber), so depth has a hard ceiling: float32 ray marching ~1e4–1e5
  (low-reliability source, agrees with our landscape estimate), double-float ~1e10–1e12 at
  7–12× the cost (inference). That is the opposite of almondbread's 1e-3000.
- Frame cost (estimate, unmeasured): ~0.1–0.2 Mflop per pixel, 80–150 ms per 2560×1600 frame on
  an M1 Pro; 60 Hz needs a quarter of the pixels plus temporal upscaling (MetalFX); 120 Hz at
  full quality is not realistic; an iPad has about half to two thirds of that GPU.
- Reuse only ~20–30 % (Metal host, frame loop, palettes, overlay text, UI shell); the deep-zoom
  engine (references, BLA/LA, scheduler levels, interior probe, seek, π, journeys) does not apply.
- Crowded niche (shadertoys, Mandelbulber, App Store viewers; count unverified) without our
  depth advantage.
- **Cheaper route:** the 2.5D landscape ([[decisions/landscape-mode]] L0–L2) already flies at
  118–120 fps, keeps perturbation and unlimited depth (its 1e-12 limit is a prototype choice).
  Revisit true 3D only if the user wants the 3D look more than depth.

## Earlier ideas already filed elsewhere

- Landscape mode with altitude as zoom, flight recorder and rewind:
  [[decisions/landscape-mode]].
- Game mechanics (atlas, daily challenge, canyon racing, depth missions,
  journey mode) and store distribution: kept in private notes, not in this wiki.
