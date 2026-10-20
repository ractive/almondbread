---
title: "mathr: Practical interior distance rendering (2014)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2014
raw: "web/mathr-interior-distance.html"
status: digested
tags: [interior-detection, distance-estimation, atom-domain, newton-raphson, period-detection]
---

# mathr: Practical interior distance rendering (2014)

**URL:** <https://mathr.co.uk/blog/2014-11-02_practical_interior_distance_rendering.html> · **Saved:** `raw/web/mathr-interior-distance.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Blog post (2 Nov 2014) on computing Mandelbrot interior distance estimates in practice: find the
period from atom-domain partials, find the attracting cycle with Newton's method, test the
multiplier, then evaluate the interior DE. Introduces a "biased" scheme that postpones interior
checks using the previous pixel's outcome.

## What it covers

- Interior DE: `(1 - |dz f_c^p(z0)|^2) / |dc dz f_c^p(z0) + dz dz f_c^p(z0) * dc f_c^p(z0) / (1 - dz f_c^p(z0))|`, with `f_c^p(z0) = z0` (attracting cycle point of period `p`).
- Finding `z0` by plain iteration needs huge iteration counts; instead solve `f_c^p(z) = z` with Newton: `z <- z - (f_c^p(z) - z) / (dz f_c^p(z) - 1)`, starting from the current orbit point, up to 64 steps, tolerance `1e-10` (post has C99 code).
- Interior test: `|dz f_c^p(z0)| <= 1` (multiplier inside the unit disc).
- Period candidates = atom-domain partials: iterations `p` where `|f_c^p(0)|` reaches a new minimum; atom domains enclose hyperbolic components of the same period.
- Interior DE evaluation: one pass over the cycle accumulating `z`, `dz`, `dzdz`, `dc`, `dcdz` (recurrences given in the post).
- Benchmarks (graphs only): "unbiased" (check at every partial) beats "plain" (exterior DE only, interior iterated to max) in the default view because many interior pixels have low periods, but is much slower where no interior is visible.
- "Biased" algorithm: if the previous pixel was interior, check at each partial; if exterior, store the check inputs and only run them if the pixel reaches max iterations. Near plain speed in exterior-dominated views.
- Flaw: near double's resolution the interior DE fails badly; fix with more precision, or with perturbation given a reference at the nucleus of the dominant island (not detailed).

## Reliability

Primary, by the later KF2+/Fraktaler 3 author, but pre-perturbation era (2014) and double precision
only. The biased trick and Newton cycle-finding are sound; the deep-zoom remark is only a pointer.
Benchmark numbers are only in images, not in the saved text.

## Useful sections

The post has no subheadings; cite by topic:
- §Practical interior distance rendering (interior DE formula) — formula and Newton step for `z0`.
- §(atom domain partials paragraph) — period guesses from new minima of `|f_c^p(0)|`.
- §(biased algorithm paragraph) — postponing interior checks using the previous pixel.
- §(final paragraph) — failure near double resolution.

## Used by

[Interior detection](../techniques/interior-detection.md), [Distance estimation](../techniques/distance-estimation.md), [Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md),
[What exactly is the ball-method period recurrence?](../questions/ball-period-recurrence.md)

# Citations

- <https://mathr.co.uk/blog/2014-11-02_practical_interior_distance_rendering.html>
