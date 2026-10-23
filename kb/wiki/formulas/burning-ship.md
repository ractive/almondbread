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
- **Chaotic interiors.** Large parts of the set (the hull, the seas around deep minibrots in it)
  are bounded orbits that are not attracted to a cycle: direct f64 and BigFloat iteration
  disagree on 5–20 % of the pixels of shallow views, and a 0.001 px grid shift changes as many
  escape counts as float32 does. No derivative or probe proof exists for them; the
  attracting-cycle probe is off (its step mixes holomorphic and antiholomorphic Jacobians).
- **Derivative fold sign (review fix, 2026-10-23).** The DE and lighting Jacobian must take the
  fold sign `s = sign((X + x)(Y + y))` from the same test the step uses, `X Y / s + d` at the
  delta's scale. Taken from the float32 sum `Z + s w`, it is wrong on the needle: `Y + y`
  (~1e-97) flushes to 0, the reflection is taken at every step, and the normal points the wrong
  way wherever the true sign pattern differs. That drew nested axis-aligned rectangles with
  sharp colour steps at 8e94x (the regions between fold lines are rectangles near the axis,
  where the Jacobian is diagonal). The smooth count itself was right (GPU vs oracle 0.002 %),
  and finite differences of it showed no rectangles; 9 382 of 33 238 smooth pixels' normals
  pointed away from `-grad mu`, 0 of 34 430 after the fix (test
  `burning_ship_needle_normal_follows_the_smooth_count`). The derivative is still legitimately
  discontinuous across fold preimages, so lighting shows genuine creases there (the vertical seam
  below a needle minibrot, the arcs above the ship); finite differences of `mu` show the same.
- **Bounded orbits drawn as the set (2026-10-23).** Pixels unresolved at the limit draw as the
  interior colour, not the fog ([Fog over unresolved pixels](../techniques/unresolved-fog.md) "Exception: bounded
  orbits"): at the whole ship, of the pixels unresolved at the auto limit 306, 86 % never escape
  within 1e6 iterations (direct BigFloat, 160 × 100, `fold_truth` example), and no proven-interior
  pixel escapes. The grey smear in the hull becomes the solid hull of the usual pictures.
- **Little ships.** Minibrots seek finds inside the hull (periods 31, 403, 3224 below the deck)
  sit in a chaotic sea: at period 403 (1e-22 per pixel) BigFloat at the view's precision and 256
  bits more agree on 2 546 of 2 560 escape counts, but a 0.001 px shift changes 83 % of them
  (counts 2^10–2^12 next to each other). The sea is noise at any sampling, so it shows as the
  calm dense colour, and the ship is the skewed set of never-escaping pixels (12.5 % of the view,
  the same shape at 14 000, 100 000 and 1e6 iterations). More iterations do not help: 97 % of
  the escaping pixels escape before the auto limit 14 000. Log colouring (`--log 1 --density 0.5`)
  gives the sea a gradient towards the ship and outlines it. Ships found by seek from the
  big ship's masts (outside the hull) are recognisable ships with hull and mast in bands of
  their own shape: periods 7 (1e-4.5), 20 (1e-10.4), 60 (1e-18.6); at period 60 a 0.001 px
  shift changes 41 % of the counts and they are mostly 2^8–2^9. The first places used ships 20 and
  60; the second review replaced them with ships from the literature (below).
- **Seams are creases, not a bug (2026-10-23, second review).** The straight seams where the
  lighting brightness jumps (vertical at the four-ship armada, at the centre column of the
  cathedral ship and above the little ship at -1.861) are fold preimages: at the armada
  (240 × 160, 4 000 iterations; measured 2026-10-23, M1 Pro, release) 1 689 neighbour pairs
  whose normals turn by more than 37°; across them the GPU's smooth count is continuous (1 688),
  its one-sided finite differences turn as the normals do (1 492; the normals follow them in
  1 439), and direct f64 orbits of the two pixels have different sign patterns `(x_k, y_k)` before
  the escape in all 1 689 (other neighbour pairs: 5 of 192), first at step 7–19 of ~33. No calm
  normal points away from `-grad mu` (test
  `burning_ship_lighting_seams_are_creases_of_the_smooth_count`). The kink is in `mu` itself, so
  any normal derived from it shows the seam. A softening (proposal, not built): give the
  derivative recurrence a smoothed fold, blending the two Jacobians (`2Z` and the reflection)
  with a weight `t = smoothstep` of the folded coordinate's signed distance in pixels,
  `x_k / (k_px · |dz_k/dc| · pixel)`, so the crease becomes a gradient a few pixels wide; the
  smooth count and the step stay exact. A cheaper screen-space alternative blurs the normal over
  3–5 texels where neighbouring normals disagree.
- **Places (second review, 2026-10-23).** Fifteen: the whole set, the ship, the needle spire,
  four little ships (cathedral at -1.625 and -1.861, tail ship at -1.941 from
  [Paul Bourke's Burning Ship page](https://paulbourke.net/fractals/burnship/), the four-ship
  armada at 1e-4.7), the hourglass pagoda, glowing grid and star vault (1e-9 to 1e-14), the falcon
  sun (1e-24), needle minarets (1e-38), the needle at 1e-100, the supernova (1e-150, the centre of
  [mathr's 2018 zoom](https://mathr.co.uk/web/a-burning-ship-fractal-zoom.html)) and a spiderweb
  at 1e-300 (on the path of Maths Town's "Industrial" zoom, its published 365-digit centre).
  Frames from the [robotmoon Burning Ship page](https://robotmoon.com/burning-ship-fractal/)
  presets (its imaginary parts negated for our orientation, as are Maths Town's and mathr's B).
  At the supernova the GPU differs from the oracle in 31 % of escape counts and a 0.001 px grid
  shift of the oracle in 40 %: chaos, not error; at the spiderweb 4.5 % and 6.8 %.

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fractalshades README](../sources/fractalshades-readme.md)
- <https://paulbourke.net/fractals/burnship/>
- <https://robotmoon.com/burning-ship-fractal/>
- <https://mathr.co.uk/web/a-burning-ship-fractal-zoom.html>
- <https://archive.org/details/youtube-svxQ7CPcdPk>
- <https://commons.wikimedia.org/wiki/File:Burning_Ship_Fractal_Zoom_2.png>
