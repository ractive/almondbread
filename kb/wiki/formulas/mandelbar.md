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

## As built in almondbread (F5)

Built 2026-10-22 ([F5: Mandelbar (Tricorn) and the non-conformal path](../plans/f5-mandelbar.md) "As built"; measurements in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F5"). Our derivations, checked against direct BigFloat
iteration:

- **Real-linear maps in Wirtinger form.** Every derivative is `L d = p d + q conj(d)` (two
  complex numbers: a real 2x2 matrix `[[a, b], [c, d]]` is `p = ((a+d) + i(c-b))/2`,
  `q = ((a-d) + i(c+b))/2`). Singular values `|p| + |q|` and `||p| - |q||`, determinant
  `|p|^2 - |q|^2`, composition `y∘x = (p_y p_x + q_y conj(q_x), p_y q_x + q_y conj(p_x))`. The step's
  Jacobian is `p = 0, q = conj(2z)` (a scaled reflection), so `dz/dc` evolves as
  `p' = conj(2 z q) + 1`, `q' = conj(2 z p)`.
- **Reference orbit.** The three-product step with the imaginary part negated:
  `Y' = (X^2 + Y^2) - (X+Y)^2 + Cy` (the three-thread split carries over unchanged).
- **Perturbation.** `z' = conj((2Z + z) z) + c`, in every regime (float32, rescaled, floatexp).
- **BLA.** `A`, `B` real-linear; the radius rule with sup/inf singular values. The per-step
  radius equals Mandelbrot's (`inf|A| = 2|Z|`); merged blocks use `sup|A_x|`.
- **Distance estimate.** The gradient of `ln|z|` over `c` is `J^T z / |z|^2`, as a complex number
  `z conj(p) + conj(z) q`; the conformal formula `|z| ln|z| / |d|` with
  `d = p + conj(q) z / conj(z)` gives the same distance and normal direction.
- **Interior.** The derivative test on `max|p| + max|q|`; the attracting-cycle probe on even
  periods only, where `f^k` is holomorphic in `z`: `λ' = conj(2zλ)`, `F''' = conj(2λ^2 + 2zF'')`.
- **Newton.** `c ← c - J^{-1} z_p` with `J^{-1} w = (conj(p) w - q conj(w)) / (|p|^2 - |q|^2)`;
  the exact finish on registers. Minibrot size `1 / (|l| sqrt|det dz_p/dc|)`, no rotation.
- **Shapes.** Odd-period minibrots are small Tricorns, even-period ones skewed Mandelbrot sets
  (seen in every seek run, e.g. periods 3, 7, 833 against 20, 86, 620; measured 2026-10-22).
- **Symmetry.** `c ↦ e^{2πi/3} c` maps the set to itself, so `1 ± i√3` are images of the tip `-2`
  (Misiurewicz points off the axis; the oracle tests use them).

# Citations

- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
