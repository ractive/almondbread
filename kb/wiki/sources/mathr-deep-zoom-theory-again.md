---
title: "mathr: Deep zoom theory and practice (again) (2022)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2022
raw: "web/mathr-deep-zoom-theory-again.html"
status: digested
tags: [rebasing, bla, perturbation, hybrids, burning-ship]
---

# mathr: Deep zoom theory and practice (again) (2022)

**URL:** <https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html> · **Saved:** `raw/web/mathr-deep-zoom-theory-again.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Short follow-up blog post (21 Feb 2022) explaining Zhuoran's two 2021 ideas from fractalforums.org:
rebasing and bilinear approximation (BLA), with the author's first BLA validity radius and merge
rules as used in early Fraktaler 3.

## What it covers

- Rebasing: reset the reference iteration to the start when the pixel orbit `Z+z` comes near a critical point (0 for Mandelbrot). With several critical points, keep one reference per critical point and switch to the orbit `o` minimising `|(Z - Z_o) + z|` among the current reference and the critical-point orbits at iteration 0.
- Consequence: only as many references as critical points (one for Mandelbrot and Burning Ship); glitches are avoided rather than detected and fixed. Author calls this a big gain in efficiency and, more importantly, correctness.
- Hybrids need one reference per phase of the hybrid loop (e.g. loop `(M,BS,M,M)` needs its 4 rotations), plus one per pre-periodic step; times the number of critical points. References are serial but can be computed in parallel.
- BLA idea: when `Z` is not small and `z` is small, `z -> 2Zz + z^2 + c` is approximated by `z -> 2Zz + c`, linear in `z` and `c`.
- First validity radius (later retracted, see Reliability): `|z| < r = max(0, eps (|Z| - max|c|) / (|J_f(Z)| + 1))`, with `eps` the hardware precision (e.g. `2^-24`) and `J_f(Z) = 2Z` for Mandelbrot. Burning Ship: replace `|Z|` by `min(|X|,|Y|)`. `|Z|` halved as a safety factor.
- Non-conformal formulas use the operator (spectral) norm of the 2x2 Jacobian, computed from trace and determinant of `A^T A` (the post includes a C++ snippet; not reproduced here).
- Merge rule for BLA `x` then `y`, each `z -> A z + B c`: `l = l_y + l_x`, `A = A_y A_x`, `B = A_y B_x + B_y`, `r = min(r_x, max(0, (r_y - |B_x| max|c|) / |A_x|))`. Author admits higher-order Taylor terms may be needed for a bulletproof radius.
- Table: for a reference of `M` iterations, `2M` entries: level 0 has `M` single steps, each next level merges neighbour pairs without overlap. Start each level at iteration 1 since iteration 0 starts at the critical point (radius 0).
- Lookup: at the current reference iteration pick the valid BLA (`|z| < r`) that skips most; else do one perturbation step, rebasing as needed. One table per reference; levels can be built in parallel (suggests OpenCL on GPU).
- Claims BLA is conceptually simpler than SA, easier to parallelise, has better-understood stopping, and generalises to Burning Ship and hybrids; speed comparison left to benchmarks.
- Open: BLA initialisation for critical points not at 0; rebasing with multiple critical points was untested.

## Reliability

Primary, by the Fraktaler 3 author, but an early draft of the method. The later
[[sources/mathr-deep-zoom-page]] §3.1 says "My earlier attempt at a validity check was nonsense, best
to ignore that part of the older posts", so the radius formula here must not be used. The same page
says this table construction is "suboptimal according to Zhuoran". The rebasing description and the
merge rules for `A`, `B` remain valid.

## Useful sections

- §Rebasing — rebasing rule, multiple critical points, hybrid phase references.
- §Bilinear Approximation — BLA idea, retracted radius, merge rules, table build and lookup, comparison with SA.

## Used by

[[techniques/rebasing]], [[techniques/bla]], [[formulas/hybrids]], [[formulas/burning-ship]],
[[implementations/fraktaler-3]], [[questions/bla-vs-la-benchmark]], [[questions/higher-order-bla]]
