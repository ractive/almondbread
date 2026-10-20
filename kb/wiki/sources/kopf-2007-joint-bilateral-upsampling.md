---
title: "Kopf et al. 2007: Joint Bilateral Upsampling"
type: source
authors: ["Johannes Kopf", "Michael F. Cohen", "Dani Lischinski", "Matt Uyttendaele"]
year: 2007
raw: "web"
status: digested
tags: [reconstruction, upsampling, bilateral-filter, graphics-paper]
---

# Kopf et al. 2007: Joint Bilateral Upsampling

**URL:** <https://johanneskopf.de/publications/jbu/paper/FinalPaper_0185.pdf> (ACM TOG 26(3), doi 10.1145/1276377.1276497) · **Saved:** not saved · **Terms:** ACM copyright; author's copy

## What it is

SIGGRAPH 2007 paper: compute an expensive solution at low resolution, then
upsample it with a bilateral filter whose range weights come from a cheap
full-resolution guide image.

## What it covers

- Formula: `S~_p = (1/k_p) * sum over q in low-res window of S_q * f(|p↓ - q↓|) * g(|I_p - I_q|)`,
  `f` a spatial Gaussian on the low-res grid, `g` a range Gaussian on the
  full-res guide `I`.
- Cost O(N r²) for N output pixels and filter radius r, independent of the
  upsampling factor. 5x5 Gaussian, σ_d = 0.5. About 2 s per megapixel on a 2007
  CPU.
- Works for tone-mapping exposure maps, colourisation chroma, stereo depth and
  graph-cut labels: solutions that are piecewise smooth with edges that match
  the guide's edges. Tested up to 32x32 upsampling.

## Reliability

Primary, widely used. The method needs a guide whose edges coincide with the
solution's edges. For fractal iteration data the expensive quantity is itself
the only full-resolution signal, so a cheap guide must be invented (inference).

## Useful sections

- §3 Joint Bilateral Upsampling: the formula.
- §5 Performance and Accuracy: cost, filter sizes.

## Used by

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md)

# Citations

- <https://johanneskopf.de/publications/jbu/paper/FinalPaper_0185.pdf>
