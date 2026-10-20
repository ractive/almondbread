---
title: "Smooth and efficient zooming and panning (van Wijk & Nuij)"
type: source
authors: [Jarke J. van Wijk, Wim A. A. Nuij]
year: 2003
raw: web
status: skimmed
tags: [navigation, zoom, animation, ui]
---

# Smooth and efficient zooming and panning (van Wijk & Nuij)

**URL:** <https://www.win.tue.nl/~vanwijk/zoompan.pdf> · IEEE InfoVis 2003, pp. 15–22 ·
**Saved:** not saved (raw: web)

## What it is

A short paper on animating a camera between two views of a 2D plane (position `u`, view
width `w`). It defines a metric on (u, w) space in which zooming out, panning and zooming
back in is cheaper than a long pan at constant zoom, and derives the optimal (geodesic)
path in closed form.

## What it covers

- The path for a pan distance `d = |u1 − u0|` and widths `w0`, `w1`, with one parameter ρ
  (the zoom/pan trade-off; user tests favoured ρ ≈ 1.42, i.e. √2):
  `b_i = (w1² − w0² ± ρ⁴ d²) / (2 w_i ρ² d)` (+ for i = 0, − for i = 1),
  `r_i = ln(−b_i + √(b_i² + 1))`, path length `S = (r1 − r0)/ρ`,
  `w(s) = w0 cosh r0 / cosh(ρ s + r0)`,
  `u(s) = w0/ρ² · (cosh r0 · tanh(ρ s + r0) − sinh r0)`; for `d = 0` a pure zoom with
  `S = |ln(w1/w0)|/ρ`.
- Constant speed along `s` gives a perceptually uniform animation; the duration can be set
  from `S`.

## Reliability

Peer-reviewed and widely used (d3-zoom's `interpolateZoom` implements the same formulas).
We use the formulas only; the derivation was not re-checked, but our implementation's tests
check the path's end points, continuity and exact landing ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Fly to").

## Used in

[Controls: keyboard, pointer and touch](../decisions/controls.md) (fly to coordinates, as built): the van Wijk part of the path, with
log-space head and tail for deep zoom.

# Citations

- <https://www.win.tue.nl/~vanwijk/zoompan.pdf>
