---
title: "Imagina"
type: implementation
status: draft
license: "AGPL-3.0"
sources: ["[[sources/imagina-readme]]", "[[sources/imagina-hinfla-evaluator]]", "[[sources/muency-bla]]", "[[sources/fractalshark-readme]]", "[[sources/kf-page]]"]
tags: [implementation, linear-approximation, rebasing, floatexp, cpu-simd]
---

# Imagina

Zhuoran's (GitHub 5E-324) "fast fractal renderer & explorer", Windows,
C++, depends on MPIR and libpng (src: [[sources/imagina-readme]]). Zhuoran
invented rebasing and BLA/LA on fractalforums and later collaborated with
mathr (src: [[sources/muency-bla]]).

## Algorithms

- Perturbation with rebasing.
- `HInfLAEvaluator`: up to 256 LA stages whose step boundaries follow the
  reference's near-returns to 0 (period detection), ~16x coarsening per stage
  when no period is found; each LA step does its first iteration non-linearly,
  validity tested on Chebyshev norms of `dz(2Z+dz)` and `dc` with thresholds
  scaled by 2^-24; O(1) lookup by walking a stage and descending on failure;
  derivatives propagate through LA steps
  (src: [[sources/imagina-hinfla-evaluator]]). Full description in
  [[techniques/linear-approximation-imagina]].
- AT: for a periodic reference, map the pixel into renormalised minibrot
  coordinates and iterate plain `z <- z^2 + c` in low precision, one step per
  period (src: [[sources/imagina-hinfla-evaluator]]).
- Feature finder (double click finds a nearby feature), reference-orbit
  periodicity detection and orbit compression for save files; FractalShark
  took its periodicity detection and compression approach from Imagina
  (src: [[sources/imagina-readme]]; [[sources/fractalshark-readme]]).
- JIT formula evaluator (`JitEvaluator.cpp`, inferred from filenames only).

## Number types and GPU

double where coefficient range permits, FloatExp otherwise, decided per LA
stage; 4-wide SIMD vectors with masks on CPU. No GPU backend is documented
(src: [[sources/imagina-hinfla-evaluator]]; [[sources/imagina-readme]]).

## Performance (attributed)

No published numbers. KF's page recommends it as a BLA-capable renderer
(src: [[sources/kf-page]]); FractalShark's author expects Imagina "will
probably beat it on performance in the long term"
(src: [[sources/fractalshark-readme]]).

## Status

Last push to the public repo 2024-01-23 (gh api, 2026-10-14). README: "The
project is being rewritten. This repository may be renamed and replaced"
(src: [[sources/imagina-readme]]). 61 commits, no releases (notes:
implementations_and_formulas §Q1).

## What almondbread can learn

The fastest known skip method for power-2 Mandelbrot (LA + AT), worth porting
to Metal as roadmap step 9. AGPL: implement from our own description in
[[techniques/linear-approximation-imagina]], never from the code.
