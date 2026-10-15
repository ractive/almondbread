---
title: "Sample reuse while zooming and panning (sample cache in fractal coordinates)"
type: technique
status: draft
sources: ["[[sources/xaos-algorithms]]", "[[sources/wikipedia-xaos]]"]
tags: [frame-reuse, sample-reuse, temporal, interactive, xaos, measured]
---

# Sample reuse while zooming and panning

Keep the point samples a gesture computed, move them with the view, and compute new samples
only where the image lacks them. A sample of the Mandelbrot set is a pure function of its
position `c` (and the iteration limit), so a sample computed in an earlier frame is **exact**
in every later frame; only where it sits in the new pixel grid changes. R4 measured this
against the FL pipeline, which recomputes a fresh jittered preview every gesture frame and
reuses only the previous *image* ([[techniques/frame-reuse-and-progressive]] FL). Results
and recommendation: [[synthesis/motion-sample-reuse]].

## XaoS: approximate reuse of rows and columns

XaoS (Hubička, 1990s) keeps only the previous frame and remembers one real coordinate per
column and one imaginary coordinate per row, not per pixel (src: [[sources/xaos-algorithms]]
§Saving Previous Pixels; [[sources/wikipedia-xaos]] §XaoS). For each new frame it matches old
rows/columns to new ones by dynamic programming in linear time: a reuse costs the squared
distance from the ideal position, a fresh row/column costs `(4·step)²`, matches must stay in
order (no doubling, no swaps), at most ~8 candidates per old line (src:
[[sources/xaos-algorithms]] §Approximation Algorithm). Unmatched lines are computed fresh at
the mean of their neighbours' positions, so displayed rows and columns are **not** at their
ideal positions: XaoS trades a positional error of up to a few pixel steps for computing about
`xsize + ysize` samples per frame instead of `xsize × ysize`
([[techniques/frame-reuse-and-progressive]] XaoS). Errors show as distortion; doubling makes the
image jagged (src: [[sources/xaos-algorithms]]). Dynamic resolution computes the middle of the
largest uncomputed block first and raises rows near the zoom centre (src:
[[sources/xaos-algorithms]] §Dynamic Resolution).

## Exact positions instead of approximate ones

A GPU does not need the row/column restriction. Store each sample with its position and its
raw kernel outputs, and draw it where it really is:

- **Position**: exact. A pan/zoom maps frame A's pixels to frame B's by
  `x_B = (x_A − C)·a + C + b` (C the view centre, `a = s_A / s_B`, `b` the centre offset in B's
  pixels, taken from the `BigComplex` centres with `delta_in`, so at any depth). Store each
  sample's birth position (its texel + jitter in the birth frame, exact in float32) and the
  birth frame's map; recompute its current position from the birth frame each frame instead
  of moving it frame by frame, so float32 error does not accumulate (~1e-4 px at 3456 px,
  never growing). Prototype: positions in f64 re-expressed each frame.
- **Data**: escape count `n`, smooth offset, distance estimate, normal angle, class
  (exterior / interior / unresolved), the limit it ran to. Colour is **not** stored: it is
  re-shaded every frame, because the look depends on the DE in *current* output pixels
  (outline width, palette filter level; [[techniques/coloring-methods]] FL). FL's history
  image bakes the old pixel scale into colours; a sample cache does not.
- **Validity**: escaped samples are valid forever; derivative-proven interior samples too.
  Unresolved samples are stale once the limit has grown (prototype: evicted when the limit
  exceeds 1.25× theirs). Colour-setting changes cost nothing (re-shade).
- **Reconstruction** (each frame): Gaussian weights (σ = 0.5 px, as FL's compose) over the
  samples in the 3 × 3 neighbouring cells; fallback the nearest sample within 5 × 5. A box
  (mean of the samples inside the pixel) or the nearest sample (XaoS-like display) are worse:
  both alias (measured, synthesis page).
- **Density control**: a cell (output pixel) gets a new sample when it has fewer than `m`
  (fill policy), or the frame's iteration budget goes to the cells with the fewest samples
  first (budget policy); at most `K` samples per cell (newest kept). A zoom-in spreads the
  samples by `a²` per frame, so at a zoom rate of `r` e-folds/s and refresh `f` the steady
  state needs about `1 − e^(−2r/f)` new samples per pixel per frame for one sample per pixel:
  3.3 % at 1 e-fold/s and 60 Hz, 8 % at 2.5, 22 % at 7.6. A pan needs the exposed strip only
  (speed / width per frame). FL recomputes `ratio²` of the pixels every frame whatever the
  motion.
- **Hybrid reconstruction**: where samples are sparse (fast zoom on a small budget), blend
  the cache's reconstruction with its own reprojected previous output (Catmull-Rom, weight
  0.3 × FL's zoom-in decay), so gaps fill smoothly instead of showing the nearest sample.

## Positional error, quantified

With exact positions there is no positional error: every displayed sample sits where it was
computed; what remains is reconstruction from irregular samples, which the Gaussian handles.
The XaoS-like variant (show the sample nearest the pixel centre; compute a new centre sample
when no cached sample lies within ε of the centre, per axis) has positional error up to ε:
measured in [[synthesis/motion-sample-reuse]] (`xn-*` rows). Its aliasing, not ε, dominates.

## Interaction with temporal accumulation and AA

The cache *is* the temporal accumulation, of exact samples instead of resampled colours:
no Catmull-Rom blur per frame, no confidence heuristics. Under a pan or slow zoom it keeps
gaining samples per pixel (up to K) while moving, i.e. it anti-aliases in motion. When the
view rests, the cached samples inside each pixel are legitimate jittered AA samples for the
settled image: the idle AA (target 9 per pixel, [[techniques/anti-aliasing]] FL) can count them
and compute only the remainder.

## The "shown frame = fresh render" check

The check compares the refined (centre-sample) level with a fresh render
([[platform/render-architecture]], `displayed_frame_mismatch`); gesture frames are already
exempt. Proposal: keep the refined level computed fresh at pixel centres (the check is
unchanged), and add a **cache check**: re-iterate a random subset (e.g. 4096) of the cached
samples from scratch at their recomputed birth positions with a fresh renderer and require
identical class and `n` (bit-identical `smooth`, DE, angle). A wrong transform, a stale
sample or a mis-evicted one changes `n` in chaotic regions immediately, so the check catches
the bugs the frame check would have. Samples reused as AA samples go through the same check.

## Measured (R4)

Measured 2026-10-15 on an M1 Pro, release builds, in a deterministic simulator of five flights
against 16-sample references. Full tables are in [[synthesis/motion-sample-reuse]].

| strategy | iterations per gesture frame vs FL | quality vs FL |
|---|---|---|
| cache, one sample per pixel (fill) | pan at 1e5: 2.4 %; filament cruise: 15 %; fast dive at 7.6 e-folds/s: 32 %; seahorse dive: 36–43 % in its shallow phases | as good or better |
| cache at FL's own iterations (budget policy, hybrid fill) | 1× | ΔE 1.4–1.8× lower, equal or lower temporal error; no gain in the deep, sample-starved zoom past 1e-24 |
| XaoS-like nearest reuse (ε 0.5 px) | about the same as fill | 2–3× FL's temporal error, about 2× ΔE p99 (shimmer) |
| box reconstruction | same as fill | aliases like nearest reuse |

GPU bookkeeping, unoptimised prototype: 3.6–6.3 ms per frame at 1080p. It needs ~3× less
before the iteration savings turn into GPU time savings.

## Pitfalls

- **Use a reconstruction filter, never the nearest sample.** A sample replaced under a pixel
  moves edges by its positional error, and that reads as shimmer.
- **Iteration limit.** Samples run to their birth frame's limit. Evict unresolved samples on
  any limit growth.
- **Sparse samples.** On a starved budget the cache is grainy where FL is blurry. Fill gaps
  from the reprojected output (hybrid).
