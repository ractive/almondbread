---
title: "Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)"
type: source
authors: ["Phil Thompson"]
year: 2023
raw: "web/philthompson-bla.html"
status: digested
tags: [bla, perturbation, auto-epsilon, period-detection, benchmarks, javascript]
---

# Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)

**URL:** <https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html> · **Saved:** `raw/web/philthompson-bla.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

A blog post (9 May 2023) by the author of the JavaScript viewer Very Plotter, on adding experimental BLA in its v0.10.0 release. It walks through the maths he took from the fractalforums.org thread (Zhuoran, Claude Heiland-Allen), the merge-tree storage, a linear validity search, an automatic epsilon search, and timings against series approximation on an M1 Mac.

## What it covers

- Pipeline for a BLA render: find an on-screen minibrot nucleus, compute the reference orbit there, find epsilon, build BLAs, then render each pixel with BLA where valid and plain perturbation otherwise.
- BLA only switches on deeper than 1e25 pixels per unit (auto mode).
- Nucleus search: ball-arithmetic period finding (gerrit's "1st order" version; the 2nd order one was too slow), with the ball radius set to the half-width and half-height of the window. Then Newton's method on the period. If the nucleus is off screen, repeat for a deeper, higher-period domain. Newton convergence is tested by comparing leading digits.
- With a periodic reference only one period of orbit is stored, and fewer BLAs are built. If no nucleus is found the screen centre is used; this works for perturbation and SA but makes his BLA much slower.
- Single-step BLA: drop the `Δz²` term when `|Δz²| < ε·|2 Z Δz|`; then `Δz_{m+l} = A_l Δz_m + B_l Δc` with `A_1 = 2Z_m`, `B_1 = 1`.
- Validity radius for one step: `r_1 = (ε|Z| − |B_1||Δc|) / |A_1|`; apply the BLA when `|Δz_m| < r`. The factor 2 is folded into ε.
- Naive "every skip length from every iteration" is O(n²) storage and checks (1,000-iteration orbit: ~500,000 records). Fixed-length chunks (e.g. skip 100 every 100th iteration) are mentioned as a reader's working alternative (Rosetta Code python).
- Merge tree (from Claude): merge adjacent pairs level by level, giving a binary tree of about 2n records. Dropping the 1- and 2-step levels (Zhuoran's suggestion) leaves about n/2 records.
- Merge rule as printed: `A = A_x·A_y`, `B = A_x·B_x + B_y`, `r = min(r_x, max(0, (r_y − |B||c|)/|A|))`. The `B` formula looks like a typo, see Reliability.
- Lookup: test the lowest level first; if it fails, no BLA at this iteration is valid. Otherwise test from the highest level down and take the first valid one. This works because radii shrink as levels go up.
- Epsilon from the forum: `2e-24` for float32 and `2e-53` for float64. He found these too large at deeper zooms: smaller ε is needed as depth grows, even with the same number type.
- Auto-epsilon: compute "true" iteration counts with plain perturbation on test points; for a candidate ε, build BLAs and compare. If any point differs by more than 0.001 %, ε is too large. Otherwise keep the ε that skips the most iterations. Binary search on the exponent (double/halve to bracket, then bisect). More test points for larger renders.
- Manual ε via the algorithm string, e.g. `bla-floatexp-blaepsilon1.0eminus114`.
- Timings (M1, 7 workers, 828x587): location 1 BLA 99 s vs SA 1009 s; location 2 BLA 358 s vs SA 12,958 s; "Flake" 58 s vs 89 s (1600x900, 6 workers: 220 s vs 536 s); "Evolution of Trees" 3882 s vs 6553 s (~989,000 vs ~1,063,940 iterations skipped per pixel).
- Counter-example (Dinkydau "SSSSSSSSSSSSSSSS", floatexp): BLA 3,438 s vs SA 2,076 s. A megapixel sweep at 16:9 fits BLA ≈ 422 s + 4562 s/MP and SA ≈ 158 s + 3915 s/MP, so SA always wins there. Auto-ε chose `1e-122` at every size.
- BLA did not yet work with stripe-average colouring.

## Reliability

- Primary for Very Plotter's own implementation, secondary for the theory (taken from the fractalforums thread). Author's own benchmarks on one machine; SA and BLA used different "algorithm" settings per location, so comparisons are rough.
- The merge formula for `B` reads `A_x·B_x + B_y`. Composing x then y gives `Δz = A_y(A_x Δz + B_x Δc) + B_y Δc`, so `B = A_y·B_x + B_y` (inference; compare the merge rule in [[sources/mathr-deep-zoom-theory-again]]). Treat the post's version as a typo.
- The post defines `Δz_n = Z_n − z_n` and `Δc = C_ref − c`, the opposite sign to the usual convention, yet writes the standard `+Δz²` recurrence. The sign slip does not affect the magnitudes used in the radius tests (inference).
- The text conversion turned "O(n/2)" into "O(n2)"; the HTML has the fraction.
- Auto-epsilon is a heuristic with no error bound; it only checks the sampled test points.

## Useful sections

- §Using BLA in Very Plotter — the five-step render pipeline and the 1e25 switch-over.
- §Finding A Periodic Point — ball-method period finding plus Newton, reference over one period.
- §Calculating the BLA Coefficients — single-step coefficients and validity radius.
- §Merging BLAs — O(n²) problem, merge tree, dropping low levels, merge formulas.
- §BLA Validity Checking — lowest-then-highest-down lookup.
- §Epsilon / §Auto-Epsilon / §Manual Epsilon — forum ε values and the automatic search.
- §Speed Comparison — per-location timings and the megapixel fit.

## Used by

- [[techniques/bla]]
- [[techniques/reference-selection]]
- [[implementations/very-plotter]]
- [[questions/bla-vs-la-benchmark]]
