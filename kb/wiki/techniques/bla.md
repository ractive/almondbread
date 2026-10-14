---
title: "Bivariate linear approximation (BLA)"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/muency-bla]]", "[[sources/philthompson-bla]]", "[[sources/fraktaler-3-site]]", "[[sources/desarso-mandelbrot-webgpu-readme]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/fractalforums-interior-detection]]"]
tags: [bla, iteration-skipping, perturbation, core]
---

# Bivariate linear approximation (BLA)

While the delta `z` is small and the reference is not near 0, the perturbed
step is almost linear in `z` and `c`. Many steps then compose into one
`z ← A z + B c`. A binary tree of such steps, built once per reference,
lets each pixel skip most of its iterations. This is mathr's formulation,
used in Fraktaler 3 and Very Plotter; Imagina's refinement is
[[techniques/linear-approximation-imagina]].

## Single step

Drop `z^2` from `z → 2Zz + z^2 + c`:

```text
z_{m+1} ≈ A z_m + B c      with  A = 2 Z_m,  B = 1
```

Valid when `|z^2| << |2 Z z + c|` (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
Two radius formulas are in use:

1. mathr 2022 (Fraktaler 3 lineage):
   `r = max(0, ε · (|Z| − max_image|c|) / (|J_f(Z)| + 1))`, with
   `J_f(Z) = 2Z` for Mandelbrot and ε the hardware precision (2^-24 for
   float32, 2^-53 for double). mathr halves `|Z|` "just to be extra safe".
   The derivation is "a bit handwavy"
   (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
2. mathr deep-zoom page: neglect `c`, factor out `z`, giving
   `|z_n| << |2 Z_n| = |A_{n,1}| =: R_{n,1}`. He adds that his "earlier
   attempt at a validity check was nonsense"
   (src: [[sources/mathr-deep-zoom-page]] §3.1 Single Step BLA).
   Very Plotter turns the "<<" into a factor ε and rolls the `c` term in,
   using `R = (ε |Z| − |B| |c|) / |A|` per entry (src: [[sources/philthompson-bla]] §Calculating the BLA Coefficients).
3. The deep-zoom page's general form (stated for 2×2 matrices, reducing to
   complex magnitudes for conformal formulas):
   `R = ε |A| − |B| |c| / |A|`, i.e. `2ε|Z| − |c| / (2|Z|)` for a Mandelbrot
   single step (src: [[sources/mathr-deep-zoom-page]] §4.1 Non-Conformal BLA;
   the conformal reduction is our inference).

Recurrences for longer steps from one start (useful for checks):
`A_{l+1} = 2 Z A_l`, `B_{l+1} = 2 Z B_l + 1`, `A_0 = 1`, `B_0 = 0`
(src: [[sources/muency-bla]]).

## Merge

For `x` (first, `l_x` steps from `m`) followed by `y` (`l_y` steps from
`m + l_x`), with `T_z = T_y ∘ T_x`
(src: [[sources/mathr-deep-zoom-page]] §3.2 Merging BLA Steps):

```text
l_z = l_x + l_y
A_z = A_y A_x
B_z = A_y B_x + B_y
R_z = max(0, min(R_x, (R_y − |B_x| · max|c|) / |A_x|))
```

The radius rule says: the composite is valid if `x` is valid and the delta
after `x` (at most `|A_x| |z| + |B_x| |c|`) lands inside `y`'s radius.
"Higher order terms of Taylor expansion are probably necessary to get a
bulletproof radius" (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).

## Table

- `M` single steps, then merge neighbours without overlap into ⌈M/2⌉,
  ⌈M/4⌉, ... until one entry skips `M − 1`. About 2M entries, O(M)
  (src: [[sources/mathr-deep-zoom-page]] §3.3 BLA Table Construction).
- Start every level at iteration 1: iteration 0 is at the critical point,
  radius 0, always a non-linear step
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- One table per reference; each level can be built in parallel, even on a
  GPU (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- "This style of table construction is suboptimal according to Zhuoran"
  (src: [[sources/mathr-deep-zoom-page]] §3.3 BLA Table Construction);
  see [[techniques/linear-approximation-imagina]].
- `R` depends on `max|c|`, so the table belongs to one framing. Fraktaler 3
  says reusing BLA across a zoom-out sequence is "not generally applicable"
  (src: [[sources/fraktaler-3-site]]).

```text
// build (CPU, double or floatexp; store as the GPU's type)
for m in 1 ..< M:  T[0][m] = (A: 2*Zref[m], B: 1, l: 1, R: radius1(Zref[m], maxC))
level = 0
while count(T[level]) > 1:
    for j in 0 ..< count(T[level]) / 2:
        x = T[level][2j]; y = T[level][2j+1]
        T[level+1][j] = (A: y.A*x.A, B: y.A*x.B + y.B, l: x.l + y.l,
                         R: max(0, min(x.R, (y.R - abs(x.B)*maxC) / abs(x.A))))
    level += 1
```

## Lookup per pixel

At reference index `m`, use the entry starting at `m` with the largest skip
whose `|z| < R`; if none, do one perturbation step; check rebasing after
either (src: [[sources/mathr-deep-zoom-page]] §3.4 BLA Table Lookup).

```text
// entries starting at m exist at levels 0..L(m), L(m) = trailing zeros of (m - 1)
best = none
if |z| < R[level 0][m]:                // radii shrink with level
    for lev in L(m) down to 1:
        if |z| < R[lev][m] and m + len(lev) <= M: best = lev; break
    if best == none: best = 0
if best != none: z = A*z + B*c; m += len; n += len
else:            z = (2*Zref[m] + z)*z + c; m += 1; n += 1
// then bailout and rebase tests as in techniques/rebasing
```

Very Plotter's search does exactly this: test the lowest level first and
give up if it fails, otherwise test from the highest level down
(src: [[sources/philthompson-bla]] §BLA Validity Checking).

## Tuning and variants

- **ε**: forum values were 2^-24 (float) and 2^-53 (double). Very Plotter
  found deeper locations need smaller ε even with the same number type and
  searches ε automatically: binary search on the exponent, rejecting any ε
  where a test point's iteration count differs from plain perturbation by
  more than 0.001% (src: [[sources/philthompson-bla]] §Epsilon; §Auto-Epsilon).
  At 9.5e693 it settled on about 1e-122 (src: [[sources/philthompson-bla]] §Example Location 3).
- **Merge and cull**: drop levels 1 and 2 (skips of 1 or 2 are not worth a
  validity test), storing about 500 entries for a 1000-iteration orbit
  (src: [[sources/philthompson-bla]] §Merging BLAs).
- **Fraktaler 3**: a "BLA step limit" (default 1024; "a few thousand" is
  usually enough; too low gives glitchy images) and, since v3, "BLA skip
  levels" to save memory (src: [[sources/fraktaler-3-site]]).
- **Second order**: mandelbrot-webgpu keeps
  `w_out = A w + B δ + C w² + D w δ + E δ²`, pushing the first neglected
  term to cubic, so entry deltas can be about the square root larger.
  Measured at 2.8e40: 21× faster, 96.9% of iterations skipped,
  bit-identical to the unapproximated render
  (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Skipping most of the work).
  See [[questions/higher-order-bla]].

## Contradictions

The deep-zoom page (last updated 2024) says mathr's "earlier attempt at a
validity check was nonsense, best to ignore that part of the older posts"
(src: [[sources/mathr-deep-zoom-page]] §3.1 Single Step BLA). The most
likely target is formula 1 above, from the 2022 post, which the research
report and roadmap still quote. The page itself is not fully consistent:
§3.1 gives only `|z| << |A|`, while §4.1 gives `ε inf|A| − sup|B||c|/inf|A|`.
Also, Imagina scales its thresholds by 2^-24 even when a stage runs in
double, while BLA users quote 2^-53 for double; the inequalities differ, so
the numbers are not directly comparable (inference). Very Plotter's post
prints the merge as `B = A_x B_x + B_y`; composing the maps gives
`A_y B_x + B_y`, so treat the post's version as a typo. Practical answer for
almondbread: implement formula 3, keep ε a tunable, and validate against the
skip-off diff ([[questions/bla-vs-la-benchmark]]).

## Other formulas

- **Non-conformal** (Mandelbar/Tricorn): `A`, `B` become 2×2 real matrices;
  derivatives via dual numbers with two dual parts. With `sup`/`inf` the
  largest/smallest singular values: single step
  `R = ε inf|A| − sup|B| |c| / inf|A|`; merge
  `R_z = max(0, min(R_x, (R_y − sup|B_x| |c|) / sup|A_x|))`
  (src: [[sources/mathr-deep-zoom-page]] §4.1 Non-Conformal BLA).
  The operator norm can be computed from the trace and determinant of
  `AᵀA`: `‖A‖² = (T + sqrt(max(0, T² − 4D))) / 2`
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- **Abs variations** (Burning Ship): the fold lines add
  `R = max(0, min(ε inf|A| − sup|B| |c| / inf|A|, |X|, |Y|))`; Fraktaler 3
  halves `|X|` and `|Y|`; merge unchanged
  (src: [[sources/mathr-deep-zoom-page]] §4.2 ABS Variation BLA).
- **Hybrids**: one reference and one table per phase
  (src: [[sources/mathr-deep-zoom-page]] §4.3 Hybrid BLA).
  See [[formulas/burning-ship]], [[formulas/mandelbar]], [[formulas/hybrids]].

## Costs and storage

- Build: O(M) complex multiplies on the CPU, done once per reference and
  framing; parallel per level.
- Per entry: `A`, `B` complex plus `R` real; in float32 16 + 4 B, more with
  exponents. Over long skips `A` reaches about 1e700, so as plain doubles
  coefficients become infinite and radii NaN; store coefficients and radii
  with explicit exponents (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Skipping most of the work).
  In float32 this happens much sooner, and each quantity needs its own
  exponent because `A` grows exponentially while other fields stay O(1)
  (inference; see [[techniques/extended-range-floats]]).
- Per pixel: a BLA step is one complex multiply-add pair, about the cost of
  one perturbation step, so on a GPU skipping 1 or 2 iterations gains
  nothing (inference).
- Measured gains (CPU JavaScript, M1): 10.2× at 2.9e89 and 36.2× at
  4.99e152 against SA, with up to about 989k of 1.2M iterations skipped per
  pixel (src: [[sources/philthompson-bla]] §Speed Comparison); details on
  [[techniques/series-approximation]].
- mathr's 2022 experimental BLA program (later Fraktaler 3) rendered the
  "Flake" at 2400×2400 in 5 s on CPU and 3 s with OpenCL, versus 7.5 s and
  6.6 s for SA-based KF 2.15.5.2 (src: [[sources/fractalforums-interior-detection]] §Reply #9).

## History

Zhuoran hinted at BLA in the rebasing thread on fractalforums.org (topic
4360, 2021); mathr worked out the radius, merge rule and binary table
(src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
Mu-Ency says newer KF versions and contemporaneous programs use BLA and that
it is most useful at dwell limits in the millions (src: [[sources/muency-bla]]);
KF's own page says KF does not implement it ([[questions/kf-bla-support]]).

## For almondbread

Roadmap step 5: build the table on the CPU per reference and framing with radius formula 3 and
ε = 2^-24 as a starting value, drop the lowest levels, store floatexp
coefficients and radii past about 1e-30, and verify with a skip-off pixel
diff. Keep BLA for non-Mandelbrot formulas even after
[[techniques/linear-approximation-imagina]] lands. Benchmarks:
[[questions/bla-vs-la-benchmark]].
