---
title: "fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)"
type: source
authors: ["fractalforums.org members"]
year: 2022
raw: "web/fractalforums-interior-detection.html"
status: digested
tags: [interior-detection, bla, perturbation, extended-range-floats, benchmarks, forum]
---

# fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)

**URL:** <https://web.archive.org/web/20221120133203/https://fractalforums.org/fractal-mathematics-and-new-theories/28/interior-detection-for-deep-mandelbrot-set-zooms/4802> · **Saved:** `raw/web/fractalforums-interior-detection.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Archived single-page thread started by GBy (fractalshades author) on 25 June 2022: interior
detection via the derivative `dz/dZ_1` combined with perturbation and BLA, with replies by claude
(Claude Heiland-Allen) including benchmarks of his experimental BLA renderer (later Fraktaler 3)
and a speed table of CPU number types.

## What it covers

- Problem: a view with a large minibrot (period 14292, nucleus size `7.35e-33`) needs iteration counts of hundreds of periods (10 million here); all interior pixels would run to the limit.
- Method (after Chéritat's page): iterate the derivative w.r.t. `z_1` and stop when its modulus drops below `eps`. Reference part `dZ_{n+1}/dZ_1 = 2 Z_n dZ_n/dZ_1` at standard precision. Under a BLA step `z <- A z + B dc`, the derivative updates as `dz/dz_1 <- A dz/dz_1`.
- `eps = 0.001` (or even 0.01) seems safe; only worth enabling when a minibrot is in view.
- Deeper example: period 57853 at `dx = 5.07e-433`, 400 million iteration limit, 3200x3200 in under 5 minutes in fractalshades (i5-7300HQ, 4 cores).
- claude (Reply #1): his renderer reports per-image statistics (steps, BLA steps, perturbation steps, iterations, max reference iteration used, rebases due to smallness vs running out of reference, speedup factor vs perturbation only) to diagnose too-small or too-large limits.
- claude (Reply #6): same location, perturbation + BLA with x87 long double, no interior detection, 3200x3200, 1 jittered sample, 400,100,100 iterations, 10100 max perturbation steps, period+1 reference iterations: about 13 min, 16 threads.
- claude (Reply #7) number-type speed table (his desktop, higher = faster; exponent/mantissa bits): float 269,947,109 (8/24); double 255,631,760 (11/53); long double 143,128,704 (15/64); softfloat 8,168,234 (31/32, int+int); floatexp 4,307,958 (24/24, float+int). Same render: long double 13 min, softfloat 30 min, floatexp 55 min.
- GBy (Reply #8): fractalshades uses floatexp (int32 + float64) below `1e-300`.
- claude (Reply #9): Dinkydau's Flake at 2400x2400: fractalshades 14-16 s; KF 2.15.5.2 CPU 7.5 s (SA, no BLA), KF OpenCL GPU 6.6 s; his new program CPU 5 s, GPU (OpenCL) 3 s. KF with NanoMB1 + interior checking took 20 min on the `e432` mini; fractalshades 2m12s. uint32+uint32 software float about 2x faster than float32+int32 for basic arithmetic.
- claude (Reply #11): implemented it by extending dual numbers from 2 to 4 parts, initialising the extra part at `z_1`, comparing the operator norm of the `d/dz_1` matrix with inscape radius `1/1024`. Times on the `e432` mini: long double 13m -> 2m35, softfloat 30m -> 6m20, floatexp 55m -> 8m39 (about 5x). Also works for interior regions of some hybrids.
- claude (Reply #12): derivation that `d(Z+z)/d(Z_1+z_1)` reduces to `dz/dz_1` when `C` is a nucleus (`Z = 0` in the orbit); asks what to do otherwise. Alternative: cap non-linear perturbation steps at a few thousand; dropping max perturbation iterations to 2048 gave 3x speedup with similar image quality.
- marcm200 (Reply #5): multiplier-based sector filling of the main cardioid (radial rays to multiplier 0.3 ... 0.95, fill convex sectors).

## Reliability

Primary forum discussion between two renderer authors; numbers are single-location, single-machine
measurements (claude's machine is the Ryzen 7 2700X named in [[sources/kf-page]], inferred). The
number-type table is CPU-only and says nothing about GPU float32. The `dz/dz_1` justification
is stated to hold only for nucleus references; general case left open.

## Useful sections

- §Interior detection for deep Mandelbrot set zooms (opening post, GBy) — method, BLA rule for the derivative, `eps`.
- §Reply #1 — renderer statistics worth collecting.
- §Reply #6 — long double BLA baseline timing.
- §Reply #7 — CPU number-type speed table.
- §Reply #9 — Flake timings, KF vs fractalshades vs new program, CPU vs OpenCL.
- §Reply #11 — dual-number implementation, inscape radius 1/1024, 5x speedup.
- §Reply #12 — `dz/dz_1` derivation and the perturbation-step cap alternative.

## Used by

[[techniques/interior-detection]], [[techniques/bla]], [[techniques/extended-range-floats]],
[[implementations/fraktaler-3]], [[implementations/fractalshades]], [[implementations/kalles-fraktaler-2]],
[[questions/bla-vs-la-benchmark]]
