---
title: "Burning Ship and abs variations"
type: formula
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fraktaler-3-site]]", "[[sources/kf-page]]", "[[sources/fractalshades-readme]]"]
tags: [burning-ship, formula, abs-variation, perturbation, bla]
---

# Burning Ship and abs variations

## Iteration

`X + iY <- (|X| + i|Y|)^2 + C`, i.e. `X' = X^2 - Y^2 + Cx`,
`Y' = 2|X Y| + Cy` (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Abs Variations).
Relatives: Buffalo, Celtic, perpendicular variants (KF has many,
src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Perturbed form

Naive perturbation of `|XY + Xy + xY + xy| - |XY|` cancels to 0. Use the
case analysis by "laser blaster", `diffabs(a, d) = |a + d| - |a|`, which we
write as:

- if `a >= 0`: result is `d` when `a + d >= 0`, else `-(2a + d)`;
- if `a < 0`: result is `2a + d` when `a + d > 0`, else `-d`.

The `+-d` branches dominate when `d` is small. The perturbed imaginary
update is `y' = 2 diffabs(XY, Xy + xY + xy) + cy`, and the real part is the
ordinary `x' = 2Xx + x^2 - 2Yy - y^2 + cx`. With rescaling the argument
becomes `diffabs(XY/s, Xy + xY + s x y)`; when `s` underflows to 0 this
reduces to `sign(X) sign(Y) (X y + x Y)`, which also avoids underflow in
`sign(XY)` (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Abs Variations).

## Deep needle

Mandelbrot needs full floatexp iterations only when both parts of `Z` are
small; abs formulas need them when *either* `X` or `Y` is small. Near the
needle (`Y ~ 0` throughout), rescaled iteration is slower than plain floatexp
everywhere, and automatic detection of such locations "remains unresolved in
KF". The problem is worse in single precision (src:
[mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Deep Needle). For a float32 GPU this is
the main risk of the formula (inference).

## BLA form

- Non-conformal: `A`, `B` are real 2x2 matrices; derivatives use dual
  numbers with two dual parts (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.1
  Non-Conformal BLA).
- Single-step radius: `R = max(0, min(eps inf|A| - sup|B| |c| / inf|A|, |X|, |Y|))`
  where sup/inf are largest/smallest singular values; the |X|, |Y| caps are
  the fold lines. Fraktaler 3 halves |X| and |Y| for safety; the merge rule
  is unchanged (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.2 ABS Variation BLA).
- Singular value from `T = trace(A^T A)`, `D = det(A^T A)`:
  `sup|A| = sqrt((T + sqrt(max(0, T^2 - 4D)))/2)` (our restatement of the
  operator norm in [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear
  Approximation).
- Series approximation is nearly useless here: only valid until the first
  fold, "typically only a few iterations" (src:
  [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Series Approximation).

## Rebasing

One critical point at 0 for the ship too, so one reference (src:
[mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing).

## Support

Fraktaler 3 (|X| and |Y| toggles), fractalshades (Mandelbrot and Burning
Ship to 1e-2000, src: [Fractalshades README](../sources/fractalshades-readme.md)), KF (SA),
Fractal Zoomer.

## Verdict for almondbread

Feasible after Mandelbar: needs the matrix BLA path and diffabs. Budget for
a floatexp-only kernel for needle locations and a detection heuristic (e.g.
fraction of steps where `min(|X|,|Y|)` is tiny), since nobody has solved
automatic switching (inference).

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fractalshades README](../sources/fractalshades-readme.md)
