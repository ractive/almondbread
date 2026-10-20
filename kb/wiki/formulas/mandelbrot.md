---
title: "Mandelbrot set (power 2)"
type: formula
status: draft
sources: ["sources/mathr-deep-zoom-theory", "sources/mathr-deep-zoom-theory-again", "sources/mathr-deep-zoom-page", "sources/fractalforums-zhuoran-rebasing-bla", "sources/muency-bla", "sources/imagina-hinfla-evaluator"]
tags: [mandelbrot, formula, perturbation, bla, rebasing]
---

# Mandelbrot set (power 2)

The baseline formula and the only one almondbread v1 must do perfectly. Every
deep-zoom technique in this wiki was first worked out for it.

## Iteration

`z_{n+1} = z_n^2 + c`, `z_0 = 0` (the critical point). Escape when `|z| > R`;
use `R >= 256` for smooth colouring (see [Smooth (continuous) iteration count](../techniques/smooth-coloring.md)).
Real form: `x' = x^2 - y^2 + cx`, `y' = 2xy + cy`.

## Perturbed form

Reference `Z_{m+1} = Z_m^2 + C` in high precision; pixel `Z_m + z_n`,
`C + c`. Expanding `(Z+z)^2 + (C+c)` and cancelling the reference terms
symbolically gives

`z_{n+1} = 2 Z_m z_n + z_n^2 + c = (2 Z_m + z_n) z_n + c`

The cancellation must be done on paper, not in floats: computing
`(Z+z)^2 + C + c - Z_{m+1}` loses `z` by catastrophic absorption
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Perturbation). Details in
[Perturbation](../techniques/perturbation.md).

## Rebasing

One critical point (0), so one reference suffices. After each step, if
`|Z_m + z_n| < |z_n|` or the reference is exhausted, set `z <- Z_m + z`,
`m <- 0` (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Another
solution to perturbation glitches; [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §2.1
Rebasing). Zhuoran reported correct single-precision renders at depth with
5 million iterations using this rule alone. See [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md).

## BLA / LA form

- Jacobian of one perturbed step with respect to `z`: `2Z` (complex,
  conformal), so `A_{m,1} = 2 Z_m`, `B_{m,1} = 1`.
- Single-step radius: `r = max(0, eps (|Z| - max|c|) / (2|Z| + 1))`,
  eps = 2^-24 for float32; mathr halves `|Z|` for safety
  (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear Approximation).
- Merge `y after x`: `A = A_y A_x`, `B = A_y B_x + B_y`,
  `R = min(R_x, max(0, (R_y - |B_x| max|c|)/|A_x|))`.
- Imagina-style LA (non-linear first step, period-aligned stages, AT) is
  Mandelbrot-specific and the fastest known
  (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)). See [Bivariate linear approximation (BLA)](../techniques/bla.md) and
  [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md).

## Derivatives

- c-derivative for distance estimation: `dc' = 2 (Z+z) dc + 1`
  (scaled by pixel spacing in practice); see [Distance estimation](../techniques/distance-estimation.md).
- z-derivative for interior detection: `dz' = 2 (Z+z) dz`, stop when
  `|dz| < 1e-3` (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §6 Interior
  Detection); see [Interior detection](../techniques/interior-detection.md).

## Rescaling

`z = S w`, `c = S d` gives `w <- 2 Z w + S w^2 + d`. Full floatexp
iterations are needed only when `|Z|` itself is tiny, which near a
periodic reference happens once per period
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Rescaling, §Full Iterations).
No "deep needle" problem: that is an abs-formula issue
(see [Burning Ship and abs variations](burning-ship.md)). See [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md).

## Support in renderers

Every deep zoomer: [Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md) (SA, NanoMB1),
[Fraktaler 3](../implementations/fraktaler-3.md) (BLA), [Imagina](../implementations/imagina.md) (LA+AT),
[FractalShark](../implementations/fractalshark.md) (LA on CUDA), [Fractal Zoomer](../implementations/fractal-zoomer.md),
[Fractalshades](../implementations/fractalshades.md), [swift-mandelbrot (drquicksilver)](../implementations/swift-mandelbrot.md),
[mandelbrot-webgpu (Desarso)](../implementations/mandelbrot-webgpu.md).

## Verdict for almondbread

Ship first. Pipeline: CPU reference (nucleus when found, see
[Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md)), float32 rebasing kernel, BLA tree,
rescaled float32 past ~1e-25, then Imagina-style LA+AT
(see [Deep zoom Mandelbrot rendering techniques](../synthesis/deep-zoom-rendering-techniques.md) roadmap).

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
