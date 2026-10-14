---
title: "Mandelbrot set (power 2)"
type: formula
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/muency-bla]]", "[[sources/imagina-hinfla-evaluator]]"]
tags: [mandelbrot, formula, perturbation, bla, rebasing]
---

# Mandelbrot set (power 2)

The baseline formula and the only one almondbread v1 must do perfectly. Every
deep-zoom technique in this wiki was first worked out for it.

## Iteration

`z_{n+1} = z_n^2 + c`, `z_0 = 0` (the critical point). Escape when `|z| > R`;
use `R >= 256` for smooth colouring (see [[techniques/smooth-coloring]]).
Real form: `x' = x^2 - y^2 + cx`, `y' = 2xy + cy`.

## Perturbed form

Reference `Z_{m+1} = Z_m^2 + C` in high precision; pixel `Z_m + z_n`,
`C + c`. Expanding `(Z+z)^2 + (C+c)` and cancelling the reference terms
symbolically gives

`z_{n+1} = 2 Z_m z_n + z_n^2 + c = (2 Z_m + z_n) z_n + c`

The cancellation must be done on paper, not in floats: computing
`(Z+z)^2 + C + c - Z_{m+1}` loses `z` by catastrophic absorption
(src: [[sources/mathr-deep-zoom-theory]] §Perturbation). Details in
[[techniques/perturbation]].

## Rebasing

One critical point (0), so one reference suffices. After each step, if
`|Z_m + z_n| < |z_n|` or the reference is exhausted, set `z <- Z_m + z`,
`m <- 0` (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Another
solution to perturbation glitches; [[sources/mathr-deep-zoom-page]] §2.1
Rebasing). Zhuoran reported correct single-precision renders at depth with
5 million iterations using this rule alone. See [[techniques/rebasing]].

## BLA / LA form

- Jacobian of one perturbed step with respect to `z`: `2Z` (complex,
  conformal), so `A_{m,1} = 2 Z_m`, `B_{m,1} = 1`.
- Single-step radius: `r = max(0, eps (|Z| - max|c|) / (2|Z| + 1))`,
  eps = 2^-24 for float32; mathr halves `|Z|` for safety
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- Merge `y after x`: `A = A_y A_x`, `B = A_y B_x + B_y`,
  `R = min(R_x, max(0, (R_y - |B_x| max|c|)/|A_x|))`.
- Imagina-style LA (non-linear first step, period-aligned stages, AT) is
  Mandelbrot-specific and the fastest known
  (src: [[sources/imagina-hinfla-evaluator]]). See [[techniques/bla]] and
  [[techniques/linear-approximation-imagina]].

## Derivatives

- c-derivative for distance estimation: `dc' = 2 (Z+z) dc + 1`
  (scaled by pixel spacing in practice); see [[techniques/distance-estimation]].
- z-derivative for interior detection: `dz' = 2 (Z+z) dz`, stop when
  `|dz| < 1e-3` (src: [[sources/mathr-deep-zoom-page]] §6 Interior
  Detection); see [[techniques/interior-detection]].

## Rescaling

`z = S w`, `c = S d` gives `w <- 2 Z w + S w^2 + d`. Full floatexp
iterations are needed only when `|Z|` itself is tiny, which near a
periodic reference happens once per period
(src: [[sources/mathr-deep-zoom-theory]] §Rescaling, §Full Iterations).
No "deep needle" problem: that is an abs-formula issue
(see [[formulas/burning-ship]]). See [[techniques/extended-range-floats]].

## Support in renderers

Every deep zoomer: [[implementations/kalles-fraktaler-2]] (SA, NanoMB1),
[[implementations/fraktaler-3]] (BLA), [[implementations/imagina]] (LA+AT),
[[implementations/fractalshark]] (LA on CUDA), [[implementations/fractal-zoomer]],
[[implementations/fractalshades]], [[implementations/swift-mandelbrot]],
[[implementations/mandelbrot-webgpu]].

## Verdict for almondbread

Ship first. Pipeline: CPU reference (nucleus when found, see
[[techniques/reference-selection]]), float32 rebasing kernel, BLA tree,
rescaled float32 past ~1e-25, then Imagina-style LA+AT
(see [[synthesis/deep-zoom-rendering-techniques]] roadmap).
