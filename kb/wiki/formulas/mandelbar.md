---
title: "Mandelbar (Tricorn)"
type: formula
status: draft
sources: ["sources/mathr-deep-zoom-page", "sources/fraktaler-3-site", "sources/kf-page"]
tags: [mandelbar, tricorn, formula, non-conformal, bla]
---

# Mandelbar (Tricorn)

## Iteration

`X + iY <- (X - iY)^2 + C`, i.e. `z <- conj(z)^2 + c`
(src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.1 Non-Conformal BLA). In
Fraktaler 3 it is the default line with the -Y toggle checked
(src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)). KF lists it too
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Perturbed form (our derivation)

`conj(Z+z)^2 - conj(Z)^2 = 2 conj(Z) conj(z) + conj(z)^2`, so

`z' = conj(2 Z z + z^2) + c`

i.e. the Mandelbrot delta step followed by conjugation, then `+c`. No
cancellation hazards beyond those of [Mandelbrot set (power 2)](mandelbrot.md) (inference).

## BLA form

The step is not complex-analytic (angles are reversed), so `A` and `B`
become real 2x2 matrices. For one step, `A` is "multiply by `2Z`, then
conjugate": with `Z = X + iY`, `A = [[2X, -2Y], [-2Y, -2X]]` acting on `(x, y)`, and `B = I`
because `+c` is not conjugated (our derivation).
Radius `R = eps inf|A| - sup|B| |c| / inf|A|`; merge
`R_z = max(0, min(R_x, (R_y - sup|B_x| |c|)/sup|A_x|))` with singular values
(src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.1 Non-Conformal BLA). For the
Mandelbar step the matrix is a scaled reflection, so `inf|A| = sup|A| = 2|Z|`
and the radius reduces to the conformal case (inference); the matrix form
matters once steps are merged with other formulas in hybrids.

Distance estimation and interior detection need dual numbers with two (DE)
or four (DE + interior) dual parts and a matrix norm
(src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §5 Distance Estimation, §6 Interior
Detection).

## Rebasing

Critical point at 0, one reference (inference from the single-critical-point
rule in [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)).

## Verdict for almondbread

Cheapest way to build and test the 2x2 matrix BLA path that Burning Ship and
hybrids need. Do it before [Burning Ship and abs variations](burning-ship.md).

# Citations

- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
