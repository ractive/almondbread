---
title: "mathr: Adaptive super-sampling using distance estimate (2014)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2014
raw: "web"
status: digested
tags: [adaptive-sampling, anti-aliasing, distance-estimation, supersampling, mathr]
---

# mathr: Adaptive super-sampling using distance estimate (2014)

**URL:** <https://mathr.co.uk/blog/2014-11-22_adaptive_supersampling_using_distance_estimate.html> · **Saved:** not saved · **Terms:** copyright the author; the linked C99 code (`adaptive-dem.c`) is not copied

## What it is

A short blog post with a small C renderer that supersamples only the pixels a
distance estimate (DE) says are near the Mandelbrot set boundary, with timings
on a sparse view and a dense view.

## What it covers

- Colour is `tanh(DE_px)`, with DE measured in pixels (`DE / pixel_spacing`). By
  Koebe 1/4, every dark pixel is near the boundary and every pixel near the
  boundary is dark. So light pixels stay light when subdivided and need no
  extra samples.
- Rule: if `DE_px <= 4` and depth remains, split the pixel into 4 sub-pixels
  at quarter-pixel offsets, recurse with half the spacing, and average. Else
  return the colour. Escape radius 512.
- Interior (non-escaping) points return 0. Interior DE is not used (left as an
  exercise).
- Sparse view, 960x540, 1e6 iterations: depth 1–4 cost 134%, 229%, 494%, 1233%
  of the 1 spp samples (0.66 s to 8.7 s, base 0.44 s).
- Dense view, same size: 431%, 1669%, 6283%, 23466% of the base samples (base
  17 s, depth 4 = 82 min). It only beats uniform 4^d supersampling from depth 3
  up (1669% vs 1600% at depth 2; 6283% vs 6400% at depth 3).
- Efficiency depends on the apparent fractal dimension of the view; when every
  pixel qualifies, the intermediate levels make it slower than uniform
  supersampling.

## Reliability

Primary, by the author of Kalles Fraktaler 2+ and Fraktaler 3. One machine,
one run per setting, two views. The 4-pixel threshold is a fixed choice, not
derived. Exterior only.

## Useful sections

- The code listing: DE-in-pixels threshold and recursive split.
- The two timing tables: sparse vs dense break-even.

## Used by

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md)
