---
title: "Wikipedia: Plotting algorithms for the Mandelbrot set"
type: source
authors: ["Wikipedia contributors"]
year: 2026
raw: "web/wikipedia-plotting-algorithms.html"
status: digested
tags: [escape-time, smooth-coloring, distance-estimation, interior-detection, periodicity-checking, guessing, perturbation, series-approximation]
---

# Wikipedia: Plotting algorithms for the Mandelbrot set

**URL:** <https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set> · **Saved:** `raw/web/wikipedia-plotting-algorithms.html` (+ `.txt`; formulas are images in the `.txt`, recovered from `alttext` in the HTML) · **Terms:** CC BY-SA 4.0 (Wikipedia)

## What it is

Encyclopedia survey of classic Mandelbrot rendering: escape time, colouring (histogram, smooth,
exponential/cyclic, HSV/LCH), distance estimates, cardioid/bulb checks, periodicity checking,
border tracing, rectangle checking (Mariani-Silver), symmetry, threads, and a short section on
perturbation and series approximation. Flagged by Wikipedia as textbook-style.

## What it covers

- Escape test `x^2 + y^2 > 4`; the inner loop can be reduced to three multiplications per iteration (`x2 = x*x`, `y2 = y*y`, `y = 2xy + y0`, `x = x2 - y2 + x0`).
- Histogram colouring: four passes (iteration counts, histogram, total, cumulative normalised hue); independent of max iterations.
- Smooth colouring (normalised iteration count): potential `phi(z) = lim log|z_n| / P^n`; with large bailout `N`, `nu = n - log_P(log|z_n| / log N)`, fraction in [0,1). Example code uses `|z|^2 <= 2^16` (`N = 2^8`) and `nu = log(log|z_n|/log 2)/log 2`, `iteration + 1 - nu`, then linear palette interpolation.
- Exponential cyclic mapping `v = ((i/max_i)^S N)^1.5 mod N`; HSV `h = ((i/max_i) 360)^1.5 mod 360`; an LCH recipe that lowers chroma as luma rises to stay in gamut. Notes sRGB vs linear blending.
- Exterior DE: `b = lim 2 |P_c^n(c)| ln|P_c^n(c)| / |dP_c^n(c)/dc|`, with `dP^{n+1}/dc = 2 P^n dP^n/dc + 1`, `dP^0/dc = 1`; by Koebe 1/4 no point of M is closer than `b/4`.
- Interior DE: `b = (1 - |dz P_c^p(z0)|^2) / |dc dz P_c^p(z0) + dz dz P_c^p(z0) * dc P_c^p(z0) / (1 - dz P_c^p(z0))|`; practical problems: finding `z0` and the correct period (rounding may detect a multiple, overestimating distance).
- Cardioid test: `q = (x - 1/4)^2 + y^2`, inside if `q (q + (x - 1/4)) <= y^2 / 4`; period-2 bulb: `(x+1)^2 + y^2 <= 1/16`. No exact tests for higher bulbs.
- Periodicity checking: compare against a saved orbit point refreshed every 20 iterations (detects periods up to 20).
- Border tracing: M is full, so a closed boundary of one iteration band can be filled; animation example saves 93.16% of iterations at 1000 max iterations.
- Rectangle checking: Mariani's grid; Mariani-Silver recursive subdivision (25x25 -> 13x13 -> 7x7 -> 4x4); halving with 1.4:1 boxes (DIN). Only valid for discrete colour regions.
- Symmetry and multithreading (embarrassingly parallel tiles).
- Perturbation: `eps_{n+1} = 2 z_n eps_n + eps_n^2 + delta`, deltas in hardware floats; glitch points detected by orbit distance and recomputed from another reference. Series: `eps_n = A_n delta + B_n delta^2 + C_n delta^3 + ...` with `A_{n+1} = 2 z_n A_n + 1`, `B_{n+1} = 2 z_n B_n + A_n^2`, `C_{n+1} = 2 z_n C_n + 2 A_n B_n`. Claims around two orders of magnitude speedup.

## Reliability

Secondary, community-edited, retrieved 2026. Classic sections are well established; the colouring
sections are partly unsourced (explicit citation-needed banner) and the perturbation section is
dated (no rebasing, no BLA) and its final paragraphs on interpolated bounds and binary search are
speculative. Use primary sources for deep-zoom claims.

## Useful sections

- §Optimized escape time algorithms — three-multiply inner loop.
- §Histogram coloring — four-pass method.
- §Continuous (smooth) coloring — normalised iteration count formula and code.
- §Exponentially mapped and cyclic iterations / §LCH coloring — palette mapping recipes.
- §Exterior distance estimation — DE formula and Koebe bound.
- §Interior distance estimation — interior DE and its period pitfalls.
- §Cardioid / bulb checking — closed-form tests.
- §Periodicity checking — cheap cycle detection.
- §Border tracing / edge checking and §Rectangle checking — Mariani-Silver and variants.
- §Perturbation theory and series approximation — delta recurrence and SA coefficients.

## Used by

[Smooth (continuous) iteration count](../techniques/smooth-coloring.md), [Colouring methods](../techniques/coloring-methods.md), [Distance estimation](../techniques/distance-estimation.md),
[Interior detection](../techniques/interior-detection.md), [Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md), [Perturbation](../techniques/perturbation.md),
[Series approximation (legacy)](../techniques/series-approximation.md), [Mandelbrot set (power 2)](../formulas/mandelbrot.md)

# Citations

- <https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set>
