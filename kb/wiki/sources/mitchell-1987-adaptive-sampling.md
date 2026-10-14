---
title: "Mitchell 1987: Generating Antialiased Images at Low Sampling Densities"
type: source
authors: ["Don P. Mitchell"]
year: 1987
raw: "web"
status: digested
tags: [adaptive-sampling, anti-aliasing, reconstruction, perception, graphics-paper]
---

# Mitchell 1987: Generating Antialiased Images at Low Sampling Densities

**URL:** <http://mentallandscape.com/Papers_siggraph87.pdf> (ACM: doi 10.1145/37401.37410) · **Saved:** not saved · **Terms:** ACM copyright; author's copy

## What it is

SIGGRAPH '87 paper (Computer Graphics 21(4), 65–72) on ray tracing with about
one sample per pixel plus targeted supersampling, and on reconstructing a
pixel image from the resulting non-uniform samples.

## What it covers

- Base pass: about 1 sample per pixel in a blue-noise pattern. Supersampling
  at 4–9 samples per pixel where needed; "at 9 samples per pixel, the image
  looks perfect".
- Decision per square cell of 8–9 base samples: contrast
  `C = (Imax - Imin) / (Imax + Imin)` per colour channel; supersample if any
  channel exceeds its threshold: red 0.4, green 0.3, blue 0.6 (eye most
  sensitive to green noise). Variance is called a poor perceptual measure.
- Failure mode: low-rate sampling cannot reliably detect high frequencies;
  "coarse sampling can miss minute isolated features". The example misses gaps
  in a thin shadow; the fix is doubling the base density, which doubles cost.
- Reconstruction: repeated box filters of growing width on a grid 4x finer
  than pixels (16 points per pixel area), so dense clusters of supersamples are
  normalised locally before mixing with base samples. Plain weighted averaging
  lets clusters dominate.
- Cost of the filter: about 2 CPU minutes for 640x480 on a VAX 8650, small
  next to the ray tracing.

## Reliability

Primary, widely cited, still the standard reference for contrast-driven
adaptive sampling. Thresholds are from a "prototype system", not a study. The
scene type (ray-traced objects with edges) differs from fractal texture, where
nearly every cell has high contrast.

## Useful sections

- §3 Adaptive sampling: cell test, contrast formula, thresholds, failure case.
- §4 Reconstruction from nonuniform samples: multi-stage box filter.

## Used by

[[techniques/adaptive-sampling]]
