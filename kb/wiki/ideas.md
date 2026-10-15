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

## Earlier ideas already filed elsewhere

- Landscape mode with altitude as zoom, flight recorder and rewind:
  [[decisions/landscape-mode]].
- Game mechanics (atlas, daily challenge, canyon racing, depth missions,
  journey mode): [[decisions/gamification]].
- Selling on several stores: [[decisions/distribution]].
