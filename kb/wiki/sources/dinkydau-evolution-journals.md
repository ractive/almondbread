---
title: "Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals"
type: source
authors: ["Dinkydau (DinkydauSet)"]
year: 2019
raw: "web"
status: digested
tags: [julia-morphing, shape-stacking, evolution, newton, places]
---

# Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals

**URLs** (read 2026-10-17, not saved; facts only):

- "Newton-Raphson zooming and Evolution zoom method" (DeviantArt journal, 2017-01-13)
  <https://www.deviantart.com/dinkydauset/journal/Newton-Raphson-zooming-and-Evolution-zoom-method-657051421>
- "Simulating Julia morphings" (2019-04-09)
  <https://www.deviantart.com/dinkydauset/journal/Simulating-Julia-morphings-793105862>
- ExploreFractals readme <https://github.com/DinkydauSet/ExploreFractals>

## What it is

Notes by one of the best-known deep-zoom artists (the "Evolution of trees" series, the 2013
e1500 zoom; see [[sources/deep-zoom-videos]]) on how deep shapes are built by hand.

## What it covers

- **Evolution method**: double the shape, morph one of the two copies, repeat with the
  morph as the new shape; each round keeps the original and every stage visible ("evolution
  set"); "the more steps taken, the better", and the look converges.
- **Depth rules of thumb**: doubling = zoom to about **3/4 of the depth** (in log
  magnification) of a chosen minibrot outside the shape; a morph uses a minibrot inside the
  shape. Simulation post: a view at depth `2^n` containing a minibrot of depth `2^m` looks,
  at depth `2^((n+m)/2)` (halfway in log), like a Julia morphing `c → c² + c_mini` of its
  shape; repeated morphs with the same minibrot keep averaging the depth towards the
  minibrot's. The approximation improves with depth; near the minibrot the morph becomes a
  ring "highly deformed by the minibrot".
- **Newton-Raphson zooming** (Claude's algorithm in KF): finds the nearest minibrot in a
  region and its depth from "an 'accurate enough' guess" without rendering; usually faster
  than zooming by hand, slower at high iteration counts, can run unattended and in parallel.
  It does not choose a location for you.
- **ExploreFractals**: previews morphs by transforming the plane (inflections) before
  rendering; "an inflection halves the distance (the exponent in the magnification factor) to
  deeper shapes"; "Deeper shapes are Julia transformed versions of lesser deep shapes at the
  same zoom path". Not for deep perturbation renders.

## Reliability

Practitioner knowledge, consistent with [[sources/muency-newton-raphson-zooming]] (`s^(3/4)`)
and [[sources/heiland-allen-inflected-julia-sets-2025]] (half the zoom again per morph).

## Used by

[[synthesis/finding-deep-places]]
