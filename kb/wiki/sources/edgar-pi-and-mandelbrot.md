---
title: "Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)"
type: source
authors: ["Gerald Edgar", "David Petry"]
year: 1992
raw: "web/edgar-pi-and-mandelbrot.html"
status: digested
tags: [pi, parabolic, heuristic, history]
---

# Gerald Edgar: Pi and the Mandelbrot set

**URL:** <https://people.math.osu.edu/edgar.2/piand.html> · saved
`raw/web/edgar-pi-and-mandelbrot.html` (+ `.txt`), retrieved 2026-10-15 · **Terms:** copyright
the author; personal research copy.

## What it is

Edgar's page on Boll's discovery: links Boll's post and the newsgroup chronology (1991–1996),
notes that the material went into Peitgen, Jürgens, Saupe *Chaos and Fractals* (1992,
pp. 859–862), and reproduces Edgar's alt.fractals post of 27 March 1992, which quotes David
Petry.

## Useful parts

- **Cusp heuristic (Petry):** `x = y + 1/2` turns `x² + 1/4 + ε` into `y' = y² + y + ε`; with
  steps small, `y'(n) = y² + ε`, `y = √ε tan(√ε n + C)`; start and end are consecutive poles, so
  `√ε N = π`. This is the shifted map the app's counter iterates
  ([π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md)).
- **Neck heuristic (Edgar):** the fixed point -1/2 of `z² - 3/4` has multiplier -1, so take
  f∘f (multiplier +1), shift `z = -1/2 + y`, and the time near 0 is an integral that behaves
  like π/(2r) steps of f∘f, so π/r steps of f.
- **-5/4** (Petry): at `(-1.25, ε)` the value `nε/π` jumps around but stays close to an integer
  or half-integer (proved in [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](brockmoeller-pi-everywhere.md) for a parabola of points).
- Guckenheimer & Holmes, *Nonlinear Oscillations*, §6.8 as the reference for the approach.

## Reliability

Heuristic (Edgar says so); the proofs are [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](klebanoff-pi-in-mandelbrot.md) (cusp) and
[Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](brockmoeller-pi-everywhere.md) (neck).

# Citations

- <https://people.math.osu.edu/edgar.2/piand.html>
- [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](brockmoeller-pi-everywhere.md)
- [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](klebanoff-pi-in-mandelbrot.md)
