---
title: "mathr: Deep zoom (reference page)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2022
raw: "web/mathr-deep-zoom-page.html"
status: digested
tags: [perturbation, rebasing, bla, distance-estimation, interior-detection, burning-ship, mandelbar, hybrids]
---

# mathr: Deep zoom (reference page)

**URL:** <https://mathr.co.uk/web/deep-zoom.html> · **Saved:** `raw/web/mathr-deep-zoom-page.html` (+ `.txt`; the `.txt` is mangled MathML, formulas recovered from the TeX annotations in the HTML) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Concise, maintained reference page "Deep zoom theory and practice for the Mandelbrot set and other
fractals. As used in Fraktaler 3 and other renderers." Footer date 2024-05-31. It is the current,
corrected statement of perturbation + rebasing + BLA, plus distance estimation and interior
detection, and is the page Fraktaler 3's §Theory points to.

## What it covers

- Reference: `Z_{m+1} = Z_m^2 + C`, `Z_0 = 0`, high precision. Pixel orbit `Z_m + z_n`, `C + c`; delta step `z_{n+1} = 2 Z_m z_n + z_n^2 + c`, `z_0 = 0`. Note two counters: `m` (reference) and `n` (pixel).
- Rebasing: when `|Z_m + z_n| < |z_n|`, set `z_n := Z_m + z_n` and `m := 0`.
- BLA: `l` iterations from `n` approximated by `z_{n+l} = A_{n,l} z_n + B_{n,l} c`, valid when the dropped non-linear part is smaller than the rounding error of the low-precision type.
- Single step: `|z_n^2| << |2 Z_n z_n + c|`; neglecting `c` and factoring gives `|z_n| << |2 Z_n| = |A_{n,1}| =: R_{n,1}`. Single-step coefficients are `A = 2Z_n`, `B = 1`.
- Author retracts his earlier radius: "My earlier attempt at a validity check was nonsense, best to ignore that part of the older posts."
- Merge `T_z = T_y ∘ T_x` (`T_x` from `m_x` skipping `l_x`, `T_y` from `m_x + l_x`): `A_z = A_y A_x`, `B_z = A_y B_x + B_y`, `R_z = max(0, min(R_x, (R_y - |B_x||c|) / |A_x|))`.
- Table: `M` single-step BLAs (parallel), merge neighbours without overlap into `ceil(M/2)` two-step BLAs, repeat until one BLA skips `M-1`; start at iteration 1 because `Z_0 = 0` is a non-linear step. Table size `O(M)`. Marked "suboptimal according to Zhuoran".
- Lookup: from reference iteration `m` take the largest skip `l` with `|z| < R`; else one perturbation step. Check for rebasing after every BLA or perturbation step.
- Non-conformal (e.g. Mandelbar `X+iY -> (X-iY)^2 + C`): `A`, `B` become 2x2 real matrices; derivatives via dual numbers with two dual parts. With `sup|M|`, `inf|M|` = largest/smallest singular values: single-step `R = eps inf|A| - sup|B| |c| / inf|A|`; merged `R_z = max(0, min(R_x, (R_y - sup|B_x||c|) / sup|A_x|))`.
- Abs variations (Burning Ship `X+iY -> (|X|+i|Y|)^2 + C`): folds near `X=0` or `Y=0`, so single-step `R = max(0, min(eps inf|A| - sup|B||c|/inf|A|, |X|, |Y|))`. Fraktaler 3 divides `|X|`, `|Y|` by 2 as a paranoia factor. Merge rule unchanged.
- Hybrids: one reference per phase of the loop; rebasing switches to the current phase's reference; one BLA table per reference.
- Multiple critical points (none in Fraktaler 3's formulas): one reference and BLA table per critical point (per phase); rebase to the nearest orbit.
- Distance estimation: track `dz/dk` w.r.t. pixel coordinates `k` with dual numbers (dual part `1+0i` on pixel coordinates, then transform to `C+c`); result is prescaled by pixel spacing, which also avoids overflow. Non-analytic formulas: two dual parts, combined into a Jacobian.
- Interior detection: track derivative of `Z+z` w.r.t. `Z_1+z_1`; when its magnitude falls below a threshold such as `0.001`, mark interior and stop. Non-analytic: four dual parts (two DE, two interior) and operator norm. Using just `dz/dz_1` is justified when `C` is a nucleus (periodic with `Z = 0` in the orbit).
- References: SFT maths PDF; fractalforums topics 4360 (rebasing, BLA) and 4802 (interior detection); mathr.co.uk/helm (distance estimates).

## Reliability

Primary and the most authoritative statement of the BLA method by the Fraktaler 3 author;
supersedes the radius in [[sources/mathr-deep-zoom-theory-again]]. Still heuristic: no proof of
the radius bounds. Small inconsistency: §3.1 derives `R_{n,1} = |A_{n,1}|` without the precision
factor `eps` or the `c` term, while §4.1 gives `R = eps inf|A| - sup|B||c|/inf|A|`; for the
conformal case the §4.1 form with complex moduli is presumably the intended one (inference). The
manifest year 2022 is the content year; the page footer says 2024-05-31.

## Useful sections

- §1 The Mandelbrot Set — reference recurrence.
- §2 Perturbation — delta recurrence with separate `m`, `n`.
- §2.1 Rebasing — the rebasing test `|Z_m + z_n| < |z_n|`.
- §3 Bivariate Linear Approximation — validity principle.
- §3.1 Single Step BLA — single-step radius; retraction of the older check.
- §3.2 Merging BLA Steps — `A`, `B`, `R` merge formulas.
- §3.3 BLA Table Construction — pairwise binary table, start at iteration 1.
- §3.4 BLA Table Lookup — largest valid skip, rebase check after each step.
- §4.1 Non-Conformal BLA — matrix BLA, singular-value radii.
- §4.2 ABS Variation BLA — fold radii `|X|`, `|Y|`.
- §4.3 Hybrid BLA — per-phase references and tables.
- §4.4 Multiple Critical Points — per-critical-point references.
- §5 Distance Estimation — dual numbers, prescaled by pixel spacing.
- §6 Interior Detection — `dz/dz_1` threshold 0.001 and justification.

## Used by

[[techniques/perturbation]], [[techniques/rebasing]], [[techniques/bla]], [[techniques/distance-estimation]],
[[techniques/interior-detection]], [[formulas/mandelbrot]], [[formulas/mandelbar]], [[formulas/burning-ship]],
[[formulas/hybrids]], [[implementations/fraktaler-3]], [[questions/higher-order-bla]], [[questions/bla-vs-la-benchmark]]
