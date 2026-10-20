---
title: "Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)"
type: source
authors: ["Christoph Schied", "Anton Kaplanyan", "Chris Wyman", "Anjul Patney", "Chakravarty R. Alla Chaitanya", "John Burgess", "Shiqiu Liu", "Carsten Dachsbacher", "Aaron Lefohn", "Marco Salvi"]
year: 2017
raw: "web"
status: digested
tags: [reconstruction, temporal-accumulation, a-trous, denoising, graphics-paper]
---

# Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)

**URL:** <https://cg.ivd.kit.edu/publications/2017/svgf/svgf_preprint.pdf> (HPG 2017, doi 10.1145/3105762.3105770) · **Saved:** not saved · **Terms:** ACM copyright; authors' preprint

## What it is

HPG 2017 best paper: real-time reconstruction of 1 sample-per-pixel path
traced images by temporal accumulation plus a variance-guided, edge-avoiding
à-trous wavelet filter.

## What it covers

- Inputs: 1 spp noisy colour plus a noise-free G-buffer (depth, normals,
  motion vectors, mesh ids).
- Temporal accumulation: exponential moving average with α = 0.2 after
  reprojection; per-pixel luminance variance from accumulated first and second
  moments, a spatial estimate after disocclusion.
- Spatial filter: five à-trous levels with growing footprint; weights from
  depth (σ_z = 1), normals (σ_n = 128) and luminance difference scaled by local
  standard deviation (σ_l = 4). Variance steers how much each pixel is blurred.
- Cost: about 4.4 ms at 1280x720 and 10.2 ms at 1920x1080 on a TITAN X
  (Pascal).
- Limitations (§6): over-blurred chrominance; detached shadows under motion;
  unstable noise at very low sample density (variance estimate unreliable);
  over-blurred specular and sharp features under motion; needs noise-free
  primary hits.

## Reliability

Primary, peer reviewed, many follow-ups (A-SVGF 2018). Built for Monte Carlo
noise with a clean geometric guide. In a fractal the high-frequency "noise" is
deterministic detail, and there is no separate clean guide (inference).

## Useful sections

- §4.1–4.2 Temporal accumulation and variance estimation.
- §4.3 Edge-avoiding à-trous wavelet transform; §4.4 Edge-stopping functions: weights and parameters.
- §5.3 Performance; §6 Limitations.

## Used by

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md)

# Citations

- <https://cg.ivd.kit.edu/publications/2017/svgf/svgf_preprint.pdf>
