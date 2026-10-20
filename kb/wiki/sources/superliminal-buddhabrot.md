---
title: "Melinda Green: The Buddhabrot Technique"
type: source
authors: ["Melinda Green"]
year: 1993
raw: "web"
status: digested
tags: [buddhabrot, orbit-density, nebulabrot, metropolis-hastings]
---

# Melinda Green: The Buddhabrot Technique

**URL:** <http://superliminal.com/fractals/bbrot/> · read 2026-10-19, not saved

## What it covers

- Not a new fractal but "a different way" of displaying the Mandelbrot set (Green, 1993).
- Algorithm: random `c`, iterate; discard points that do not escape; replay escaping orbits and
  increment a counter for every pixel `z_n` visits; the hit-count image converges.
- Images depend strongly on the iteration limit; Nebulabrot = three greyscale renders at limits
  500, 5000, 50000 mapped to blue, green, red.
- Cost: half an hour for a low-resolution animation on a P233; a close-up took "an entire long
  weekend".
- "Not possible to zoom arbitrarily deeply"; close-ups were crops of large renders. Alex
  Boswell's Metropolis–Hastings sampling greatly speeds up zoomed regions.

## Reliability

Primary (the originator). Old hardware numbers.

## Used by

[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
