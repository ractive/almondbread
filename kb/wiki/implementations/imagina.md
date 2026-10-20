---
title: "Imagina"
type: implementation
status: draft
license: "AGPL-3.0"
sources: ["sources/imagina-readme", "sources/imagina-hinfla-evaluator", "sources/muency-bla", "sources/fractalshark-readme", "sources/kf-page"]
tags: [implementation, linear-approximation, rebasing, floatexp, cpu-simd]
---

# Imagina

Zhuoran's (GitHub 5E-324) "fast fractal renderer & explorer", Windows,
C++, depends on MPIR and libpng (src: [Imagina README](../sources/imagina-readme.md)). Zhuoran
invented rebasing and BLA/LA on fractalforums and later collaborated with
mathr (src: [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)).

## Algorithms

- Perturbation with rebasing.
- `HInfLAEvaluator`: up to 256 LA stages whose step boundaries follow the
  reference's near-returns to 0 (period detection), ~16x coarsening per stage
  when no period is found; each LA step does its first iteration non-linearly,
  validity tested on Chebyshev norms of `dz(2Z+dz)` and `dc` with thresholds
  scaled by 2^-24; O(1) lookup by walking a stage and descending on failure;
  derivatives propagate through LA steps
  (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)). Full description in
  [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md).
- AT: for a periodic reference, map the pixel into renormalised minibrot
  coordinates and iterate plain `z <- z^2 + c` in low precision, one step per
  period (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)).
- Feature finder (double click finds a nearby feature), reference-orbit
  periodicity detection and orbit compression for save files; FractalShark
  took its periodicity detection and compression approach from Imagina
  (src: [Imagina README](../sources/imagina-readme.md); [FractalShark README](../sources/fractalshark-readme.md)).
- JIT formula evaluator (`JitEvaluator.cpp`, inferred from filenames only).

## Number types and GPU

double where coefficient range permits, FloatExp otherwise, decided per LA
stage; 4-wide SIMD vectors with masks on CPU. No GPU backend is documented
(src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md); [Imagina README](../sources/imagina-readme.md)).

## Performance (attributed)

No published numbers. KF's page recommends it as a BLA-capable renderer
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)); FractalShark's author expects Imagina "will
probably beat it on performance in the long term"
(src: [FractalShark README](../sources/fractalshark-readme.md)).

## Status

Last push to the public repo 2024-01-23 (gh api, 2026-10-14). README: "The
project is being rewritten. This repository may be renamed and replaced"
(src: [Imagina README](../sources/imagina-readme.md)). 61 commits, no releases (notes:
implementations_and_formulas §Q1).

## What almondbread can learn

The fastest known skip method for power-2 Mandelbrot (LA + AT), worth porting
to Metal as roadmap step 9. AGPL: implement from our own description in
[Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md), never from the code.

# Citations

- [Imagina README](../sources/imagina-readme.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
- [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)
- [FractalShark README](../sources/fractalshark-readme.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
