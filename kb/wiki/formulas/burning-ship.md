---
title: "Burning Ship and abs variations"
type: formula
status: draft
sources: ["sources/mathr-deep-zoom-theory", "sources/mathr-deep-zoom-theory-again", "sources/mathr-deep-zoom-page", "sources/fraktaler-3-site", "sources/kf-page", "sources/fractalshades-readme"]
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

## As built in almondbread (F6)

Built 2026-10-23 ([F6: Burning Ship family](../plans/f6-burning-ship.md) "As built"; numbers in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F6"). Our own design and measurements
unless cited.

- **Orientation.** The literature's iteration has its ships upside down with the imaginary
  axis pointing up. almondbread iterates `z <- (|x| - i|y|)^2 + c` (`x' = x^2 - y^2 + cx`,
  `y' = cy - 2|xy|`): the same set mirrored in the real axis, so the ships stand upright like the
  usual pictures, and every view, pan and overlay keeps its normal orientation. Our coordinates
  are the literature's with the imaginary part negated (a ship at `-1.76 - 0.03i` there is at
  `-1.76 + 0.03i` here).
- **Perturbed step.** `x' = Re((2Z + z) z) + cx`, `y' = cy - 2 diffabs(XY, Xy + xY + xy)`.
  In the rescaled loop (`z = s w`) the fold is decided at the delta's own scale:
  `diffabs(XY/s, X w.y + w.x Y + s w.x w.y)`, with `XY/s` clamped to `±2^100` (no fold is
  possible there) and flushed below `2^-150`. The sign cases compare quantities of the size of
  `w`, so float32 holds them at any depth; the "full floatexp iterations whenever either part of
  `Z` is small" of the deep-needle note are not needed. What is needed is `XY` itself where one
  part of `Z` is far below the other: the shared-exponent floatexp reference flushes the smaller
  part (a needle reference has `Y ~ 1e-100` beside `X ~ 1`), so a folding formula's reference
  keeps each part with its own exponent (`FloatExpSplit`, the second exponent in the GPU
  struct's padding word). The kernel reads `XY` from the float32 reference while both parts are
  above `2^-60` and from the split one below.
- **Needle.** With that, the needle renders in the ordinary rescaled kernel: GPU against the
  oracle at the axis near -1.9, 1e-30 to 1e-1000 per pixel, class mismatch 0.000 %. No needle
  heuristic or floatexp-only pipeline was built. The needle stays slow in another way: a
  reference on the axis has `Y = 0` at every step, so no BLA block is ever valid there.
- **Jacobian and BLA.** Away from the folds the step is linear with Jacobian
  `[[2X, -2Y], [-2sY, -2sX]]`, `s = sign(XY)`: Mandelbar's reflection `δ ↦ conj(2Zδ)` for
  `s = +1` (also on a fold, one-sided) and multiplication by `2Z` for `s = -1`. The matrix BLA
  of [F5: Mandelbar (Tricorn) and the non-conformal path](../plans/f5-mandelbar.md) carries over;
  the single-step radius gains the fold cap `min(|X|, |Y|) / 2`.
- **Newton.** Jacobian Newton converges from inside the nucleus's smooth piece; the ball
  period bound `(|z| + R)^2 - |z|^2` holds (the fold keeps moduli); the box method does not
  (the fold maps the corners' quadrilateral onto one side of the axis).
- **Chaotic interiors.** Large parts of the set (the grey regions of the hull, the seas around
  deep minibrots) are bounded orbits that are not attracted to a cycle: direct f64 and BigFloat
  iteration disagree on 5–20 % of the pixels of shallow views, and a 0.001 px grid shift changes
  as many escape counts as float32 does. They stay unresolved (no derivative or probe proof
  exists); the attracting-cycle probe is off (its step mixes holomorphic and antiholomorphic
  Jacobians).

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fractalshades README](../sources/fractalshades-readme.md)
