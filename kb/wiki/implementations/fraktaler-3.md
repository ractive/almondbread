---
title: "Fraktaler 3"
type: implementation
status: draft
license: "AGPL-3.0"
sources: ["[[sources/fraktaler-3-site]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fractalforums-interior-detection]]", "[[sources/kf-page]]", "[[sources/wikibooks-fraktaler-3]]"]
tags: [implementation, bla, rebasing, opencl, hybrids]
---

# Fraktaler 3

Claude Heiland-Allen's successor to KF: "fast deep hybrid escape time
fractals". Cross-platform (Linux, Windows, Android 5+, web with
SharedArrayBuffer, slower than native); GUI with SDL2, OpenGL ES and Dear
ImGui; batch mode for large images and zoom sequences; EXR raw export
compatible with KF and zoomasm (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)). No macOS
build is listed.

## Algorithms

- Perturbation with rebasing and mathr-style binary-tree BLA; the deep-zoom
  page describes the method "as used in Fraktaler 3"
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)). BLA extensions: non-conformal
  matrix BLA, ABS-variation radius capped by |X|/2 and |Y|/2; multiple
  critical points not implemented (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
  §4.2 ABS Variation BLA, §4.4 Multiple Critical Points).
- Separate limits: total iterations, maximum reference iterations (set equal
  to iterations; too low corrupts images), maximum perturb iterations (a few
  thousand; raise if spiral centres are blobby), maximum BLA steps (default
  1024; raise if glitchy). Too-high limits risk GPU timeouts
  (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- "Locking" maximum reference iterations to a known period can speed up
  high-iteration areas (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- Interior detection via derivative threshold (the deep-zoom page's §6; EXR
  metadata shows an inscape radius ~2^-10, src: [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)).
- v3 (2025-06-10): GLSL colouring with cached tiles for recolouring, BLA skip
  levels to reduce memory, periodicity check in the reference calculation,
  iteration-count and DE histograms to inspect limits and skew. v3.1:
  histogram colouring (`getHistogram()` quantile of N), partial KFR import.
  Reference reuse across zoom-out sequences works; BLA reuse is "not generally
  applicable" because it depends on zoom depth (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- Exponential map transform for zoom-out sequences (zoom factor 2, very wide
  aspect) (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## Number types, devices, wisdom

- float, double, long double (x86), floatexp, plus doubleexp (double-precision
  floatexp, "use when single precision floatexp is not precise enough") and
  float128, both added in v3.1. "Wisdom" benchmarks each (platform, device,
  number type) and picks the fastest type with enough range and precision per
  location (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- OpenCL on CPU and GPU, together with plain CPU code; OpenCL on CPU is usually
  faster than the CPU code except ~1e300-1e4920 where x86 long double wins
  (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- Tiles: default 128x128; larger is faster (960x1080 took 1m34s vs 3 min) but
  risks OS GPU timeouts (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## Formulas

One line = power P with |X|, |Y|, -X, -Y toggles (Mandelbrot, Burning Ship,
Mandelbar); lines stack into hybrids; advanced opcode list (store, sqr, mul,
absx, absy, negx, negy, rot, add). N lines cost N x reference work and memory
(src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)). See [Hybrid formulas](../formulas/hybrids.md).

## Performance (attributed)

- mathr's prototype in June 2022 (x87 long double, BLA, no interior check):
  GBy's 5e-433 location at 3200x3200 in ~13 min on 16 threads; Dinkydau's
  Flake 2400x2400 in 5 s CPU / 3 s OpenCL vs KF 7.5 s / 6.6 s
  (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md) §Reply #6, §Reply #9).
- Number-type throughput on his desktop (iterations/s): float 270M, double
  256M, long double 143M, softfloat 8.2M, floatexp 4.3M; he found
  uint32+uint32 software floats about 2x faster than float32+int32 floatexp
  for basic arithmetic on CPU (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
  §Reply #7, §Reply #9).
- Example minibrot at 5e-433 in just over 2 min on a Ryzen 7 2700X
  (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Status

v3.1 released 2025-12-15 (28 commits since v3), AGPL-3.0, (c) 2021-2025,
active todo and bug lists (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## What almondbread can learn

Algorithms, not code (AGPL): the opcode formula model, per-device wisdom
benchmarking of number types (a natural fit for Metal pipelines per number
type), separate iteration/perturb/BLA-step limits, tile sizing under OS
timeouts ([GPU watchdog and chunked, resumable kernels](../platform/gpu-watchdog-and-chunking.md)), per-image statistics
(mathr collects mean/min/max of BLA steps, perturb steps, rebases by cause)
for diagnosing limit problems (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
§Reply #1).
