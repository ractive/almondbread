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

## Earlier ideas already filed elsewhere

- Landscape mode with altitude as zoom, flight recorder and rewind:
  [[decisions/landscape-mode]].
- Game mechanics (atlas, daily challenge, canyon racing, depth missions,
  journey mode): [[decisions/gamification]].
- Selling on several stores: [[decisions/distribution]].
