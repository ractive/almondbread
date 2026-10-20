---
title: "Chéritat: Mandelbrot set (wiki-draw rendering techniques)"
type: source
authors: ["Arnaud Chéritat"]
year: 2020
raw: "web/cheritat-mandelbrot-rendering.html"
status: digested
tags: [interior-detection, distance-estimation, smooth-coloring, coloring-methods, potential, anti-aliasing]
---

# Chéritat: Mandelbrot set (wiki-draw rendering techniques)

**URL:** <https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set> · **Saved:** `raw/web/cheritat-mandelbrot-rendering.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed (page footer says content under Creative Commons Attribution)

## What it is

Rendering-techniques page by Arnaud Chéritat, a complex-dynamics mathematician (Toulouse), on
"Techniques for computer generated pictures in complex dynamics". Covers escape time, the potential
and log-potential colouring, derivative-based interior detection, Milnor and Henriksen distance
estimators, and normal-map shading, with Python-like pseudocode. Last modified 30 Oct 2016 per the
footer (manifest says 2020).

## What it covers

- Escape radius: `|z| > 2` decides escape for `z^2 + c`; using a larger `R` (e.g. 1000) makes bands follow equipotentials.
- Potential `V(c) = lim log+|P_c^n(0)| / 2^n`; when `|z| > R` after `n` steps, `V ~ log|z| / 2^n`, relative error of order `1/R`. Gives continuous colouring.
- Deep zooms: colour by `log V` (zooming near tips/branch points shifts `log V` by a constant); periodic palette `g(log V / K)`, `K = log 2` a natural start, larger `K` in "furry" deep zooms; no single `K` fits all filament scales.
- Interior detection: iterate `der <- der * 2z` alongside `z` starting from `z = c`, `der = 1` (i.e. `(P^n)'(c)`), and declare interior when `|der| < eps`. Order matters: update `der` before `z`. Credited to Xavier Buff. False positives are rare and close to M; problems concentrate at parabolic parameters (`eps = 0.1` visibly wrong near `c = -3/4`).
- Speed: each iteration costs about twice as much; overall only ~1.5x faster for M at a well-tuned `N`, but the real gain is that a too-large `N` (e.g. `1e6`) costs little, so `N` needs less retuning when zooming. Example uses `eps = 0.001`, `R = 10`, `N = 10000`.
- Milnor DE: `d_n = 2 |z_n| log|z_n| / |rho_n|`, `rho_{n+1} = 2 z_n rho_n + 1`; true distance `d` satisfies `1/5 < d / lim d_n < 1` for `|c| < 5.7` (never better than 1/4). Boundary drawn where `d_n < thickness * pixel_size`, compared in squares to avoid square roots.
- DE as cheap antialiasing: blend boundary and outside colours by `t = d / (thickness * pixel)`, clamped to 1.
- Henriksen DE: track the image of a pixel-sized disc (`der <- 2 z der + dc` with `dc` = pixel size times thickness) and mark boundary when `|z| < |der|` (orbit passes near the critical point); `R = 10`, thickness <= 0.25. Variant testing `z + c` (i.e. `P^n(0) = -c`) avoids marking component centres; spherical-metric variant.
- Normal-map shading: normal from `u = z / der` normalised, Lambert with light height `h2 = 1.5`; variant using the DE's level sets needs `d^2z/dc^2` (`der2 <- 2 (der2 z + der^2)`).
- Mentions Triangle Inequality Average (Kerry Mitchell) and prefers Stripe Average (Härkönen 2007).

## Reliability

Secondary/tutorial but written by a research mathematician; maths statements are careful (e.g.
DE bounds, Fatou's conjecture). Note the factor 2 in Milnor's `d_n` appears in the Principle section
but is dropped in the later normal-map variation formula (affects scale only). Double-precision,
non-perturbative; deep-zoom applicability of the interior trick is shown in
[fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](fractalforums-interior-detection.md).

## Useful sections

- §The potential — `V(c)` and continuous colouring.
- §Deep zooms and log-potential scale — `log V / K` periodic colouring.
- §Following the derivative — `(P^n)'(c)` iteration and the update-order pitfall.
- §The idea — interior criterion `|der| < eps`.
- §Images and speed — cost/benefit, `eps` choice, parabolic failures.
- §Milnor's (§Principle, §Implementation) — exterior DE and bounds.
- §Variation: (partial) antialias effect without oversampling — DE blending.
- §Henriksen's — disc-tracking boundary detection and variants.
- §Normal map effect — slope shading.
- §Radial strands — TIA vs stripe average.

## Used by

[Interior detection](../techniques/interior-detection.md), [Distance estimation](../techniques/distance-estimation.md), [Smooth (continuous) iteration count](../techniques/smooth-coloring.md),
[Colouring methods](../techniques/coloring-methods.md), [Anti-aliasing and supersampling](../techniques/anti-aliasing.md), [Automatic maximum-iteration control](../techniques/auto-iterations.md)

# Citations

- <https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set>
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](fractalforums-interior-detection.md)
