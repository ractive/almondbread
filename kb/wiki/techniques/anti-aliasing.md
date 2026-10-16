---
title: "Anti-aliasing and supersampling"
type: technique
status: draft
sources: ["[[sources/kf-manual]]", "[[sources/zoomasm]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/wikibooks-fraktaler-3]]", "[[sources/fractalshark-readme]]"]
tags: [anti-aliasing, jitter, supersampling, progressive]
---

# Anti-aliasing and supersampling

The Mandelbrot boundary has detail at every scale, so one sample per pixel
aliases: moiré in the exterior bands, missing or sparkling filaments.

## What existing renderers do

- **KF2+ jitter.** A non-zero jitter seed enables a PRNG offset per pixel;
  jitter scale is in pixels, 1 px is sensible. Uniform jitter is the default
  since 2.12.11; Gaussian is "probably best left disabled (uniform jitter looks
  better)". Images with different seeds can be stacked for AA instead of
  rendering large and downscaling; this reduces moiré at the cost of noise.
  Downscaling uses Pixman and is sRGB gamma-correct since 2.15.2; dithering
  reduces banding (src: [[sources/kf-manual]]). Directional DE (DEX/DEY) fixes
  noisy slope texture when jitter is on (src: [[sources/kf-manual]]).
- **zoomasm** has samples per pixel (reduces "sparkle") and shutter speed
  (motion blur), and recommends KF jitter to reduce moiré (src: [[sources/zoomasm]]).
- **Fraktaler 3** EXR metadata records `image.subframes = 64`: it accumulates
  many jittered subframes (src: [[sources/wikibooks-fraktaler-3]]).
- **Chéritat** renders large and downscales by an integer factor (e.g. 8000² →
  800²) with "extra attention" to a gamma-correct downscale. Pixel-centre
  sampling hides thin strands (src: [[sources/cheritat-mandelbrot-rendering]] §Images and speed).
- **FractalShark** lists GPU antialiasing (src: [[sources/fractalshark-readme]]).

## DE-based coverage (no extra samples)

`t = clamp(d / (thickness·pixel_size), 0, 1)`, lerp from boundary colour to the
exterior colour, thickness ≈ 1.414
(src: [[sources/cheritat-mandelbrot-rendering]] §Implementation). With `de_px`
already in pixels: `coverage = clamp(de_px / 1.414, 0, 1)`. Partial AA: it
widens filaments to visibility but does not fix exterior band moiré
(inference). See [[techniques/distance-estimation]].

## Progressive jittered accumulation (design, inference)

```text
on camera change: accum = 0; count = 0; render sample at pixel centre
each idle frame:
    offset = lowDiscrepancy(count)          // R2 or Halton in [-0.5, 0.5]^2
    render all pixels at centre + offset    // same reference, same skip table
    colour = colourise(n, frac, de, ...)     // per sample, in linear light
    accum += colour; count += 1
display: toSRGB(accum / count)               // RGBA16F/32F accumulation texture
```

- Each extra sample is just another delta `c` against the same reference, so it
  costs one more perturbation pass and no reference work (inference).
- Average **colours in linear light**, not iteration counts: averaging `μ` and
  colouring afterwards gives wrong colours at band edges (inference).
- **Adaptive:** only pixels with `de_px < 1–2` or high neighbour colour variance
  get more samples; compact them into an indirect dispatch (inference).
- The per-sample colourise step means palette changes re-run accumulation
  unless raw samples are kept; storing all raw samples is "too space intensive"
  for mathr's exports, which is why it uses 1 jittered sample per pixel for raw
  data (src: [[sources/fractalforums-interior-detection]] §Reply #6).

## Denoising

No fractal-specific denoiser was found. A DE- and iteration-guided bilateral
filter is possible but risks blurring filaments (inference).

## Pitfalls

- Gamma: downscale/accumulate in linear light.
- Regular-grid supersampling still moirés on periodic bands; jitter trades
  moiré for noise (src: [[sources/kf-manual]]).
- Gaussian jitter wider than a pixel blurs.

## As built in almondbread (FL: progressive adaptive AA)

Built 2026-10-14 (measured M1 Pro, release; [[synthesis/benchmarks]] FL):

- **Progressive, idle only.** After the refined frame has faded in, idle frames accumulate
  jittered samples (Halton(2,3) positions; the refined level is the pixel-centre sample 0)
  as a running mean in linear light in the history image (rgba16f;
  `mean += (s − mean)/(n+1)`). The supersampling setting is the maximum: factor² samples
  (default 9, options 1/4/9/16). Offscreen renders and exports keep the grid.
- **Adaptive.** A GPU pass lists the pixels that gain from more samples: exterior within 4
  pixels of the set by the distance estimate (all exterior without DE) and interior pixels
  with an exterior neighbour. Far exterior is smooth (and palette-filtered, see
  [[techniques/coloring-methods]]) and deep interior is flat and the most expensive to
  iterate. AA samples iterate only the listed pixels (the unresolved-list admission of the
  extension passes, `listMode 2`), at the nominal limit; unresolved samples do not count.
- **Interruptible.** AA chunks are 8 ms of GPU with at most two queued, so input waits at
  most ~16 ms; a gesture replaces the job and drops the samples. Each view's AA may take
  1.5 GPU-s in all; heavier views settle with fewer samples.
- **Pipelined.** The next sample is queued in the same refresh that composes the previous
  one (one refresh per sample instead of two).
- **Each sample's palette filter is its share of the pixel** (fixed 2026-10-16). The palette
  filter ([[techniques/coloring-methods]], palette anti-aliasing) already averages a pixel's
  colour band over the pixel's footprint. Every AA sample (and every texel of the offscreen
  supersampling grid) was filtered over the *whole* pixel and then averaged with the others:
  a second box filter on top of the first, so fine rings went to the palette's mean grey after
  AA. Now an AA sample filters over `1/√N` of a pixel (N = the AA target, 9 by default) and a
  grid texel over `1/ss`; the dense-region calm and the outline's "dense" darkening still judge
  the whole pixel. Against a 256-sample reference (2560 × 1600 at 4 × 4 grid, palette filter
  off, box-filtered to 640 × 400 in linear light; dense-5e-14, Aurora palette, calm off so the
  reference's styling is not pixel-scale dependent): AA mean |error| 0.0388 → 0.0261 and
  chroma 86 % → 97 % of the reference; the 3 × 3 grid 0.0386 → 0.0232, 86 % → 97 %; refined
  frames (one sample) bit-identical; elephant (no dense rings) unchanged (0.0095, 98 %).
  Unfiltered AA samples are not the answer either: 0.0415 and 112 % (9 samples are too few
  for the rings: noise). Averaging is in linear light throughout (rgba16f history, colorize's
  box filter), as above. (measured 2026-10-16, M1 Pro, release)
- **Measured.** Settled (refined + faded + 9 samples) 0.95–1.2 s after the last input in
  the shallow interaction scenarios at 60 Hz (refined frame itself 0.2–0.25 s); headless
  home view 0.42 s from a cold start. Views with long extension passes (the -3/4 neck)
  run their extension budget first and take longer.

## Cheaper AA samples in dense regions (R3)

In dense boundary views nearly every pixel is on the adaptive list, so the settled image costs
~9× the refined frame (dense-5e-14: 8.94×). Starting each AA sample from its pixel's orbit
with a local series approximation cuts that to 2.05× (pixel seeds alone) or 1.24× (with
quadtree tile seeds for the centre samples), with the same error against a converged reference
([[techniques/local-series-approximation]], [[synthesis/dense-region-sampling]]). Variance-stopped
AA saved 9–35 % there at a visible noise cost ([[techniques/adaptive-sampling]]).

## For almondbread

Interactive: one centre sample, then progressive uniform low-discrepancy jitter
(1 px box) accumulated in linear light, reset on camera change. Always apply DE
coverage for filaments. Export: N subframes (16–64) with jitter, gamma-correct
resolve. Adaptive extra samples by DE in a later step. Related:
[[techniques/frame-reuse-and-progressive]], [[techniques/coloring-methods]].
