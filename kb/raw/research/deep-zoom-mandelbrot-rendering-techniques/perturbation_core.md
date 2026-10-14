# Perturbation-Theory Deep-Zoom Rendering: Core Mathematics (state of the art, 2026)

Notation used throughout (follows mathr / Claude Heiland-Allen): uppercase = high-precision reference (`Z_m`, `C`), lowercase = low-precision per-pixel delta (`z_n` = dz, `c` = dc). Pixel orbit = `Z_m + z_n`, pixel parameter = `C + c`. Note: reference index `m` and pixel iteration count `n` diverge once rebasing is used.

Source-access caveat: fractalforums.org (Zhuoran's original thread #4360, BLA thread #4802) returned HTTP 429 on every attempt, so Zhuoran's own wording is quoted via mathr's write-ups, which describe the method in that thread's first post.

---

## 1. Perturbation formula and history

### Takeaway
Iterate one reference orbit `Z_{m+1} = Z_m^2 + C` in arbitrary precision. For every pixel, iterate only the delta `z_{n+1} = 2 Z_m z_n + z_n^2 + c` in hardware floats. This idea came from K.I. Martin's SuperFractalThing (2013) and was industrialised in Kalles Fraktaler. It is still the foundation in 2026, and every modern renderer adds rebasing plus BLA on top.

### Cited Findings
- Reference orbit, high precision: `Z_{m+1} = Z_m^2 + C`, `Z_0 = 0`. Pixel deltas, low precision: `z_{n+1} = 2 Z_m z_n + z_n^2 + c`. — [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)
- Derivation: substitute `Z+z`, `C+c` into `z^2+c` and cancel the reference terms symbolically. Computing `(Z+z)^2 + (C+c)` directly and then subtracting causes "catastrophic absorption", so the symbolic delta form is mandatory. The reference orbit is computed in high precision and rounded to double (or float) for storage. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Wikipedia form: `ε_{n+1} = 2 z_n ε_n + ε_n^2 + δ`, with ε_0 = 0 (or ε_1 = δ). For most iterations ε needs no more than ~16 significant figures, so hardware FP suffices. Credited to "Superfractalthing – Arbitrary Precision Mandelbrot Set Rendering in Java" (fractalforums) and K.I. Martin's "Superfractalthing Maths" PDF. — [Wikipedia: Plotting algorithms](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- Series approximation (SA, SuperFractalThing): `ε_n = A_n δ + B_n δ^2 + C_n δ^3 + …` with `A_1=1, B_1=0, C_1=0`, `A_{n+1} = 2 z_n A_n + 1`, `B_{n+1} = 2 z_n B_n + A_n^2`, `C_{n+1} = 2 z_n C_n + 2 A_n B_n`. The coefficients depend only on the reference, so many initial iterations can be skipped for all pixels at once. The page claims a speedup of about two orders of magnitude. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **[SUPERSEDED]** SA validity is checked against probe points: "roll back one iteration" when the series deviates. "There is still no complete mathematical proof of correctness." KF also had biseries/NanoMB1/NanoMB2 (knighty) for periodic references, marked "highly experimental". — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- BLA is "conceptually simpler, easier to implement and parallelize, has better-understood stopping conditions" than SA, and generalises to hybrids. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Kalles Fraktaler 2 is cited by Wikipedia as a perturbation implementation. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set); KF2+ manual: [mathr.co.uk/kf/manual.html](https://mathr.co.uk/kf/manual.html)
- Ultra Fractal also implements perturbation and documents the need for a new reference "because the original reference orbit differs too much". — [Ultra Fractal help](https://www.ultrafractal.com/help/formulas/perturbationcalculations.html)

### Inferences
- In 2026 the pipeline to implement is: high-precision reference on the CPU, a BLA table built from the stored reference, and a GPU per-pixel loop of {try longest valid BLA skip, else one perturbation step, then rebase check}. SA, multi-reference glitch correction and NanoMB are legacy.
- Pixel delta `c` should be computed as `(pixel − reference_pixel) × pixel_spacing`, both in the low-precision type (or floatexp), never by subtracting two high-precision coordinates on the GPU.

### Gaps
- The exact 2013 date of SuperFractalThing was not confirmed. Wikipedia's archive link is dated 2014.

---

## 2. Glitch detection (Pauldelbrot) and the legacy multi-reference approach

### Takeaway
A glitch happens when the pixel orbit `Z+z` passes near 0 (the critical point) while the reference does not. Pauldelbrot's test `|Z+z|^2 < G·|Z|^2` detects it. Before 2021 the fix was to re-render glitched pixels with new references, possibly recursively. This is **superseded by rebasing** for the Mandelbrot set, but the test is still useful as a diagnostic.

### Cited Findings
- Criterion: a glitch is possible when `|Z+z| << |Z|`. Test `|Z+z|^2 < G·|Z|^2`, with "G somewhere between 1e-2 and 1e-8, depending how strict you want to be". `|Z|^2` can be precomputed per reference iteration and stored. G too large is slow, too small misses glitches. The fix is to recompute glitched pixels with a new reference, possibly repeatedly. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Pauldelbrot's original "magic number" was 0.001. knighty later proposed an interval-arithmetic method that saw little adoption. — [fractalforums.com KF 2.7 thread](https://www.fractalforums.com/kalles-fraktaler/kalles-fraktaler-2-7/15)
- KF exposes a "glitch low tolerance" checkbox (off by default). Enabling it took "16x longer" on one test location. Since 2.15.3 the tolerance can be a fraction between 0 and 1: 1 gives good images but is very slow, 0 is fast but can give bad images. Glitches are always detected with the 2-norm. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **[LEGACY]** KF multi-reference: methods for choosing the next reference among glitched pixels are "Original", "argmin|z|" and "Random". Max references defaults to and is hard-limited at 10000. "Special locations can produce endless references." — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Wikipedia's version: measuring the orbit distance between the reference and the low-precision point detects points that cannot be computed correctly. These are recalculated from a closer reference. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)

### Inferences
- With rebasing (§3), Pauldelbrot detection is unnecessary for power-2 Mandelbrot. It can still serve as a debug overlay to verify that a float32 path is not silently producing glitches.

### Gaps
- No primary source found quantifying an optimal G for float32 specifically. Since float32 epsilon is 2^-24 ≈ 6e-8, the strict end of the range (1e-8) is below float32 resolution, which is an inference.

---

## 3. Zhuoran's rebasing (2021)

### Takeaway
Whenever `|Z_m + z_n| < |z_n|`, or the reference runs out (m reaches its end), set `z ← Z_m + z` and `m ← 0`, then continue. The pixel's full value becomes the new delta against the start of the same reference orbit (`Z_0 = 0`). This makes glitches impossible by construction, so one reference suffices for power-2 Mandelbrot. Multiple critical points or hybrids need one reference per critical point or phase.

### Cited Findings
- Rebasing "means resetting the reference iteration to the start when the pixel orbit gets near a critical point". It avoids glitches rather than detecting them. It is explained in the first post of Zhuoran's forum thread. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Exact trigger: rebase when `|Z_m + z_n| < |z_n|`, switching to the full orbit value and resetting the reference index. After every perturbation step, check for rebasing. — [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)
- Multiple critical points: pick the orbit o that minimises `|(Z − Z_o) + z|`, using the current reference at the current iteration and each critical-point orbit at iteration 0. Hybrids need one reference per phase (e.g. "(M,BS,M,M)" needs 4). These can be computed in parallel. Status (2022): implemented only for a single critical point at 0, so bugs may remain. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- KF 2.15.5 added a "single-reference implementation for avoiding glitches (thanks Zhuoran)", enabled by default. Known issues: does not work with every hybrid formula, may fail with more than one critical point, and "single reference method does not cure all glitches". — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Thread IDs: KF manual cites fractalforums.org topics 4360 (rebasing) and 4802 (BLA). — [KF2+ manual](https://mathr.co.uk/kf/manual.html), via search summary
- A 2021 independent account: rebasing was found in August 2021, and Imagina (Zhuoran) shipped the "HInfM" algorithms. — [nostr post](https://nostr.at/nevent1qqsv82wequ5qey0wnzvecuyahtypzuhpfprw095qdh8a7qn6emuz82cpz4mhxue69uhhyetvv9ujumt0wd68ytnsw43qr0f2vm) (secondary, low authority)
- A float32 WebGPU implementation: "when |z| < |w|, the pixel restarts against the beginning of the orbit", replacing glitch detection and second references. Bug found: with an extended-range type stored as `Hdr(x,0)` (magnitude in mantissa, exponent 0), the rebase test compared exponents first. At a minibrot nucleus rebasing never fired and the frame rendered black. — [Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)

### Pseudocode (from the above sources, plus BLA lookup from §3/§4 sources)
```
m = 0; z = 0                      // z: delta, type T (float, floatexp, ...)
for n in 0..<maxIter:
    // 1. BLA skip (optional)
    if let (l, A, B) = bla.longestValid(m, |z|):   // |z| < R_{m,l}
        z = A*z + B*c; m += l; n += l-1           // n counts real iterations
    else:
        z = 2*Zref[m]*z + z*z + c; m += 1
    Zfull = Zref[m] + z
    if |Zfull|^2 > escapeR^2: break               // escaped
    if |Zfull|^2 < |z|^2  or  m == refLen-1:      // rebase
        z = Zfull; m = 0
```
(Comparisons use squared norms. With an extended-range type, compare normalised magnitudes, not raw exponents. See the Desarso bug above.)

### Why a single reference suffices (reasoning from the cited definitions)
- Glitches arise when the delta becomes comparable to or larger than the full value, so precision is lost in the `2Zz` term relative to the true orbit. Rebasing whenever `|Z+z| < |z|` keeps `|z| ≤ |Z+z|`, so the delta never dominates. Resetting to `Z_0 = 0`, which is exactly the critical point, means the new delta equals the full value with no cancellation. Since any reference orbit starts at the critical point, running out of reference iterations is handled by the same reset. The reference needs only to be long enough to be useful, not as long as the deepest pixel.

### Inferences
- Rebasing interacts with BLA as follows. After a rebase, `m = 0`, and BLA tables start at m = 1, because "iteration 0 starts at a critical point and gives a validity radius of 0" ([mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)). So one ordinary step follows each rebase, then BLA lookups resume.
- The "reference exhausted" rule is what allows a short periodic nucleus orbit (§5) to serve pixels with much higher iteration counts.

### Gaps
- Zhuoran's original post text could not be fetched (HTTP 429). No primary source found for refinements since 2022 beyond the per-critical-point extension and the KF/Fraktaler 3 implementations.

---

## 4. BLA (bivariate linear approximation): the 2026 replacement for SA

### Takeaway
Precompute, per reference index m and skip length l, coefficients with `z_{m+l} ≈ A_{m,l} z_m + B_{m,l} c`, valid while `|z| < R_{m,l}`. Build single steps, then merge pairs into a binary tree of about 2M entries. On the GPU, take the longest valid skip.

### Cited Findings
- Single-step BLA `z → 2Zz + c` is valid when `z^2 << 2Zz + c`. Radius: `r = max(0, ε·(|Z| − max_image|c|)/(|J_f(Z)| + 1))`, where ε is the hardware precision (e.g. **2^-24 for float32**) and `J_f(Z) = 2Z` for Mandelbrot. The author halves |Z| for safety. The derivation is "a bit handwavy". — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Alternative statement: single-step validity `|z_n| << |A_{n,1}| =: R_{n,1}`. — [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)
- Merge, with x first and y second: `l = l_y + l_x`, `A = A_y A_x`, `B = A_y B_x + B_y`, `R = min(R_x, max(0, (R_y − |B_x|·max|c|)/|A_x|))`. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Table: M single steps, then M/2, M/4, … (2M total). Every level starts at iteration 1. Choose the longest skip from the current m with `|z| < R`. Otherwise do a normal perturbation step, rebasing as needed. One table per reference. The table can be built in parallel (even on GPU). Zhuoran considers this level construction suboptimal. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html); [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)
- Burning Ship / non-conformal: use 2×2 real matrices with radii from singular values, `R = ε inf|A| − sup|B|/inf|A| · |c|`, capped by `min{…, |X|, |Y|}`. — [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)
- Zhuoran's Imagina uses an "HInfLA" evaluator (filename `HInfLAEvaluator.cpp`), a variant of linear approximation. Imagina is being rewritten. — [Imagina GitHub](https://github.com/5E-324/Imagina) (inferred from filenames only)
- FractalShark ports FractalZoomer's float/exp linear and bilinear approximation to CUDA, crediting "Claude and Zhuoran". — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- Second-order extension: `w_out = A w + B δ + C w^2 + D w δ + E δ^2` pushes the first neglected term to cubic and allows larger entry deltas. Measured at 2.8e40: 21× faster, 96.9% of iterations skipped, bit-identical output. Over long skips, BLA coefficients reach ~1e700, overflowing double (radii become NaN), so coefficients and radii must be stored as extended-range values. — [Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)

### Inferences
- On float32 Metal, the BLA coefficients `A`, `B` and radii `R` must be stored as floatexp (float mantissa + int exponent) whenever the zoom is past about 1e-30. A float32 coefficient table would overflow or underflow far sooner than the ~1e700 that breaks a double table.
- Using ε = 2^-24 in the radius formula makes valid skips shorter than with double. That is the correct cost of float32 accuracy, and ε must not be inflated.

### Gaps
- No published side-by-side benchmark of BLA vs SA was found. mathr says "speed comparisons are still needed" (2022).

---

## 5. Reference point selection: center vs nucleus, period detection, Newton, atom domains, Misiurewicz

### Takeaway
Best practice is to find the period p of the lowest-period minibrot (atom) in or near the view using the ball method (or Munafo's box/Jordan-curve method). Then refine its nucleus with Newton on `z_p(c) = 0`, and use that nucleus as the reference. The reference orbit is then exactly periodic, so only p iterations need high precision, and rebasing on exhaustion wraps around. The center of the view works too, since rebasing removes the glitch risk, but costs `maxIter` high-precision iterations.

### Cited Findings
- KF Newton-Raphson zooming: actions Period, Center, Size, Skew. The **Ball Method is the default**, with the Box Method as alternative. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- kf-2.12.7: "use interval arithmetic ball-period method instead of box-period (speeds up Newton-Raphson zooming a bit) (idea from knighty and gerrit)". It later became the default and "works much better in skewed locations". Theory: knighty 2017 "A little note about Ball arithmetic". — [KF2+ manual](https://mathr.co.uk/kf/manual.html), via search summary
- Box/Jordan-curve method (Munafo): iterate the corners of a bounding rectangle and count edge crossings of the origin. Gives the period of a mu-atom known only to lie within the rectangle. — [mrob Jordan curve method](https://www.mrob.com/pub/muency/jordancurvemethod.html); [mrob Period](https://www.mrob.com/pub/muency/period.html)
- Newton for nucleus: solve `L_P(C) = 0` (P = period). `L_N(C)` and `L_N'(C)` are both computed by normal iteration, so polynomials need not be expanded. An approximate starting location is enough. — [mrob Newton-Raphson](https://mrob.com/pub/muency/newtonraphsonmethod.html)
- Newton-Raphson zooming: the nucleus position becomes "exact to within the precision being used", allowing large zoom steps. Size estimate from a series of reciprocals of partial products of the first p−1 iterates. Embedded Julia set sizes: s^(3/4) (Heiland-Allen) or s^(1/2) and s^(3/4) (Munafo) relative to island size s. — [mrob NR zooming](https://mrob.com/pub/muency/newtonraphsonzooming.html)
- Atom domains (Peitgen & Richter 1986): the dominant period is the iteration index at which `|Z_n|` reaches its minimum, excluding n=0. — [mrob Atom domain](https://mrob.com/pub/muency/atomdomain.html)
- Periodicity scan: box-period, then Newton, then a verification step. — [mathr Periodicity scan](https://mathr.co.uk/blog/2017-05-17_periodicity_scan.html)
- FractalShark Feature Finder locates periodic points and refines them with high-precision Newton, in Direct, Perturbation and LA modes. Its reference-orbit periodicity detection is taken from Imagina. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- KF option "Newton-Raphson zooming calculates a new reference after each zoom". NanoMB1 needs the period set. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Misiurewicz (pre-periodic) points: mathr notes that pre-periodic parts add more references when multiple critical points exist. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html); see also [Wikipedia Misiurewicz point](https://en.wikipedia.org/wiki/Misiurewicz_point) and [mathr preperiodic Newton basins](https://mathr.co.uk/blog/2013-01-29_preperiodic_mandelbrot_set_newton_basins.html)

### Pseudocode (standard formulations, NOT verified against a primary source in this session; treat as inference)
```
// Ball period (knighty/gerrit): disc of radius r around c
z = 0; dz = 0 (dz/dc); R = 0
for n in 1..maxPeriod:
    R  = 2|z|R + R^2 + r      // (some variants use |dz|*r instead: R_n ≈ |dz_n| r)
    dz = 2 z dz + 1
    z  = z^2 + c
    if |z| < R: return n      // disc image contains 0 -> period n candidate
// Newton for nucleus of period p (in MPFR, precision > zoom bits)
repeat:
    z = 0; dz = 0
    for i in 1..p: dz = 2 z dz + 1; z = z^2 + c
    c = c - z/dz
until |z/dz| < tiny
```
- Inference: Newton converges quadratically, so it needs about log2(bits) iterations, each costing p high-precision iterations. Doubling precision per Newton step (starting low) is a common optimisation.
- Inference: with a nucleus reference, `Z_p = 0`, which is exactly the critical point. Storing p+1 values and rebasing on exhaustion is therefore mathematically equivalent to an infinite periodic orbit. Rebasing to `Z_0` at m=p is a no-op in value (`z ← Z_p + z = z`).
- Inference: the reference only needs to lie near the view. Rebasing removes the old requirement that it be "inside" or long-lived. A nucleus outside the frame is fine, as long as `|c|` stays small enough for the float type and BLA radii.

### Gaps
- knighty's ball-arithmetic note was not fetched, so the exact ball recurrence (r term) is unverified.
- No primary source found for the Misiurewicz-as-reference strategy in single-critical-point Mandelbrot.

---

## 6. Reference orbit in arbitrary precision: precision bits, cost, parallelism, compression

### Takeaway
The orbit is inherently serial (each `Z_{m+1}` depends on `Z_m`). Speedups come from:
1. Fewer iterations, by using a periodic nucleus.
2. Faster bignum squaring: multithreaded component squaring on the CPU, or NTT on the GPU in FractalShark.
3. Lower memory, by compressing the stored low-precision orbit (Zhuoran/Imagina's method, extended by FractalShark).

### Cited Findings
- KF needs "up to ~40 bytes per reference iteration (depending on number type)". MPFR limits precision to 2 gigabits (~646 million decimal digits). — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- FractalShark CPU "MT3": three threads (two squaring, one coordinating), reported fastest on an AMD 5950X. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- FractalShark GPU Ref2/v2: a persistent fused CUDA kernel using Number-Theoretic-Transform multiplication, signed carry propagation and normalisation. The author reports ~10× over multithreaded MPIR+AVX2 on an RTX 4090 at 16384 32-bit limbs (~158k digits), ~9× on an RTX 5090, and ~70× over single-threaded MPIR (v0.54). He caveats that the multithreaded comparison was broken. The GPU may win beyond ~1e10000 zoom, and the CPU wins at shallower depths. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- FractalShark's deepest example is a minibrot at ~1e650452, found with GPU Newton and GPU reference orbits. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- Reference orbit compression was first implemented in Imagina (Zhuoran) for save/load. FractalShark extends it to decompress orbit segments on demand during rendering. A period-600,000,000 case saved multiple GB. Imagina-compatible files, compression levels 1–20. Details are in the FractalShark PDF §14 (not fetched). — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- "Perturbed perturbation": FractalShark caches intermediate-resolution reference values for reuse across zooms. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- KF "Reuse reference" keeps the reference for further zooming. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- A WebGPU renderer computes the reference on the GPU in fixed-point u32 limbs, emulating 32×32→64 multiplies with 16-bit partial products. One workgroup runs the serial recurrence with multiplication spread across 256 threads. 8 limbs reach ~1e-42. — [Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)

### Inferences
- Precision rule of thumb (inference, not found stated in a fetched source): bits ≈ log2(zoom magnification) + log2(image width in pixels) + guard bits (e.g. 32–64). For example, zoom 1e100 needs about 332 + 11 + 32 ≈ 380 bits. The reference coordinate must resolve the pixel spacing, and the orbit stays bounded by |Z| ≤ 2 before escape, so fixed-point or MPFR with a constant exponent works.
- Compression mechanism (inference: the algorithm is in FractalShark PDF §14 and Imagina source, both unread): the stored orbit is only float/double precision. Long stretches can therefore be regenerated in low precision from sparse "waypoints", recomputing `Z_{m+1} = Z_m^2 + C` in double and re-seeding from a stored exact value whenever the low-precision recomputation drifts beyond a tolerance. This trades GPU/CPU compute for memory.
- For Apple Silicon: MPFR or GMP on the performance cores, with real and imaginary parts squared in parallel threads (MT3-style: `X' = (X+Y)(X−Y) + Cx`, `Y' = 2XY + Cy`). A Metal NTT bignum is plausible but unproven. FractalShark only shows a win beyond ~1e10000.

### Gaps
- The exact compression algorithm (FractalShark PDF §14, Imagina source) was not retrieved.
- No source found giving an official precision formula used by KF or Fraktaler 3.

---

## 7. Extended range: floatexp, rescaled iterations, thresholds for float32/float64

### Takeaway
float32 deltas underflow below ~1e-38 (normal minimum 1.18e-38; denormals to ~1.4e-45). float64 deltas underflow at ~1e-308. Beyond those, either use floatexp (mantissa + separate int exponent, renormalised every op; slow) or **rescaled iterations**. Rescaled iterations keep a scale factor S, iterate `w = z/S` in plain hardware floats, and renormalise only every few hundred iterations or when `|Z|` is small. For float32-only Metal, rescaled float32 (with floatexp fallback for "full" iterations) is the practical way past 1e-38, and the same scheme is depth-unbounded.

### Cited Findings
- Range: "Double range underflows to 0 below about 1e-308". "Single precision underflows below about 1e-38 (relevant to deep needle locations)". "Double (53-bit) is more than enough for perturbed orbits; single (24-bit) can also work." — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Floatexp: a double mantissa normalised near 1 plus a separate integer exponent. Supports arbitrarily deep zooms but is slow, because every operation is software. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **Rescaling (Heiland-Allen)**: substitute `z = S·w`, `c = S·d`, giving `w → 2Zw + S·w^2 + d`. Choose S so |w| ≈ 1, and rescale when |w| risks over- or underflow, "typically every few hundred iterations" since |Z| ≤ 2 except near escape. If S underflows to 0 in hardware precision, drop the `S·w^2` term (when Z is not small), and likewise drop d if it underflows. **Full iterations** when Z is tiny: compute `z = S·w` in floatexp, iterate `z → 2Zz + z^2 + c` in floatexp, then set `S = |z|`, `w = z/S`, `d = c/S` (in floatexp, rounded to hardware), and compute the hardware-precision `s` for the next rescaled run. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- KF thresholds: single precision "for shallow zooms (until zoom e20)", disabled by default, and may cause "undetected glitches at some locations". Double SIMD "up to zoom depth 1e300 or so". Scaled double SIMD to ~1e600 (power 2) and ~1e400 (power 3). Rescaled perturbation (2.15.3) is "arbitrarily deep" for Mandelbrot powers 2 and 3, Burning Ship power 2 and hybrids. Known issue: "Rescaled Double is slow in some locations". Between ~1e300 and ~1e4900 the CPU beats OpenCL because OpenCL lacks x87 long double and falls back to floatexp. 2.15.3 added single-precision OpenCL for devices without fp64. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- KF rescaled SA is ~30% faster than all-floatexp. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Float32 WebGPU thresholds (units/pixel): >1e-5 direct f32 with no reference; >1e-25 perturbation with plain f32 delta; deeper uses an exponent-carrying delta (mantissa/exponent pair renormalised every iteration) plus the skip table. The 1e-25 handover is "thirteen decades above the f32 floor". The older WebGL path's plain f32 deltas underflowed to zero, which set its depth limit (~1e-34). Renormalisation bug: a single `exp2(-shift)` overflows when the mantissa falls below 2^-128 near a zero of the reference, fixed by rescaling in two halves. — [Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)
- FractalShark's "2×32" type: two float32s plus a shared exponent, ~48-bit effective mantissa, CUDA-only, used for speed on consumer GPUs. It also has HDR (float+exponent) types. — [FractalShark](https://github.com/mattsaccount364/FractalShark)
- Distance-estimate derivatives: use dual numbers with dual parts initialised to the pixel spacing, which "avoids overflow of the derivative". — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html); [mathr Deep Zoom](https://mathr.co.uk/web/deep-zoom.html)

### Pseudocode: rescaled float32 iteration for Metal (built from the mathr 2021 description; GPU details are inference)
```
// per pixel; c_fe is floatexp (float mantissa, int32 exponent)
S_fe = |c_fe|  (or pixel spacing);  w = float(c_fe / S_fe); d = float(c_fe / S_fe)
s = float(S_fe)               // may underflow to 0 -> then S*w^2 term is dropped
loop:
    if |Zref[m]| small (e.g. < some threshold) or |w| outside [2^-60, 2^60]:
        z_fe = S_fe * w                         // full floatexp iteration(s)
        z_fe = 2*Zref[m]*z_fe + z_fe^2 + c_fe
        S_fe = |z_fe|; w = float(z_fe/S_fe); d = float(c_fe/S_fe); s = float(S_fe)
    else:
        w = 2*Zref[m]*w + s*w*w + d             // pure float32 FMA path
    m++
    // escape / rebase tests need Z+z = Zref[m] + S*w: only meaningful when S is
    // not tiny; when s underflows, |S*w| << |Z| so no rebase or escape is possible
    // except when Zref[m] itself is tiny -> that's the floatexp branch.
```
- Inference: when `s = float(S)` underflows to 0 (deeper than ~1e-38), `Z + z ≈ Z`. Rebase can only trigger when |Z_m| is itself comparable to |z|, i.e. tiny. That case is caught by the "|Z| small → full floatexp iteration" branch. So the hot path is pure float32, with rare floatexp excursions near the reference's own close approaches to 0 (and at every rebase).

### Inferences: zoom depth to number type for a float32-only GPU
| Pixel spacing | Delta type |
|---|---|
| > ~1e-5 to 1e-7 | plain float32 direct iteration (no perturbation) |
| ~1e-7 to ~1e-30 | float32 perturbation, with float32 BLA coefficients |
| deeper than ~1e-30 (safety margin before 1e-38) | rescaled float32 (S as floatexp) or floatexp deltas, with floatexp BLA coefficients and radii. Unbounded depth (1e-300, 1e-10000, …): the only limit is reference precision and orbit memory. |

The handover margin exists because `|z|` and `c` shrink below pixel spacing, and `z^2`, `|z|^2` (in squared-norm tests) underflow at sqrt(1e-38) ≈ 1e-19. Squared-norm comparisons (`|Z+z|^2 < |z|^2`) therefore break much earlier than the deltas themselves. The Desarso 1e-25 handover is consistent with this. For float32, use `max(|re|,|im|)`-style norms, or compare in floatexp, for the rebase test at depth.

### Gaps
- Denormal behaviour on Apple GPUs was not found in a fetched source (needs the Metal Shading Language spec). The safe assumption is that denormals may be flushed to zero, so the effective float32 floor is ~1.18e-38, not 1.4e-45.
- No fetched source documents a Metal-specific deep-zoom renderer.

---

## 8. When float32 deltas give visibly wrong images even with rebasing

### Takeaway
Rebasing removes the classical glitch, but float32's 24-bit mantissa still causes errors. The causes, mostly from KF's warnings and the BLA ε analysis plus inference, are:
- **Range failures:** underflow of `z^2`, `|z|^2`, `c` or BLA coefficients.
- **Accumulated rounding over very long iteration counts** near the boundary, where orbits are chaotic and errors are amplified by `|∏ 2Z|`.
- **Too-loose BLA radii.**
- **Rounding of the stored reference to float32.**

### Cited Findings
- KF disables single precision by default because it can cause "undetected glitches at some locations"; images should be inspected. KF limited single precision to shallow zooms (until zoom e20). — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- KF 2.15.5: the "single reference method does not cure all glitches". — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- BLA radius scales with ε. For float32, ε = 2^-24 must be used, otherwise skipped terms exceed rounding error. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- The float32 WebGPU renderer verifies with a pixel-diff mode (approximated vs unapproximated) and a BigInt oracle self-test. Two of its listed failure modes are "silent black frames" (rebase test bug at a nucleus) and BLA coefficient overflow to Infinity/NaN. — [Desarso/mandelbrot-webgpu](https://github.com/Desarso/mandelbrot-webgpu)
- mathr: "single precision (24-bit) can also work" for perturbed orbits, while noting the 1e-38 underflow for deep needle locations. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)

### Inferences
- **Reference storage:** store `Zref[m]` as float32 (it is bounded by 2, so no range issue). Its relative error of 6e-8 enters the delta as `2·δZ·z`, a relative error on z of order 1e-7 per step. That is fine for smooth colouring but compounds over 1e5–1e7 iterations near minibrots. A float-float (2×float32) reference or delta, like FractalShark's 2×32 type, is the standard mitigation if artefacts appear.
- **Pixel offset `c` quantisation:** in float32, the pixel offset from the reference has 24 bits. If the reference lies far outside the view (e.g. |c| ≈ 1000 pixel spacings), only about 14 bits of sub-pixel resolution remain, causing blockiness or noise in smooth iteration counts and DE. Keep the reference near the view centre, or store c relative to the pixel spacing (rescaled: d ≈ O(1)).
- **Smooth colouring and DE** need `log|Z+z|` and derivative magnitudes. Derivatives easily exceed float32 range (~3e38) at deep zooms, so dual numbers scaled by pixel spacing (cited above) or floatexp derivatives are required.
- **Expected visible symptoms with float32:**
  - noisy or "speckled" iteration bands deep inside spirals or near minibrots with very high iteration counts
  - wrong interior/exterior classification near the boundary
  - BLA seams where skip-length boundaries are visible (radius too generous)
  - whole-frame failure when `|z|^2` underflows in rebase/escape tests
- Mitigation checklist:
  - squared-norm-free or floatexp comparisons
  - ε = 2^-24 in BLA radii, with the extra /2 safety factor
  - rescaled float32 with floatexp full iterations near |Z| ≈ 0
  - an optional float-float mode for verification renders
  - a CPU double/floatexp reference renderer for diffing tiles

### Gaps
- No quantitative study (2023–2026) was found measuring float32 vs float64 perturbation image error as a function of iteration count or depth. This is an open gap. Empirical testing on target locations is required.
- No 2023–2026 academic paper on these topics was found. The state of the art lives in software (Fraktaler 3, Imagina rewrite, FractalShark, KF2+) and forum or blog posts.
