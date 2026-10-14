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

## For almondbread

Interactive: one centre sample, then progressive uniform low-discrepancy jitter
(1 px box) accumulated in linear light, reset on camera change. Always apply DE
coverage for filaments. Export: N subframes (16–64) with jitter, gamma-correct
resolve. Adaptive extra samples by DE in a later step. Related:
[[techniques/frame-reuse-and-progressive]], [[techniques/coloring-methods]].
