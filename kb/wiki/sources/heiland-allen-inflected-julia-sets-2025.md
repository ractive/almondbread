---
title: "Heiland-Allen: Patterns in Inflected Julia Sets (2025)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2025
raw: "web"
status: digested
tags: [julia-morphing, shape-stacking, inflection, places]
---

# Heiland-Allen: Patterns in Inflected Julia Sets (2025)

**URL:** <https://2025.algorithmicpattern.org/proceedings/CVCS8B/paper.html> (Alpaca 2025
proceedings, read 2026-10-17, not saved).

## What it covers

- Julia morphing in the Mandelbrot set is costly because "each next step in Julia morphing
  typically needs half as much zooming again as the total zoom of the previous step".
  Inflection (each click wraps the current pattern two-fold around the clicked point) makes
  comparable patterns cheaply in a Julia set (the author's Inflector Gadget).
- Building blocks by where the next click lands in the shortest spiral arm: **disk**
  (inside the central node), **tree** (beside it), **line** (beyond it). Compositions:
  parallel evolution, **evolution** (copies of each construction stage shift outwards and
  shrink), higher order (period-4 component evolves three shapes), evolution of evolution.
- KF2 added Newton-Raphson zooming to minibrots and their embedded Julia sets, and a "Show
  Inflection" preview of what zooming to a point will look like.
- Inflector Gadget can snap inflection points to node centres (quantisation). Full
  automation (the computer continuing an artist's pattern) is an open question.
- In Mandelbrot morphing the binary angle length grows exponentially per step; in
  inflected Julia sets it grows linearly.

## Reliability

Primary, peer-reviewed conference paper by the main toolmaker of the community.

## Used by

[Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)
