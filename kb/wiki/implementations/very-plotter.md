---
title: "Very Plotter"
type: implementation
status: draft
license: "MIT"
sources: ["[[sources/very-plotter-readme]]", "[[sources/philthompson-bla]]"]
tags: [implementation, javascript, bla, series-approximation, benchmark]
---

# Very Plotter

Phil Thompson's browser Mandelbrot explorer (repo `visualize-primes`; it
began as a prime-number plotter), with perturbation and series approximation
for deep zooms (src: [Very Plotter (visualize-primes) README](../sources/very-plotter-readme.md)) and, from v0.10.0
(May 2023), an experimental "automatic with BLA" algorithm
(src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)).

## Algorithms

- Perturbation; SA with configurable term counts (algorithm names such as
  `perturb-sapx6-float`); BLA as `bla-float` or `bla-floatexp` with an epsilon
  suffix (src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)). "float" here is a JavaScript
  number, i.e. IEEE double (inference).
- BLA radius variant `r = (eps |Z| - |B| |c|)/|A|`; lookup tests the lowest
  level first and, if valid, searches from the highest level down; "merge and
  cull" drops the two finest levels (~500 BLAs kept for a 1000-iteration
  orbit) (notes: iteration_skipping §2, citing [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)).
- Automatic epsilon: binary search on the exponent of eps, rejecting any eps
  for which a test point differs from plain perturbation by more than 0.001%;
  test-point count grows with render size (src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)).
  Values found: ~1.5e-26 at float depths, ~1e-114 to 1e-122 at 1e693.
- BLA does not support stripe-average colouring (notes: iteration_skipping §7).

## Performance (author's own, M1 Mac, 7 workers, 828x587 unless noted)

| Location | Iterations | BLA | Perturbation + SA |
|---|---|---|---|
| 2.9e89 | 350k | 99 s | 1009 s (10.2x slower) |
| 4.99e152 | 2M | 358 s | 12,958 s (36.2x slower) |
| 9.5e693 (floatexp) | 350k | 3438 s | 2076 s (SA 1.66x faster) |
| 1.8e157, 1600x900, 6 workers | 50.1k | 220 s | 536 s (2.4x slower) |

(src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)). The 1e693 case is the only published
measurement where SA beat BLA; see [How much faster is Imagina-style LA than binary-tree BLA on a GPU?](../questions/bla-vs-la-benchmark.md).

## Status

Last push 2023-05-09 (gh api, 2026-10-14); MIT. Dormant.

## What almondbread can learn

The auto-epsilon search is a practical way to validate BLA radii per
location, and the benchmark table is the only public SA-vs-BLA comparison
([Bivariate linear approximation (BLA)](../techniques/bla.md), [Series approximation (legacy)](../techniques/series-approximation.md)).
