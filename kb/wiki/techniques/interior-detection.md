---
title: "Interior detection"
type: technique
status: draft
sources: ["sources/cheritat-mandelbrot-rendering", "sources/fractalforums-interior-detection", "sources/mathr-deep-zoom-page", "sources/mathr-interior-distance", "sources/wikipedia-plotting-algorithms", "sources/wikibooks-mandelbrot-interior", "sources/wikibooks-fraktaler-3", "sources/kf-manual", "sources/imagina-hinfla-evaluator"]
tags: [interior, derivative, periodicity, performance]
---

# Interior detection

Pixels inside the set never escape, so without a test they run to maxIter. Near
minibrots at depth, maxIter is often 1e6 to 1e9, so interior pixels dominate the
cost. Interior detection stops them early. It also makes a very large maxIter
cheap, which removes most of the need for hand-tuned iteration limits
(see [Automatic maximum-iteration control](auto-iterations.md)).

## Methods compared

| Method | Cost | Works at depth? | Notes |
| --- | --- | --- | --- |
| Cardioid / period-2 bulb test | ~6 flops per pixel, once | yes as built here (exact expansion per level) | exact, only for the two biggest components |
| Periodicity check (saved iterate, Brent) | 1 compare per iteration + bookkeeping | poor in float32 | tolerance must match precision |
| Derivative test `\|dz/dz₁\| < ε` | 1 complex mul per iteration | yes | the recommended method |
| Interior DE (Newton on the cycle) | Newton solve per candidate period | yes with perturbation | exact radius, expensive |

### Cardioid and bulb

Main cardioid: with `q = (x − 1/4)² + y²`, the point is inside if
`q·(q + (x − 1/4)) ≤ y²/4`. Period-2 bulb: `(x + 1)² + y² ≤ 1/16`. No exact
test exists for higher bulbs (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Cardioid / bulb checking).
Evaluated naively in the pixel's own `c` it only works at low zoom. Expanded exactly around
the view it works at any depth and matters most where the other tests fail: near the two
components' parabolic boundaries (the −3/4 neck, the cusp). See "Closed-form component test"
below.

### Periodicity checking

Keep a saved iterate; if the current `z` comes back to it within a tolerance,
declare interior. Refresh the saved point periodically (Wikipedia's example
every 20 iterations), or use Brent's power-of-two schedule to catch any period
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Periodicity checking). The
tolerance is the weak point: rounding can also make the detected period a
multiple of the true one (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Interior distance estimation).
Under perturbation in float32 the full orbit `Z+z` only has ~1e-7 relative
resolution, so it cannot resolve cycle closure at deep zoom; comparing deltas
only works if the reference is periodic too (inference). Treat it as secondary.

### Derivative test (recommended)

Track the derivative of the orbit with respect to its starting point,
`der = (Pⁿ)′`, which is the product of `2z` along the orbit. If the parameter is
hyperbolic the orbit falls into an attracting cycle and the product tends to 0.
Stop when `|der| < ε` and call the pixel interior
(src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Following the derivative).

- Start from `z = c, der = 1` (starting at the critical point 0 would multiply by 0 at once).
- Update `der` **before** `z`; `z = z² + c; der = der·2z` uses the new `z` and is wrong
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Following the derivative).
- ε = 0.001 gives good images; ε = 0.1 visibly misclassifies near parabolic
  parameters (e.g. the neck near c = −3/4)
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Images and speed).
- Each iteration costs about 2× a plain iteration, but a huge N stops mattering:
  "N = 10⁶ is OK, but avoid N = 10¹⁰"
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Images and speed).
- False positives exist (some non-hyperbolic `c` have `liminf |der| = 0`) but are
  rare and lie close to the set (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §The idea).

**Under perturbation and BLA.** GBy (fractalshades) iterates `∂z_n/∂Z₁` and stops
when it drops below ε. It "seems to work surprisingly well in combination with
perturbation and Bilinear approximation"; ε = 0.001 or even 0.01 is safe in most
cases. His examples need 1e7 iterations at period 14292, and a period-57853
minibrot at 5e-433 with 4e8 max iterations renders 3200×3200 in under 5 minutes.
He advises activating it only when a minibrot is in view, because it costs an
extra quantity per iteration (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md) §Interior detection for deep Mandelbrot set zooms).
mathr notes that without interior checking KF needed NanoMB1 to make such a
location feasible at all (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md) §Reply #9).

mathr's deep-zoom page states the rule: track the derivative with respect to
`z₁`, classify as interior when its magnitude drops below a threshold such as
0.001. Using `dz/dz₁` (the delta's derivative) instead of the full
`d(Z+z)/d(Z₁+z₁)` works when the reference is periodic with 0 in its orbit, i.e.
the reference is the nucleus of a hyperbolic component, because then the
reference's own contribution `dZ/dZ₁` is 0. For non-conformal formulas use dual
numbers with four dual parts (two for DE, two for interior) and the matrix
operator norm (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §6 Interior Detection).

Imagina carries `dzdz` (and `dzdc`) through LA steps: each LA step multiplies
the derivative by `2(dz + Ref)·ZCoeff`, so the test survives iteration skipping
almost for free (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)). With BLA the same
holds: a skip `z → A z + B c` has `∂z/∂z₁` multiplied by `A` (inference).

```text
// per pixel, float32, perturbation; reference = nucleus (periodic, Z_p = 0)
der = (1, 0)                       // d z / d z1, start after the first step
loop over steps:
    if bla skip (A, B, l):  der = A * der           // linear map
    else:                   der = 2 * (Zref[m] + z) * der
    advance z, m, n
    if dot(der, der) < EPS2: interior; break         // EPS2 = 1e-6 (eps 1e-3)
    if dot(der, der) > 1e30: stop tracking der        // exterior, avoid overflow
```

Float32 safety (inference): only `|der|` matters and `Z+z` is O(1), so rounding
(~1e-7 relative) barely affects it. Interior `der` shrinks and may underflow,
which already means interior. Exterior `der` grows: clamp or stop updating it
past ~1e30 to stay below float32 max ~3.4e38. Note this is `d/dz`, not `d/dc`;
the exterior DE derivative is separate ([Distance estimation](distance-estimation.md)).

### Interior distance estimate

mathr's practical method (src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §Practical interior distance rendering):

1. Each new minimum of `|z_n|` gives a candidate period `p` (atom domains).
2. Newton-solve `f^p(z₀) = z₀` with `z₀ ← z₀ − (f^p(z₀) − z₀)/((f^p)′(z₀) − 1)`,
   stop at step < 1e-10, at most 64 steps.
3. If `|d| = |(f^p)′(z₀)| ≤ 1` the point is interior; then
   `DE_int = (1 − |d|²) / |∂c d + ∂z∂z f^p · ∂c f^p / (1 − d)|`.

Strategies: plain (no checks), unbiased (check at every new minimum), biased
(if the previous pixel was exterior, defer checks to maxIter). Biased was best
overall. Near double-precision limits interior DE fails visibly while exterior
DE still works; fix with more precision or perturbation around the island's
nucleus (src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §Practical interior distance rendering).
All points within `b/4` of `c` are interior; a period detected as a multiple of
the true one overestimates `b` (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Interior distance estimation).
The multiplier test `|d| ≤ 1` alone (without the distance) is the cheap part
(src: [Wikibooks: Mandelbrot set interior](../sources/wikibooks-mandelbrot-interior.md)).

### Other

- Neighbour heuristic: a pixel is probably interior if it and all neighbours are
  interior with the same period (src: [Wikibooks: Mandelbrot set interior](../sources/wikibooks-mandelbrot-interior.md)).
- KF2+ "interior checking" exists only for power-2 Mandelbrot with NanoMB1/2, is
  experimental and "may or may not speed up" (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Fraktaler 3 EXR metadata shows `bailout.inscape_radius = 0.000976562` (2⁻¹⁰);
  plausibly the interior threshold, meaning unverified
  (src: [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)) (unverified).

## Pitfalls

- Wrong update order of `der` and `z`.
- ε too large (≥ 0.1) fills necks and thin parts near parabolic points.
- With a non-periodic reference (e.g. view centre), `dz/dz₁` is not the full
  derivative; track `d(Z+z)` instead or use a nucleus reference (inference from
  [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §6 Interior Detection).
- Periodicity tolerances in float32 at depth.
- On GPU, a history-dependent "biased" strategy increases divergence; a fixed
  per-iteration test is simpler (inference).
- **A fixed ε is wrong at any value near parabolic points, and the error has
  straight edges** (found 2026-10-14 from a user report; measured M1 Pro, release).
  An exterior orbit with `c` close to a bulb root passes slowly through the
  parabolic gate. Mid-passage, `|dz/dz₁|` dips to about `C/n²` and then recovers.
  For `z → z + z² + δ` the product of `(1 + 2w_k)` over the passage is
  `(w_k² + δ)/(w_0² + δ)`. It is smallest (`≈ δ/a²`) in the middle, `n ≈ π/(2√δ)`,
  which is half the escape time. Direct Double iteration gives:
  - `c = 0.2501` (cusp): minimum 1.1e−3 at n = 150, escape at 315.
  - `c = −0.75 + 0.0021i` (the −3/4 neck): minimum 9.9e−4 at n = 724, escape at
    1500.

  So the dip is right at Chéritat's 1e−3, and it gets deeper the closer `c` is
  to the root. In the app at the neck, zoom 2.2e5, auto maxIter 2053, Chéritat's
  test painted 20.9 % of the frame black (pixels that escape before 2053). Their
  outline was a union of rotated squares with sharp concave corners. The
  max-norm test `max(|w.x|, |w.y|) < ε` is a square in `w`. Over a 2e−5-wide view
  `w(c)` at the checked iteration is nearly linear, so its preimage is a rotated
  square. A wedge of black with straight edges came out of a smooth exterior
  basin. The cusp view (0.25005, width 1e−4) had 1.7 % false interior. The
  benchmark set had 0.04 % (elephant) and 0.07 % (feigenbaum) GPU-int/CPU-ext,
  which had been taken for float noise.
  **Fix:** test `|w| n² < ε`. Interior `w` decays geometrically and beats `n²`
  within a few periods. The passage dip is `≈ C/n²` with C ≈ 27 at the cusp
  (`dip × n_dip²`). At the neck it is larger and grows like 1/δ. ε = 1e−3 leaves a
  margin of more than 10⁴. ε = 30 brings the elephant wedges back (15 % false
  interior), and ε = 1 is already clean. The cost is that slow, near-parabolic
  *interior* converges like `C/n²` at first, the same as the exterior dip, so it
  cannot be proven before the geometric phase. Those pixels are not interior
  failures but unresolved, see [Automatic maximum-iteration control](auto-iterations.md). Elephant
  (detected 26.9 % → 23.3 %) and feigenbaum (24.4 % → 15.0 %) get slower; see
  [almondbread benchmarks and verification record](../synthesis/benchmarks.md).
  Shortcuts that confirm a candidate by "no escape for k·n more iterations" or
  "`|w| n²` non-increasing" fail on an orbit that enters a second, much longer
  gate early. Its `|w| n²` grows only slowly there (inference from the gate model
  above). Do not reintroduce them without a regression view.
  Regressions: bench locations `neck-2e5`, `neck-7e6`, `cusp-1e5` and
  `root-third`. `--bench` fails above 0.02 % GPU-int/CPU-ext, and `make regress`
  runs the scripted zoom sessions.

## Measured in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, release; tables in [almondbread benchmarks and verification record](../synthesis/benchmarks.md).

- Implemented as above, with `w = dz_n/dz_1` of the **full** orbit `z = Z_m + dz` (so a
  non-periodic view-centre reference is fine), `w_1 = 1` set by a peeled first step,
  the max-abs (Chebyshev) test `max(|w.x|, |w.y|) < 1e−3`, and `w` carried with a
  power-of-two exponent (rescale by 2^±60, exact) instead of clamping, which would bias
  towards false interior.
- **Accuracy:** with ε = 1e−3, full-resolution results (7.7 M pixels) are bit-identical
  to running every interior pixel to maxIter, at all seven benchmark locations: the test
  produced **no** extra boundary pixels classified interior. **Superseded:** that
  comparison was against maxIter, not against escape. Against the CPU it was wrong near
  parabolic points (see Pitfalls); since 2026-10-14 the test is `|w| n² < 1e−3`.
  0.5–5 points of the CPU's
  interior pixels are not caught before maxIter (slow, near-parabolic components:
  elephant 26.9 % detected vs 28.6 % interior, feigenbaum 24.4 % vs 29.5 %).
- **Cost:** testing every iteration doubled the per-iteration cost (seahorse 30 → 66 ms,
  matching Chéritat's ~2×); updating `w` every iteration but testing and rescaling only
  every 8th iteration brings it to ~1.5× (48 ms). Overflow stays bounded because
  `|2z| > 4` only in the last few steps before escape.
- **Adaptive switch** (GBy's advice): each level enables the test only when the previous
  level's statistics say `interiorFraction × maxIter > 0.6 × exteriorIterationsPerPixel`.
  Exterior-only views then run at plain speed (seahorse 32.5 ms, deep 196 ms vs 33 / 193
  off).
- **"Interior" in the statistics is not proof.** The kernel's `interior` counter adds the
  proven pixels and the ones that reached maxIter unresolved (`crossed`). A view that is
  unresolved everywhere looks 100 % interior to the switch. At `sea-8007` every pixel
  reached the depth limit without escaping and without a proof, so the view showed flat
  unresolved grey (not false interior). The fix is in the iteration learner (see
  [Automatic maximum-iteration control](auto-iterations.md), "Nothing escaped"). After the ×4 render, the switch sees
  ~0.3 % interior and turns the test off for the full level. The few remaining limit pixels
  (the minibrot) are then drawn as interior, which matches the CPU.
- **Speedups** (3456×2234, GPU): home at 100 000 iterations 2211 → 48 ms (46×),
  period-4 minibrot at 50 000 790 → 52 ms (15×), elephant 222 → 112 ms, home at 1 000
  26 → 14 ms, home at 1e7 (800×500) 11.4 s → 83 ms.

## Attracting-cycle probe (built 2026-10-17)

**Problem.** `|w| n² < ε` proves only the fast core of a component. With multiplier `λ` per
period `P`, an interior orbit needs `|λ|^(n/P) n² < ε`. At the place `satellite` (P 39, auto
limit 1994 = 51 periods) that is `|λ| < ~0.7`: the core disc of the cardioid. The rest of the
body, the bulbs (P 78, 117, …) and small attached minibrots stayed unresolved, which since
the unresolved fog ([Fog over unresolved pixels](unresolved-fog.md)) is visible. Root cause is the test itself, not ε, the
check stride, BLA/LA or float32 range (measured 2026-10-17, M1 Pro, release: at limit 100 000
the same pixels are proven, 3.6 % of 3.7 %).

**Method** (our own; Newton on the cycle as in mathr's interior DE, src:
[mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §Practical interior distance rendering, plus a
Kantorovich bound for a proof). A pixel unresolved at the limit continues its orbit:

1. Scan: base point `z_B` = smallest `|z|` so far (the cycle's point nearest 0, where `z` has
   full relative precision). A return within `|z_B|/4` after `k` steps is a candidate `P = k`
   if `z_B` was the minimum for `k` steps before it too.
2. Evaluate `F = f^P` at `z_B`: `g = F(z_B) − z_B`, `λ = F′(z_B)`, `F″(z_B)` (accumulated:
   `F″ ← 2 z F″ + 2 λ²` before `λ ← 2 z λ`). Newton step `Δ = −g/(λ − 1)`. With `L = 2|F″|`,
   float32 noise `η` in `g`, `h = L(|g| + η)/|λ − 1|² ≤ 1/4` and `r = 2(|Δ| + η/|λ − 1|) ≤ |z_B|/64`,
   a fixed point `z*` of `F` lies within `r`, with `|F′(z*)| ≤ |λ| + L r`. If that is
   `≤ 1 − μ`, `f` has an attracting cycle and `c` is interior (an exterior `c` has none).
3. Otherwise Newton moves the base point to `z_B + Δ` (≤ 8 passes); a located but surely
   repelling fixed point (`|λ| − L r > 1 + 2^-10`: the parent cycle inside a satellite bulb
   of period `qP`, e.g. P 1 inside the 1/10 bulb, or a ghost on an exterior orbit) resumes
   the scan from the saved orbit point for periods `> P` only; a hopeless candidate also
   raises that floor (the next candidate is `f^(jP)`, after more convergence); a fixed point
   located at the noise floor but not provably attracting (near-parabolic) gives up.

Noise: `η = 2^-22 (1 + √P)(|δ| + |δ_B|` (+ `|Z_m| + |Z_mB|` if the reference indices
differ)`)`, `μ = 2^-20 (P + 16)`. Measured margin: either bound alone 2^12 times smaller still
gave no false interior at neck-2e5 and root-third against the CPU at 100 000; both 2^8 times
smaller (2^-30, 2^-28) gave 0.004 % and 0.002 % false interior there. Near-parabolic pixels
(`|λ| ≳ 0.99–0.998`: the −3/4 neck, the 1/3 root, the cusp's rim) cannot be proven in
float32 and stay unresolved.

**As built** (`mandelbrot.metal`, "Attracting-cycle probe"; scheduler `probe_*`):

- Float32 steps in the plain kernel, floatexp in the deep one. `λ` and `F″` carry exponents.
- Its registers cost every main-loop pixel 10–20 % when compiled into the main pipeline
  (sea-8007 120 → 145 ms with the probe off). So it is a function-constant variant (kProbe,
  constant 6): an unresolved pixel *parks* (stays live at `n = maxIter`), and the scheduler
  switches to the kProbe pipeline when the last chunk read back held parked pixels only and
  either everything is admitted or they fill half the live capacity. State in a separate
  48-byte buffer (binding 30, two halves); `PixelState` unchanged.
- Limits per pixel: `maxIter/8 + 256` steps, at most 2048 (a few failing pixels form a
  latency-bound tail, ~1.2 µs per step), and at most half the level's work so far. Probe
  chunks cap steps at `max(128, 2^24 / live)` so long probes are compacted. Off for previews
  and gesture frames, for a nucleus reference with `4 P >` the limit, and for the rest of a
  level (and its extension and AA levels) once 4096 probed pixels show < 5 % success.
- Always on, divisor 8 (the switch `ALMONDBREAD_PROBE` was removed 2026-10-17).

**Results** (measured 2026-10-17, M1 Pro, release; `verify`, GPU at the view's limit vs the
CPU oracle at a high limit; "proven" = GPU detected / CPU non-escaping; false interior 0 in
every view):

| view | CPU interior | proven before | after | unresolved before | after |
|---|---:|---:|---:|---:|---:|
| satellite (800×500) | 3.7 % | 1.1 | 3.4 | 2.84 % | 0.54 % |
| period-2 bulb (−1, 7.5e-4/px) | 84.5 | 66.8 | 83.9 | 18.3 | 1.2 |
| main cardioid edge (0.3272+0.0562i) | 86.1 | 55.7 | 84.0 | 31.0 | 4.4 |
| p2 bulb edge (−0.8649+0.2104i, 1.25e-7/px) | 57.9 | 10.7 | 47.8 | 71.3 | 34.3 |
| satellite-head | 40.0 | 4.9 | 32.4 | 36.4 | 13.1 |
| satellite-antenna | 23.9 | 4.1 | 21.7 | 21.3 | 3.7 |
| period-4-minibrot (place) | 7.5 | 5.5 | 7.4 | 2.0 | 0.2 |
| home-100k | 15.8 | 12.9 | 15.7 | 3.1 | 0.24 |
| minibrot-p4 | 11.6 | 8.7 | 11.5 | 3.0 | 0.23 |
| elephant | 28.3 | 5.6 | 27.6 | 23.6 | 1.7 |
| cusp-1e5 | 97.5 | 35.7 | 87.9 | 61.9 | 9.9 |
| feigenbaum, neck-2e5, neck-7e6, root-third, sea-8007 | | 0 | 0 | unchanged | |
| needle-1e94 / 1e305 / 1e1004 | 1.2 / 1.3 / 0.6 | 1.1 / 1.1 / 0.4 | same | | |

Settle time in the app scheduler (`almondbread progress`, 1600×1000, plain settings) with
extension passes: satellite 0.18 → 0.19 s, unresolved 6597 → 331 pixels; cardioid edge
0.26 → 0.20 s (140 489 → 23 235); elephant 43 034 → 9 456; cusp 0.80 → 0.51 s
(276 810 → 126 074); satellite-head 0.88 → 0.32 s (216 268 → 10 256). Offscreen renders
without extension passes (`bench`) pay for the probe: see [almondbread benchmarks and verification record](../synthesis/benchmarks.md).

**Known limits.** Periods above ~400 (2048 steps) are not proven: Feigenbaum (P 2048+),
sea-8007, deep needles. Where the probe needs about its whole step budget (the satellite
head's P 78 bulb near its root at limit 2743), success varies from pixel to pixel: the edge
of the proven region shows steps along horizontal lines, and small minibrots near the limit
come out speckled black and grey at 3×3 supersampling; extension passes (higher limit) clear
most of it. A slow exterior pixel that escapes during the probe is still reported unresolved.

## Closed-form component test (built 2026-10-17)

**Problem.** Interior points near the boundary of the main cardioid or the period-2 bulb have
`|λ|` close to 1 (parabolic near the −3/4 root and the 1/4 cusp), so neither `|w| n² < ε` nor
the attracting-cycle probe proves them within any sane limit. At the −3/4 neck at zoom 4176
the whole bulb half of the view stayed unresolved fog; compute further spent seconds and they
ended black anyway. Both components have exact tests (see "Cardioid and bulb" above).

**Method** (our own; `almondbread_core::components`, kernel "Closed-form component test").
The kernel maps texel offset `(u, v) = (pix.x, −pix.y)` to `c = A + T R (u, v)` (`A` = the
reference point plus the f32 `refOffset`, `T` the texel, `R` the rotation). Per level the CPU
expands `f_card(c) = q(q + X) − y²/4` and `f_bulb(c) = (x + 1)² + y² − 1/16` as polynomials of
degree ≤ 4 in `(u/L, v/L) ∈ [−1, 1]²` (`L` a power of two), 15 coefficients, in BigFloat. The
precision is `2·log2(1/(L T)) + 140` bits, raised if the rounding bound says so: the constant term cancels down to the
quadratic terms. Coefficients are normalised by the largest one's power of two and rounded to
f32. The kernel calls a fresh texel inside iff `S < −(2^-16 |S|₁ + slack)`, where `S` is the
float32 sum of the 15 terms and `|S|₁` the sum of their magnitudes. Float32 evaluation is off by at most
~25 ulps of `|S|₁` in any summation order, with or without FMA, so `2^-16` leaves a factor 10.
`slack` covers the CPU's rounding (bounded from the inputs' magnitudes), coefficients dropped
below 2^-60 and flush-to-zero. Inside texels are written interior without iterating, in every
level type (previews and gesture frames too, ~50 flops); exterior texels are never touched.
An f64 bound skips the BigFloat work and the kernel test when the view cannot reach a
component, and a view the constant term proves wholly outside gets no test.

- **Why not float-float per pixel.** `c` in float-float carries 48 bits, so it fails at
  zoom ~1e13. Taking `c − A` to float-float only moves the problem: the polynomial's value is
  a cancellation of O(1) terms down to `(L T)²`. The exact expansion on the CPU keeps all of
  it, so the kernel works in float32 at any depth.
- **Precision and depth bound.** No depth limit: tested at texels 2^-8 … 2^-300, and the
  expansion at 2^-1000 takes under 5 ms per level on the CPU (cached across a level's AA and
  extension passes). The unprovable band is `2^-16 |S|₁ / |∇S|`, at most ~`2^-16 L` texels
  (0.03 texel at 2048 texels). Tests at the cusp, the root and 8 boundary points, rotated, find
  no false inside, including texels a few f32 ulps from the boundary. Every inside texel 1e-2
  texel or more from the boundary is proven (`components::tests::never_false_inside`).
- **Interior colourings** (`--inside`): the period is 1 or 2, exactly. The multiplier is
  `λ = 1 − √(1 − 4c)` or `4(c + 1)`, the closest approach `|λ|/2` or `min |z₀|, |z₁|` of the
  2-cycle `z² + z + c + 1 = 0`, from `c` in float32, as precise as the cycle analysis there.
  The interior distance uses the closed form `b` (`(1 − |λ|²)|√(1−4c)|/2` for period 1, the
  general formula with `F = f∘f` for period 2) only where float32 `c` resolves the texel (texel ≥
  2^-16 |c|) and `1 − |λ|² ≥ 2^-10`. Elsewhere it is `2 L |S| / |∇S|`: the closed form tends to
  twice the true distance at the boundary for both components (checked numerically: ratio
  1.98–2.00 at `1 − |λ|` = 1e-2…1e-4). That is exact at any depth near a smooth boundary. At the
  cusp, a singular point of `f_card`, it is the first-order value (off by up to ~3×), which
  draws horizontal bands there. The old cycle analysis drew curved bands at the neck, from
  orbits not yet settled; the exact `b` has straight ones.
- **Interaction.** The auto-iteration learner no longer sees a large unresolved area at the
  neck, so it does not re-render at 4× the limit. The 1-texel exterior neck between the
  two components is left to the extension passes. The M5 tile fill now has little to fill
  near the cardioid (its regression test moved to the period-3 bulb).

**Results** (measured 2026-10-17, M1 Pro, release; `verify` against the CPU oracle; class
mismatch and GPU-interior/CPU-exterior unchanged in every view, 0 GPU-int/CPU-ext):

| view | unresolved before | after | GPU ms before | after |
|---|---:|---:|---:|---:|
| −3/4 neck at 4176× (−0.74991217511+3.9165e-4i, 1600×1000) | 45.16 % | 0.05 % | 25 | 2.5 |
| cusp 0.25 at 1e-9 (800×500; render 1600×1000) | 100 % | 0.002 % | 210 | 4 |
| p2 bulb edge (−0.8649+0.2104i, 1.25e-7) | 34.26 | 31.30 | 236 | 196 |
| p2 bulb (−1, 7.5e-4) | 1.23 | 0.91 | | |
| home | 0.41 | 0.33 | 6.3 | 4.2 |
| seahorse 1.3e-4 | 3.50 | 2.16 | 17 | 8 |
| cardioid edge (0.3272+0.0562i, 1e-4) | 1.13 | 0.93 | | |
| neck at 4.48e-12 (−0.749999947+3.69e-4i) | 33.68 | 33.68 | 26 | 26 |
| −0.6443−0.3662i at 8.3e5 | 78.96 | 78.96 | 195 | 199 |

The last two views are not in either component. The neck at 4.48e-12 lies ~5e-8 outside both;
its non-escaping pixels are slow orbits or other components. −0.6443−0.3662i is inside a
period-64 component with `|λ|` 0.995 (`f_card = +3.5e-3`): only the probe or a longer limit
can help there. Bench set (1600×1000, GPU ms): cusp-1e5 115.6 → 12.5, elephant 24.8 → 11.7,
neck-2e5 17.1 → 13.2, root-third 38.5 → 33.1; GPU-detected interior neck-2e5 0 → 43.6 %,
root-third 0 → 32.0 %, cusp-1e5 88.1 → 97.5 %. App scheduler (`progress`, neck at 4176×,
3200×2000): settled 6.1 s with 2.6 % unresolved at the 200 k cap → 0.63 s with 792 texels;
`verify --history jump` at 1600×1000: resolved 718 → 160 ms after input.

## Component Newton (built 2026-10-17)

**Problem.** Views touching a hyperbolic component other than the two with closed forms
showed a wide fog band inside the component's edge that never resolved (user, foreground).
There `|λ|` is 0.99–0.999, so the orbit needs thousands of periods to settle: the `|w| n²`
test fails, and so does the attracting-cycle probe, whose scan looks for the cycle along the
orbit within `maxIter/8 + 256` steps. Test views: A −0.78732917218172383+0.13759431955408224i
at 1.48e-6/px (a period-22 component), B −0.6443094650576259−0.36623470659542282i at
4.5e-9/px (period 64, nucleus ~30 view widths away, `|λ|` 0.995 at the centre).

**Observation** (our own; measured 2026-10-17, M1 Pro, f64 on 160×100 grids of A and B).
Newton on `F(z) = f_c^p(z) − z` from `z = 0` at the component's period converges for every
interior pixel, 5 passes on average and 6 at most, and never to an attracting cycle for an
exterior one. `0` is a point of the cycle at the nucleus, and the cycle point it continues to
stays in Newton's basin across the component. Newton does not care how slowly the orbit
converges (`|λ − 1|` stays large away from the root). From the 4th pass on the step at
least halved for every attracting pixel. Mathr's interior DE does the same Newton step, but
from the orbit (src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §Practical interior distance
rendering); starting at 0 with a known period is our own.

**Method.**

1. **Periods per view, CPU** (`almondbread_core::cycles`; scheduler `component_periods`).
   A 5 × 5 grid of sample points over the level. Per sample, the candidates are the indices of
   new minima of `|z_n|` up to 400 (atom domains). Keep the smallest candidate for which Newton
   from 0 converges to an attracting cycle. Periods already found are tried first. The result
   is the periods, most samples first, at most 4, whose `8 p` steps fit 4096. Shallow views
   iterate `c` directly in f64. Big references are perturbed (`z = Z_m + δ`, rebasing) in f64
   above texel 2^-900 and in DoubleExp below. The search is bounded by a step budget
   (600 000 steps in f64, 1/16 of that in DoubleExp, which is ~25× slower per step). It took
   0.04–0.5 ms in the measured views, including perturbed ones. It runs once per view, when its
   first probe chunk is
   encoded, and is cached across that view's AA and extension levels. A period is only a hint;
   the proof is per pixel.
2. **Per pixel, GPU** (kernel "Component Newton", in the probe pipeline). A pixel entering
   the probe first runs Newton from `z_B = 0` for each period: base delta 0 at reference index
   0, since `Z_0 = 0`. The first pass has `λ = F′(0) = 0` and steps to `F(0)`. Every pass is
   evaluated by the probe's `probeEval`, with the same Kantorovich bound, noise `η` and margin
   `μ` as the attracting-cycle probe. **So the proof argument is unchanged:** a fixed point of
   `f^p` exists within `r` of `z_B`, with `|F′(z*)| ≤ |λ| + L r ≤ 1 − μ`. Then `f` has an
   attracting cycle and `c` is interior. An exterior `c` has none, whatever base point Newton
   started from. Only the choice of base point is new. A period fails when a fixed point
   located at the noise floor is surely repelling or stuck, when a step exceeds 2, when the
   step does not halve from the 4th pass on, after 8 passes, or when an iterate escapes. Then
   the next period runs from 0, and after the last one the old scan runs from the pixel's
   orbit point with its own leads. The step limit grows by the periods' `8 p`. Previews and
   gesture frames have no probe.
3. **Depth.** It runs in both kernels: float32 in the plain one, floatexp in the deep one, and
   perturbed against the view's reference from `Z_0 = 0`. A forced-deep build (deep kernel and
   big references at 1e-6/px) proved A to 1.5 % unresolved with 0 false interior; B, elephant,
   root-third, neck-2e5, cusp-1e5 and satellite-head also had 0 false interior. Deep minibrot
   places do not use it: the probe is off there because the reference period is 830 to 8007,
   so `4 P` exceeds the probe limit.

**Bug fixed on the way.** A pixel that was mid-probe when a chunk of the lean pipeline ran
lost its probe state. Probe chunks and lean chunks interleave while fresh pixels are still
admitted. The lean kernel moved the pixel's slot but did not copy its `PixelStateProbe`, so the
next probe chunk read another pixel's state. In B at a fixed limit of 6000, every pixel admitted
after the live list first filled failed (42.0 → 73.3 % detected with the fix). The lean kernel
now copies the probe state of probing pixels it moves. The old probe had the same bug. Its
corrupted pixels mostly failed, and the verify runs before the change showed no false
interior.

**Results** (measured 2026-10-17, M1 Pro, release; `verify` 1600×1000 unless noted, GPU at the
view's limit against the CPU oracle; GPU-int/CPU-ext 0.000 % and class mismatch unchanged in
every view):

| view | unresolved before | after | GPU detected before | after |
|---|---:|---:|---:|---:|
| A (period 22) | 8.33 % | 4.43 % | 81.7 % | 85.6 % |
| B (period 64) | 79.14 | 22.28 | 16.5 | 73.3 |
| satellite-head | 12.13 | 7.87 | 33.1 | 37.3 |
| satellite (800×500) | 0.54 | 0.38 | 3.4 | 3.5 |
| elephant | 1.11 | 0.98 | 28.0 | 28.2 |
| home | 0.177 | 0.169 | | |
| seahorse, neck at 4176×, neck at 4.48e-12 (800×500), cusp-1e5, neck-2e5, root-third, needle-minibrot-1e91 (800×500) | unchanged | | | |

Against `neck_truth` at 10^7 (160×100), proven out of the true interior: A 79.0 → 85.3 % of
86.8 %, B 50.2 → 73.3 % of 83.5 %. Most of what is left in A, and 12.8 points of B, escapes
after the limit. The rest of B's interior lies in satellite bulbs of the period-64 component
(period `64 q`, beyond 400). App scheduler (`progress`, 1600×1000, plain settings, settle time
and texels left): B 20.6 s with 904 141 → 3.0 s with 84 776; A 0.44 s with 743 → 0.23 s with
1 501; satellite-head 0.70 → 0.66 s (5 619 → 4 917). Home, seahorse, the neck at 4176×, cusp,
satellite, elephant and needle-minibrot-1e91 were within ±0.01 s. With the app's settings
(compute further to 200 k): B 22.2 s with 57 % not finished → 3.4 s with 5.3 %; A 0.69 s with
2 012 → 0.75 s with 1 213. A leaves more slow exterior texels with plain settings because the
learner no longer re-renders at 4× the limit (as after the closed-form test). Offscreen renders
without extension passes pay for the Newton tries of failing parked pixels: satellite-head
114 → 148 ms GPU (best of 5, GPU shared with other work). A 160×100 regression test
(`component_newton_proves_slow_interior`) asserts ≥ 84 % proven in A and nothing proven that
the CPU sees escape by 20 000.

**Known limits.** Components whose period is above 400, or with `8 p` above 4096 steps, are
left to the scan and the extension passes; that covers B's satellite bulbs and deep minibrots.
Near-parabolic pixels (`|λ| ≳ 0.99–0.998`, depending on `L` and `η`) stay unprovable in
float32, as before: the −3/4 neck, the 1/3 root and the cusp. A component that no sample point
lands in gets no period. A view with one dominant component and many parked exterior pixels
pays up to `8 Σp` probe steps per exterior pixel before the scan.

## For almondbread

Use the derivative test as the default interior test: one extra complex
multiply per perturbation step, `A·der` per BLA/LA skip, ε = 1e-3
(`EPS2 = 1e-6`), clamp on growth. Enable it when the frame's statistics show
many maxIter pixels or a nucleus reference is in use (GBy's advice). Keep the
cardioid/bulb test, expanded exactly per level so it works at any depth (as built above). Skip periodicity checking and interior
DE in v1; interior DE is a later colouring feature. Store an interior flag per
pixel for colouring and for [Guessing, border tracing and rectangle subdivision](guessing-and-subdivision.md).
Related: [Reference selection: nucleus, period detection, Newton](reference-selection.md), [Bivariate linear approximation (BLA)](bla.md),
[Linear approximation with period-aligned stages and AT (Imagina)](linear-approximation-imagina.md).

# Citations

- [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md)
- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
- [Wikibooks: Mandelbrot set interior](../sources/wikibooks-mandelbrot-interior.md)
- [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
