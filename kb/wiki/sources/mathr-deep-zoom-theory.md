---
title: "mathr: Deep zoom theory and practice (2021)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2021
raw: "web/mathr-deep-zoom-theory.html"
status: digested
tags: [perturbation, glitch-detection, rescaling, extended-range-floats, series-approximation, burning-ship, hybrids]
---

# mathr: Deep zoom theory and practice (2021)

**URL:** <https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html> · **Saved:** `raw/web/mathr-deep-zoom-theory.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Blog post (14 May 2021) by the maintainer of Kalles Fraktaler 2+ summarising the state of deep-zoom
rendering as implemented in KF just before rebasing and BLA appeared: perturbation, Pauldelbrot
glitch detection, rescaled iterations, abs variations (Burning Ship), hybrids, and series
approximation including knighty's NanoMB1/NanoMB2.

## What it covers

- Notation used across the mathr material: upper case `Z, C` = high-precision reference, lower case `z, c` = low-precision per-pixel delta.
- History: K.I. Martin's SuperFractalThing (2013) popularised two ideas: one high-precision reference orbit plus low-precision deltas, and a truncated power series in `c` to skip early iterations.
- Perturbed Mandelbrot step: `z -> 2 Z z + z^2 + c`, derived by expanding `(Z+z)^2 + (C+c)` and cancelling the reference part symbolically to avoid absorption. The reference is computed at high precision and rounded to double for storage.
- Pauldelbrot glitch criterion: glitch when `|Z+z|^2 < G |Z|^2`, with `G` between `1e-2` and `1e-8`. Derived by perturbing the perturbed iteration; when `e/z` is small, neighbouring pixels "stick together". `G|Z|^2` can be precomputed per reference iteration, `|Z+z|^2` is already needed for escape. Glitched pixels are re-rendered from a new reference; choosing `G` is called an open problem.
- Precision vs range: double (53-bit mantissa) or even single (24-bit) is enough precision for deltas; the limit is range: double underflows below about `1e-308`, single below about `1e-38`.
- floatexp (mantissa normalised near 1 plus separate integer exponent) works at any depth but is slow because every operation is in software.
- Rescaling: substitute `z = S w`, `c = S d` to get `w -> 2 Z w + S w^2 + d`; choose `S` so `|w| ~ 1`; re-scale when `|w|` drifts, typically every few hundred iterations since `|Z| <= 2` until escape. If `S` underflows to 0 the `S w^2` term can be dropped (when `Z` is not small); same for `d`.
- Full iterations: when `|Z|` is tiny it underflows in plain double, so store full-range `Z` and do that iteration in floatexp (`z = S w`, full perturbed step, then `S = |z|`, `w = z/S`, `d = c/S`), then rescale (credited to Pauldelbrot's "Rescaled Iterations in Nanoscope").
- Burning Ship perturbation uses laser blaster's `diffabs(c, d) = |c+d| - |c|` case split; with rescaling becomes `diffabs(XY/s, Xy + xY + s x y)`; when `s = 0` it reduces to `sign(X) sign(Y) (X y + x Y)`.
- Deep needle: for abs variations full iterations are needed when either `X` or `Y` is small, so near the Burning Ship needle (where `Y ~ 0` throughout) rescaled iteration is slower than plain floatexp. Worse in single precision. Auto-detecting such locations is unsolved in KF.
- KF hybrid model: up to 4 stanzas, 1-2 lines each, each line two operators combined by `+ - *`; operator controls abs x/y, negate x/y, integer power `p`, complex multiplier `a`; `+c` at stanza end. Per-operator perturbation recipe uses `W = Z+z`, `B = Z` and `S = sum_{i=0}^{p-1} W^i B^{p-1-i}`, returning `a z S`.
- Rescaling hybrids: full floatexp iteration when either real or imaginary part of `Z` is small, otherwise thread `s` through (`W = Z + z s`, `diffabs(re(Z/s), re(z))`).
- Distance estimation for hybrids with dual numbers; with rescaling, initialise dual parts with the pixel spacing instead of 1 to avoid derivative overflow.
- Code generation: generating OpenCL source per formula removes inner-loop branches and runs much faster than interpreting parametric hybrids, even on the same device.
- Series approximation: `z_n = sum_k A_{n,k} c^k`; recurrences by equating coefficients; validity traditionally checked against probe points, rolling back one iteration when they diverge.
- knighty's bivariate series in `(z, c)` around a periodic reference skips whole periods (KF "NanoMB1"); chained minibrots ("NanoMB2") are experimental and fail at many locations. For abs variations SA works only until the first fold, so few iterations are skipped.
- Conclusion: no rigorous error bounds exist for SA; choice of `G` remains open.

## Reliability

Primary source by the KF2+ / Fraktaler 3 author, describing his own implementation. Accurate for
2021 practice but superseded on two points: glitch detection plus multiple references was replaced
by rebasing ([[sources/mathr-deep-zoom-theory-again]], [[sources/fractalforums-zhuoran-rebasing-bla]]),
and series approximation by BLA ([[sources/mathr-deep-zoom-page]]). The rescaling and floatexp material
remains current. Hand-wavy derivations, no benchmarks.

## Useful sections

- §Introduction — SFT history, the two core ideas.
- §Perturbation — perturbed iteration and notation.
- §Glitches — Pauldelbrot criterion `|Z+z|^2 < G|Z|^2` and its derivation.
- §Rescaling — float32/float64 range limits, floatexp cost, rescaled iteration `w -> 2Zw + S w^2 + d`.
- §Full Iterations — when `Z` is small, do one floatexp step then rescale.
- §Abs Variations — `diffabs` and its rescaled form.
- §Deep Needle — why rescaling loses near the Burning Ship needle.
- §Hybrid Fractals — KF hybrid structure and per-operator perturbation recipe, dual-number DE.
- §Code Generation — OpenCL kernel generation to remove branches.
- §Series Approximation — SA recurrences, probe points, NanoMB1/NanoMB2.

## Used by

[[techniques/perturbation]], [[techniques/glitch-detection]], [[techniques/extended-range-floats]],
[[techniques/series-approximation]], [[techniques/reference-orbit-precision]], [[formulas/burning-ship]],
[[formulas/hybrids]], [[implementations/kalles-fraktaler-2]], [[implementations/superfractalthing]],
[[techniques/distance-estimation]], [[questions/float32-error-long-orbits]]
