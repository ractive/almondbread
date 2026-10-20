---
title: "Is a higher-order (second-order) BLA worth it?"
type: question
status: open
sources: ["sources/desarso-mandelbrot-webgpu-readme", "sources/mathr-deep-zoom-theory-again", "sources/imagina-hinfla-evaluator"]
tags: [bla, iteration-skipping, contradiction]
---

# Is a higher-order (second-order) BLA worth it?

## Question

Does adding quadratic terms to each skip step (`w' = A w + B c + C w² + D w c + E c²`)
give longer valid skips that pay for the extra per-step cost on a float32 GPU,
compared with first-order BLA ([Bivariate linear approximation (BLA)](../techniques/bla.md)) and Imagina's LA
([Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md))?

## Contradictions

- The float32 WebGPU project reports a second-order extension that pushes the
  first neglected term to cubic, measured at 2.8e40: 21x faster, 96.9% of
  iterations skipped, bit-identical output (src: [mandelbrot-webgpu README (Desarso)](../sources/desarso-mandelbrot-webgpu-readme.md)).
- The iteration-skipping research notes found "no published, adopted higher-order
  method" (notes: iteration_skipping §7). mathr says higher-order Taylor terms are
  "probably necessary to get a bulletproof radius calculation" but did not
  implement them (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear Approximation).
- Both can be true: one small GPL hobby project has it; no major renderer does.
  The 21x figure is ambiguous about its baseline (versus plain perturbation or
  versus first-order BLA); check the README.

## Why it matters

Larger radii mean fewer steps per pixel, and float32's small ε = 2^-24 makes
first-order radii short. Imagina takes a different route: its first iteration in
each step is non-linear (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)).

## How to resolve

After first-order BLA works, prototype second-order coefficients and radius in the
table builder and compare steps per pixel and time on the benchmark set
([How much faster is Imagina-style LA than binary-tree BLA on a GPU?](bla-vs-la-benchmark.md)). Do not copy the GPL code; derive from the
composition rules.

# Citations

- [mandelbrot-webgpu README (Desarso)](../sources/desarso-mandelbrot-webgpu-readme.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
