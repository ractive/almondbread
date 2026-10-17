---
title: "Interior detection"
type: technique
status: draft
sources: ["[[sources/cheritat-mandelbrot-rendering]]", "[[sources/fractalforums-interior-detection]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/mathr-interior-distance]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/wikibooks-mandelbrot-interior]]", "[[sources/wikibooks-fraktaler-3]]", "[[sources/kf-manual]]", "[[sources/imagina-hinfla-evaluator]]"]
tags: [interior, derivative, periodicity, performance]
---

# Interior detection

Pixels inside the set never escape, so without a test they run to maxIter. Near
minibrots at depth, maxIter is often 1e6 to 1e9, so interior pixels dominate the
cost. Interior detection stops them early. It also makes a very large maxIter
cheap, which removes most of the need for hand-tuned iteration limits
(see [[techniques/auto-iterations]]).

## Methods compared

| Method | Cost | Works at depth? | Notes |
| --- | --- | --- | --- |
| Cardioid / period-2 bulb test | ~6 flops per pixel, once | no (view almost never inside) | exact, only for the two biggest components |
| Periodicity check (saved iterate, Brent) | 1 compare per iteration + bookkeeping | poor in float32 | tolerance must match precision |
| Derivative test `\|dz/dz₁\| < ε` | 1 complex mul per iteration | yes | the recommended method |
| Interior DE (Newton on the cycle) | Newton solve per candidate period | yes with perturbation | exact radius, expensive |

### Cardioid and bulb

Main cardioid: with `q = (x − 1/4)² + y²`, the point is inside if
`q·(q + (x − 1/4)) ≤ y²/4`. Period-2 bulb: `(x + 1)² + y² ≤ 1/16`. No exact
test exists for higher bulbs (src: [[sources/wikipedia-plotting-algorithms]] §Cardioid / bulb checking).
Only useful at low zoom; harmless to keep since it costs a few flops per pixel (inference).

### Periodicity checking

Keep a saved iterate; if the current `z` comes back to it within a tolerance,
declare interior. Refresh the saved point periodically (Wikipedia's example
every 20 iterations), or use Brent's power-of-two schedule to catch any period
(src: [[sources/wikipedia-plotting-algorithms]] §Periodicity checking). The
tolerance is the weak point: rounding can also make the detected period a
multiple of the true one (src: [[sources/wikipedia-plotting-algorithms]] §Interior distance estimation).
Under perturbation in float32 the full orbit `Z+z` only has ~1e-7 relative
resolution, so it cannot resolve cycle closure at deep zoom; comparing deltas
only works if the reference is periodic too (inference). Treat it as secondary.

### Derivative test (recommended)

Track the derivative of the orbit with respect to its starting point,
`der = (Pⁿ)′`, which is the product of `2z` along the orbit. If the parameter is
hyperbolic the orbit falls into an attracting cycle and the product tends to 0.
Stop when `|der| < ε` and call the pixel interior
(src: [[sources/cheritat-mandelbrot-rendering]] §Following the derivative).

- Start from `z = c, der = 1` (starting at the critical point 0 would multiply by 0 at once).
- Update `der` **before** `z`; `z = z² + c; der = der·2z` uses the new `z` and is wrong
  (src: [[sources/cheritat-mandelbrot-rendering]] §Following the derivative).
- ε = 0.001 gives good images; ε = 0.1 visibly misclassifies near parabolic
  parameters (e.g. the neck near c = −3/4)
  (src: [[sources/cheritat-mandelbrot-rendering]] §Images and speed).
- Each iteration costs about 2× a plain iteration, but a huge N stops mattering:
  "N = 10⁶ is OK, but avoid N = 10¹⁰"
  (src: [[sources/cheritat-mandelbrot-rendering]] §Images and speed).
- False positives exist (some non-hyperbolic `c` have `liminf |der| = 0`) but are
  rare and lie close to the set (src: [[sources/cheritat-mandelbrot-rendering]] §The idea).

**Under perturbation and BLA.** GBy (fractalshades) iterates `∂z_n/∂Z₁` and stops
when it drops below ε. It "seems to work surprisingly well in combination with
perturbation and Bilinear approximation"; ε = 0.001 or even 0.01 is safe in most
cases. His examples need 1e7 iterations at period 14292, and a period-57853
minibrot at 5e-433 with 4e8 max iterations renders 3200×3200 in under 5 minutes.
He advises activating it only when a minibrot is in view, because it costs an
extra quantity per iteration (src: [[sources/fractalforums-interior-detection]] §Interior detection for deep Mandelbrot set zooms).
mathr notes that without interior checking KF needed NanoMB1 to make such a
location feasible at all (src: [[sources/fractalforums-interior-detection]] §Reply #9).

mathr's deep-zoom page states the rule: track the derivative with respect to
`z₁`, classify as interior when its magnitude drops below a threshold such as
0.001. Using `dz/dz₁` (the delta's derivative) instead of the full
`d(Z+z)/d(Z₁+z₁)` works when the reference is periodic with 0 in its orbit, i.e.
the reference is the nucleus of a hyperbolic component, because then the
reference's own contribution `dZ/dZ₁` is 0. For non-conformal formulas use dual
numbers with four dual parts (two for DE, two for interior) and the matrix
operator norm (src: [[sources/mathr-deep-zoom-page]] §6 Interior Detection).

Imagina carries `dzdz` (and `dzdc`) through LA steps: each LA step multiplies
the derivative by `2(dz + Ref)·ZCoeff`, so the test survives iteration skipping
almost for free (src: [[sources/imagina-hinfla-evaluator]]). With BLA the same
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
the exterior DE derivative is separate ([[techniques/distance-estimation]]).

### Interior distance estimate

mathr's practical method (src: [[sources/mathr-interior-distance]] §Practical interior distance rendering):

1. Each new minimum of `|z_n|` gives a candidate period `p` (atom domains).
2. Newton-solve `f^p(z₀) = z₀` with `z₀ ← z₀ − (f^p(z₀) − z₀)/((f^p)′(z₀) − 1)`,
   stop at step < 1e-10, at most 64 steps.
3. If `|d| = |(f^p)′(z₀)| ≤ 1` the point is interior; then
   `DE_int = (1 − |d|²) / |∂c d + ∂z∂z f^p · ∂c f^p / (1 − d)|`.

Strategies: plain (no checks), unbiased (check at every new minimum), biased
(if the previous pixel was exterior, defer checks to maxIter). Biased was best
overall. Near double-precision limits interior DE fails visibly while exterior
DE still works; fix with more precision or perturbation around the island's
nucleus (src: [[sources/mathr-interior-distance]] §Practical interior distance rendering).
All points within `b/4` of `c` are interior; a period detected as a multiple of
the true one overestimates `b` (src: [[sources/wikipedia-plotting-algorithms]] §Interior distance estimation).
The multiplier test `|d| ≤ 1` alone (without the distance) is the cheap part
(src: [[sources/wikibooks-mandelbrot-interior]]).

### Other

- Neighbour heuristic: a pixel is probably interior if it and all neighbours are
  interior with the same period (src: [[sources/wikibooks-mandelbrot-interior]]).
- KF2+ "interior checking" exists only for power-2 Mandelbrot with NanoMB1/2, is
  experimental and "may or may not speed up" (src: [[sources/kf-manual]]).
- Fraktaler 3 EXR metadata shows `bailout.inscape_radius = 0.000976562` (2⁻¹⁰);
  plausibly the interior threshold, meaning unverified
  (src: [[sources/wikibooks-fraktaler-3]]) (unverified).

## Pitfalls

- Wrong update order of `der` and `z`.
- ε too large (≥ 0.1) fills necks and thin parts near parabolic points.
- With a non-periodic reference (e.g. view centre), `dz/dz₁` is not the full
  derivative; track `d(Z+z)` instead or use a nucleus reference (inference from
  [[sources/mathr-deep-zoom-page]] §6 Interior Detection).
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
  failures but unresolved, see [[techniques/auto-iterations]]. Elephant
  (detected 26.9 % → 23.3 %) and feigenbaum (24.4 % → 15.0 %) get slower; see
  [[synthesis/benchmarks]].
  Shortcuts that confirm a candidate by "no escape for k·n more iterations" or
  "`|w| n²` non-increasing" fail on an orbit that enters a second, much longer
  gate early. Its `|w| n²` grows only slowly there (inference from the gate model
  above). Do not reintroduce them without a regression view.
  Regressions: bench locations `neck-2e5`, `neck-7e6`, `cusp-1e5` and
  `root-third`. `--bench` fails above 0.02 % GPU-int/CPU-ext, and `make regress`
  runs the scripted zoom sessions.

## Measured in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, release; tables in [[synthesis/benchmarks]].

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
  [[techniques/auto-iterations]], "Nothing escaped"). After the ×4 render, the switch sees
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
the grey unresolved veil (2026-10-17) is visible. Root cause is the test itself, not ε, the
check stride, BLA/LA or float32 range (measured 2026-10-17, M1 Pro, release: at limit 100 000
the same pixels are proven, 3.6 % of 3.7 %).

**Method** (our own; Newton on the cycle as in mathr's interior DE, src:
[[sources/mathr-interior-distance]] §Practical interior distance rendering, plus a
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
- `ALMONDBREAD_PROBE=0` switches it off, `=n` sets the divisor.

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
without extension passes (`bench`) pay for the probe: see [[synthesis/benchmarks]].

**Known limits.** Periods above ~400 (2048 steps) are not proven: Feigenbaum (P 2048+),
sea-8007, deep needles. Where the probe needs about its whole step budget (the satellite
head's P 78 bulb near its root at limit 2743), success varies from pixel to pixel: the edge
of the proven region shows steps along horizontal lines, and small minibrots near the limit
come out speckled black and grey at 3×3 supersampling; extension passes (higher limit) clear
most of it. A slow exterior pixel that escapes during the probe is still reported unresolved.

## For almondbread

Use the derivative test as the default interior test: one extra complex
multiply per perturbation step, `A·der` per BLA/LA skip, ε = 1e-3
(`EPS2 = 1e-6`), clamp on growth. Enable it when the frame's statistics show
many maxIter pixels or a nucleus reference is in use (GBy's advice). Keep the
cardioid/bulb test for shallow views. Skip periodicity checking and interior
DE in v1; interior DE is a later colouring feature. Store an interior flag per
pixel for colouring and for [[techniques/guessing-and-subdivision]].
Related: [[techniques/reference-selection]], [[techniques/bla]],
[[techniques/linear-approximation-imagina]].
