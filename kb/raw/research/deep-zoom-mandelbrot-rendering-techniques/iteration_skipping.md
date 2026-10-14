# Iteration-skipping acceleration for perturbation deep zoom (state of the art, Oct 2026)

Notation used throughout: reference orbit Z_n (high precision, computed once on CPU), pixel delta z_n (low precision), pixel offset c (= dc) from the reference C. Perturbed Mandelbrot step: z_{n+1} = 2 Z_n z_n + z_n^2 + c = (2 Z_n + z_n) z_n + c.

Source note: the main primary sources for formulas are Claude Heiland-Allen's (mathr) "deep zoom theory and practice" blog posts (2021-05-14, 2022-02-21), his deep-zoom web page, and the Imagina source code (5E-324/Imagina, `HInfLAEvaluator.h/.cpp`, last pushed 2024-01-23), which I read directly. I could not fetch the fractalforums.org threads (the site returned a redirect loop), so Zhuoran's original posts are cited only through secondary sources.

---

## 1. Series approximation (SA): formulas, skip criteria, failure modes. SUPERSEDED for the Mandelbrot set

### Takeaway
SA expands z_n as a truncated polynomial in c, z_n = sum_k A_{n,k} c^k. One set of coefficients is computed alongside the reference, and every pixel starts at the same skip iteration. That skip iteration is picked heuristically using "probe points". SA is fragile because it has no rigorous error bound, uses a single global skip count for the whole image, and has trouble with folding formulas. For the Mandelbrot set, BLA/LA has superseded it in all actively developed renderers. One measured case still exists where SA beat BLA (see below).

### Cited Findings
- Formula: substituting z_n = Σ_k A_{n,k} c^k (no constant term) into the perturbed step gives Σ A_{n+1,k} c^k = 2Z Σ A_{n,k} c^k + (Σ A_{n,k} c^k)^2 + c. Equating coefficients of c^k gives the recurrences. The series lets you skip many low-precision iterations "provided truncation and low precision don't cause too much trouble." — [mathr, deep zoom theory and practice (2021-05-14)](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Skip criterion ("probe points"): the traditional method compares the series against regular or perturbed iterations at a set of probe points. When the series starts to deviate, it rolls back one iteration and initialises every pixel from the series at that iteration. No standard deviation threshold is given. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- No rigorous correctness: "no complete mathematical proof of series-skipping correctness with rigorous error bounds", though images "do most often look plausible". — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Abs/folding formulas (Burning Ship): SA becomes two bivariate real series in Re(c), Im(c). These hold only while the region is not folded by abs(), so "typically only a few iterations can be skipped." — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- Munafo classifies SA as the general term for extrapolation-based perturbation methods, with "cubic deltas" and BLA as examples. He describes BLA as "effectively a 'series' approximation of degree 1 (linear) in two variables." — [Mu-Ency: Series approximation](https://mrob.com/pub/muency/seriesapproximation.html), [Mu-Ency: BLA](https://mrob.com/pub/muency/bivariatelinearapproximati.html)
- The 2022 mathr article presents BLA as "an alternative to series approximation". It claims BLA is conceptually simpler, easier to implement and parallelise, has better-understood stopping conditions, and generalises to Burning Ship and hybrids. — [mathr, deep zoom theory and practice again (2022-02-21)](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Measured SA vs BLA (Very Plotter, JavaScript, 7 workers, M1). Times are seconds for BLA vs perturbation+SA:
  - 2.9e89, 350k iterations: BLA 99 s vs SA 1009 s (BLA 10.2x faster).
  - 4.99e152, 2M iterations: BLA 358 s vs SA 12,958 s (36.2x).
  - 1.8e157, 50.1k iterations: 1.5–2.4x faster with BLA.
  - 5.62e227, 1.2M iterations: 1.7x faster with BLA.
  - 9.5e693, 350k iterations, floatexp: BLA 3438 s vs SA 2076 s, so SA was faster (0.6x). This held at every resolution tested (SA/BLA ratio 0.73–0.85, from 0.23 to 3.0 MP).
  - Iterations skipped per pixel at 5.62e227: BLA about 989k, SA about 1,064k.
  - Source: [Phil Thompson, Faster Mandelbrot Set Rendering with BLA (2023)](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- In that test, ε for BLA had to be lowered to about 1e-122 at the 1e693 location (forum values were 2e-24 for float and 2e-53 for double). — [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)

### Inferences
- Explicit low-order recurrences, derived by me by equating coefficients in the cited formula (not quoted from a source). With z = A c + B c^2 + C c^3 + ...:
  - A_{n+1} = 2 Z_n A_n + 1
  - B_{n+1} = 2 Z_n B_n + A_n^2
  - C_{n+1} = 2 Z_n C_n + 2 A_n B_n
  - In general, coefficient k gets 2 Z_n A_{n,k} + Σ_{i+j=k} A_{n,i} A_{n,j}.
  - Typical term counts in KF-era renderers were configurable, roughly 3 to 60+. I have no fetched source for the exact defaults.
- Why SA is fragile:
  - (a) One skip count serves the whole image, so the worst pixel limits everyone.
  - (b) Probe points can miss the region where truncation error blows up, which produces silently wrong images rather than detected glitches.
  - (c) Coefficients grow, so they need extended range at depth.
  - (d) It cannot skip past the reference's near-zero returns (the periodic points), which BLA/LA handle by restarting via rebasing.
- Phil Thompson's 1e693 result is one JavaScript implementation with a naïve binary-tree BLA and a very small ε. Treat it as weak evidence that a well-tuned SA can still compete at some floatexp locations, not as a general result. Imagina and Fraktaler 3 do not ship SA for the Mandelbrot set at all (Imagina's evaluator contains only LA/AT).
- Recommendation for a new Metal renderer: do not implement SA.

### Gaps
- I found no rigorous error-bound criterion for SA. KF's exact probe-point and term-count heuristics were not retrieved (the fractalforums threads were unreachable).
- No benchmarks were found comparing SA against Imagina-style LA (as opposed to naïve BLA).

---

## 2. Bilinear / bivariate linear approximation (BLA): formulas, radius, merging, table, lookup, rebasing

### Takeaway
BLA (Zhuoran, Dec 2021, fractalforums "Another solution to perturbation glitches") replaces l perturbation steps with z_{n+l} = A_{n,l} z_n + B_{n,l} c. This is valid while |z_n| < R_{n,l}. Single-step coefficients are A = 2Z_n, B = 1. Steps compose as A = A_y A_x, B = A_y B_x + B_y, R = min(R_x, max(0, (R_y − |B_x||c|)/|A_x|)).
- Table: built bottom-up as a binary tree, O(M) entries (about 2M) for an M-iteration reference.
- Per-pixel lookup: at reference iteration m, take the largest valid skip starting at m; otherwise do one perturbation step; check rebasing after each step.
- Rebasing (|Z_m + z| < |z| → z := Z_m + z, m := 0) is integral: it lets one reference serve the whole image with no glitch detection.

### Cited Findings
- Model: l iterations from n are approximated by z_{n+l} = A_{n,l} z_n + B_{n,l} c, valid when the omitted non-linear part is smaller than the low-precision rounding error. Single-step validity is |z_n| << |A_{n,1}|. — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- Single step: A_1 = 2Z, B_1 = 1. Recurrences: A_{l+1} = 2 Z_n A_l and B_{l+1} = 2 Z_n B_l + 1 (A_0 = 1, B_0 = 0). The omitted term is Δz². — [Mu-Ency BLA](https://mrob.com/pub/muency/bivariatelinearapproximati.html), [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- Single-step radius as given by mathr in 2022: r = max(0, ε·(|Z| − max_image|c|)/(|J_f(Z)| + 1)). Here ε is the hardware precision (e.g. 2^-24 for float) and J_f(Z) = 2Z. mathr divides |Z| by 2 for safety. — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Variant used in Very Plotter: r = (ε|Z| − |B||c|)/|A|. — [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- mathr's deep-zoom page says an earlier validity-check attempt "was nonsense" and that readers should ignore that part of older posts. The radius formulas are heuristic: "a bit handwavy … seems to work ok in practice". — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html), [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Merging x then y: A_z = A_y A_x, B_z = A_y B_x + B_y, R_z = max{0, min{R_x, (R_y − |B_x||c|)/|A_x|}}, l_z = l_x + l_y. — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html), [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Table construction:
  - Make M single-step BLAs, then merge neighbours without overlap, giving ⌈M/2⌉, ⌈M/4⌉, … until one BLA skips M−1. The total is O(M), about 2M entries.
  - Start at iteration 1, because iteration 0 (Z = 0, the critical point) has radius 0 and always needs a non-linear step.
  - Each level can be built in parallel (mathr suggests OpenCL/GPU).
  - mathr notes this construction is "suboptimal according to Zhuoran."
  - Sources: [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html), [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Lookup: at reference iteration m, find the BLA starting at m with the largest skip l such that |z| < R. If there is none, do one perturbation step. Check rebasing after each BLA or perturbation step. — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- Very Plotter lookup heuristic and table pruning:
  - Test the lowest level first. If it is invalid, stop. Otherwise test from the highest level down.
  - "Merge and cull" drops levels 1–2 because they save little over plain perturbation, leaving about 500 stored BLAs for a 1000-iteration orbit.
  - ε is chosen automatically: binary search on the ε exponent, rejecting any ε for which a test point's iteration count differs from plain perturbation by more than 0.001%.
  - Source: [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- Rebasing:
  - When |Z_m + z_n| < |z_n|, set z_n := Z_m + z_n and reset m to 0.
  - With multiple critical points, choose the orbit o minimising |(Z − Z_o) + z|. Only one reference per critical point is needed (one for Mandelbrot and Burning Ship).
  - "Glitches are avoided rather than detected."
  - Hybrids need one reference (and one BLA table) per loop phase.
  - Sources: [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html), [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- History: Zhuoran introduced the rebasing tests on Fractal Forums in December 2021. Newer versions of Kalles Fraktaler and contemporaneous deep-zoom programs use BLA. BLA is most useful at dwell limits in the millions. — [Mu-Ency BLA](https://mrob.com/pub/muency/bivariatelinearapproximati.html)
- Fraktaler 3:
  - Uses BLA, with a "BLA step limit" (default 1024; "a few thousand" is usually enough; too low gives glitchy images).
  - v3 added "BLA skip levels" to reduce memory.
  - Reusing BLA across a zoom-out sequence is "not generally applicable."
  - Latest version: 3.1 (2025-12-15).
  - Source: [Fraktaler 3 site](https://fraktaler.mathr.co.uk/)

### Inferences
- Per-pixel pseudocode (Fraktaler-style BLA, my synthesis of the cited rules):
  ```
  m = 0; n = 0; z = 0
  while n < maxIter:
      bla = lookup(m, |z|)        // largest l with |z| < R[m,l] (and m+l <= M)
      if bla: z = bla.A*z + bla.B*c; m += bla.l; n += bla.l
      else:   z = (2*Z[m] + z)*z + c; m += 1; n += 1
      Zz = Z[m] + z
      if |Zz|^2 > escape: break
      if |Zz| < |z| or m == M: z = Zz; m = 0     // rebase
  ```
- Because R depends on |c| through the merge formula, the table is built for the image's max|c|. R is then image-specific: it must be rebuilt when zoom or framing changes significantly (consistent with Fraktaler's note that BLA reuse is not generally applicable).
- Level skipping (dropping fine levels) trades a few extra perturbation steps for about 2x less memory. On a GPU, dropping levels 1–2 (as Very Plotter does) is reasonable, because a perturbation step and a BLA step cost about the same.

### Gaps
- Zhuoran's original forum posts (exact radius and "optimal" merging) could not be fetched because of the fractalforums.org redirect loop.
- I found no published proof of the radius bound. All sources describe it as heuristic.

---

## 3. Imagina's LA (Zhuoran): staged, period-aligned linear approximation plus the "AT" renormalization. CURRENT STATE OF THE ART

### Takeaway
Imagina's evaluator (`HInfLAEvaluator`) is not a power-of-two BLA tree. It is a hierarchy of up to 256 stages whose step boundaries follow the reference's near-returns to 0 (its period structure), with about 16x coarsening per stage when no period is found. Each LA step does the first iteration exactly (non-linearly) and the rest linearly:
- dz' = ZCoeff · (dz·(2Z_n + dz)) + CCoeff · dc
- validity: ‖dz(2Z_n + dz)‖_∞ < LAThreshold and ‖dc‖_∞ < LAThresholdC

For a periodic reference, after the top stage an "AT" maps the pixel into renormalised coordinates around the minibrot and iterates plain z ← z² + c in double/float. This is effectively a robust nanoMB2. FractalZoomer (via Claude and Zhuoran) and FractalShark descend from this design.

### Cited Findings (all from Imagina source unless noted: [HInfLAEvaluator.h](https://github.com/5E-324/Imagina/blob/master/HInfLAEvaluator.h), [HInfLAEvaluator.cpp](https://github.com/5E-324/Imagina/blob/master/HInfLAEvaluator.cpp))
- LA record `LAInfo {complex Ref, ZCoeff, CCoeff; Real LAThreshold, LAThresholdC;}`. There is also an index record `LAInfoI {StepLength, NextStageLAIndex}` and a per-stage descriptor `{LAIndex, MacroItCount, UseDoublePrecision}`. `MaxLAStages = 256`.
- Constants: `LAThresholdScale = LAThresholdCScale = 2^-24`. Period-detection thresholds are 2^-10 (method 1) and 2^-6 / 2^-3 (method 2, on min|z|).
- Single-step extension (`Step`): with z = the reference value at that step,
  - LAThreshold' = min(LAThreshold, ‖z‖_∞/‖ZCoeff‖_∞ · 2^-24)
  - LAThresholdC' = min(LAThresholdC, ‖z‖_∞/‖CCoeff‖_∞ · 2^-24)
  - ZCoeff' = 2z·ZCoeff
  - CCoeff' = 2z·CCoeff + 1
  - Chebyshev norm (max(|re|,|im|)) is used instead of |·|.
- Composition (`Composite`, this then LA):
  - The thresholds are first tightened as above.
  - ZCoeff = 2z·ZCoeff; CCoeff = 2z·CCoeff.
  - LAThreshold = min(LAThreshold, LA.LAThreshold/‖ZCoeff‖); LAThresholdC = min(LAThresholdC, LA.LAThreshold/‖CCoeff‖).
  - ZCoeff *= LA.ZCoeff; CCoeff = CCoeff·LA.ZCoeff + LA.CCoeff.
  - It returns "period detected" when the new threshold collapses below a fraction (2^-10) of the old one, which signals a near-return of the reference toward 0.
- Evaluation of one LA step (`Prepare`/`Evaluate`):
  - newdz = dz·(2·Ref + dz)
  - unusable if ‖newdz‖_∞ ≥ LAThreshold
  - else dz = newdz·ZCoeff + dc·CCoeff, and z = dz + Ref_{next}
  - Derivative: dzdc ← dzdc·2(dz+Ref)·ZCoeff + CCoeff·scale. The same applies for dzdz. So distance-estimation derivatives come through LA steps essentially for free.
- Stage construction (`CreateLAFromOrbit`, `CreateNewLAStage`):
  - Stage 0 composes single steps and cuts a macro-step at each detected period.
  - Each higher stage composes the previous stage's macro-steps the same way.
  - If no period is detected, or the period exceeds 64x the previous step length, a synthetic period is chosen: Ratio^(1/round(log16 Ratio)) × the previous step length, i.e. stages coarsen about 16x.
  - Stages are built until no further period is found.
- Per-pixel lookup (`EvaluateLA`):
  - Start at the coarsest stage (`CurrentLAStage = LAStageCount`).
  - If ‖dc‖ ≥ the stage's LAThresholdC, drop a stage.
  - Otherwise walk that stage's macro-steps sequentially. When a step is unusable, record `NextStageLAIndex` as the position in the finer stage and descend.
  - Rebase within a stage when ‖dz‖ > ‖z‖ (`VecResetIf`) by resetting j to 0. Reaching the end of the stage (j == MacroItCount) also resets.
  - Lookup is O(1) per step: no search over levels, just "try next macro-step at the current stage; if invalid, go finer".
  - Pixels are processed 4-wide SIMD (`vec4`), with masks.
- AT (`CreateAT`, `EvaluateAT`):
  - From the top-stage LA (covering one whole period), compute: ZCoeff_AT = ZCoeff; CCoeff_AT = ZCoeff·CCoeff; RefC = Ref_next·ZCoeff; SqrEscapeRadius = min(|ZCoeff|²·LAThreshold, 2^256); ThresholdC = min(LAThresholdC, 2^256/‖CCoeff_AT‖).
  - `Usable` requires SqrRadius·|CCoeff|²·2^32 > |RefC|² and SqrEscapeRadius > 4.
  - Evaluation then runs plain `z = z*z + c` with dzdc ← 2z·dzdc + scale in ordinary precision. Each AT iteration counts as StepLength (= one period) reference iterations until |z|² exceeds SqrEscapeRadius. LA/perturbation then continues.
  - AT is used when the reference is periodic or maxIter < reference length.
  - `CreateATFromLA` tries stages from coarsest to finest and picks the first usable one.
- Precision handling:
  - Coefficients are stored as double when range permits (radius > 2^-896, or thresholds > 2^-768), otherwise as FloatExp (mantissa plus separate exponent; `FloatExp.h`, `FloatExpVector4.h`).
  - Stages are converted to double independently (`ConvertStageToDouble`), so coarse stages with huge/tiny coefficients stay FloatExp while fine stages run in double.
- Imagina repo README: "One of the fastest Mandelbrot set renderer & explorer"; "The project is being rewritten". The last push to the public repo was 2024-01-23. — [Imagina GitHub](https://github.com/5E-324/Imagina)
- FractalShark says its LA is ported from FractalZoomer and "originated with Claude and Zhuoran". It expects Imagina to beat FractalShark on performance in the long term. — [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)

### Inferences
- Why LA beats naïve BLA:
  - (1) Doing the first step non-linearly and testing the threshold on dz(2Z+dz) rather than on dz removes the radius-0 problem at near-zero reference values, so steps can span periods.
  - (2) Period-aligned boundaries mean macro-steps line up with where rebasing happens. A pixel can repeatedly apply "skip one whole period" steps and rebase cheaply, instead of falling back to fine levels at every period boundary as an unaligned power-of-2 tree does.
  - (3) Sequential walking within a stage is O(1) per step and branch-light, so it suits SIMD/GPU.
  - (4) The thresholds are relative (2^-24 scale) and the |c| dependency is held in LAThresholdC, so the table is less tied to one image frame than mathr-style R that bakes in max|c|. This last point is my reading of the code.
- AT is the practical, robust descendant of knighty's nanoMB idea:
  - Near a minibrot of period P, P delta-iterations act like one quadratic iteration in rescaled coordinates.
  - Once a pixel's dz is small enough, the renderer iterates an ordinary low-precision Mandelbrot in the rescaled frame. This skips millions of iterations at the cost of one cheap float iteration per period.
  - This is the biggest single win for deep minibrot zooms with iteration counts of 1e6–1e9.
  - Inferred from the code. "AT" is often expanded as "approximation terminator" in forum usage, but I could not confirm that from a fetched source.
- The table size is O(M) (stage 0 has about M/period-ish entries; total entries is less than 2·M). Each record is 3 complex + 2 reals: 32 B in float32, 64 B in double, 52 B in FractalShark's HDRx32 layout.

### Gaps
- No public benchmarks from Zhuoran comparing Imagina LA vs BLA vs SA were retrievable. The forum is unreachable, and the Imagina README has no numbers.
- The "being rewritten" Imagina (private?) may contain newer methods. Nothing public after Jan 2024 was found.

---

## 4. nanoMB1 / nanoMB2 (knighty) and SuperMB. SUPERSEDED (absorbed into LA + AT)

### Takeaway
nanoMB1 extends SA to a bivariate series in (z, c) around a periodic reference (minibrot nucleus), so it skips whole periods per evaluation. nanoMB2 chains minibrots at successive depths and rebases outward. Both live in Kalles Fraktaler. nanoMB2 is "highly experimental and fails for many locations." Their core idea, skipping one period per step near a nucleus and then renormalising, is what Imagina's LA stages plus AT do robustly.

### Cited Findings
- NanoMB1:
  - knighty extended the series to two complex variables. If the reference C is periodic, such as a minibrot centre, a biseries in z and c skips one whole period, and repeated application skips multiple periods.
  - An escape radius for z, derived from properties of the reference, limits how many biseries iterations run. Regular perturbation then continues to final escape.
  - It gives "a further big speedup beyond regular series approximation near minibrots" and is available in KF.
  - Source: [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- NanoMB2:
  - Uses a chain of minibrots at successively deeper levels. Start at the deepest, run biseries iterations until escape from its radius, rebase to the next outer minibrot, and repeat.
  - It is in KF but "highly experimental and fails for many locations." Iterates can be too close together when escaping a deep minibrot, causing catastrophic absorption.
  - Source: [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- No measured speedup numbers were given for nanoMB. — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)

### Inferences
- nanoMB is useful only when the view is centred near a minibrot whose period is known. LA builds the period structure automatically from the reference, so nanoMB adds nothing to an LA-based renderer.
- Do not implement nanoMB. Implement LA plus AT instead.

### Gaps
- SuperMB: I found no reliable source describing it, so it is not covered here. The searches returned only unrelated pages.
- No quantitative nanoMB benchmarks were found.

---

## 5. Fraktaler 3 and FractalShark (LA / LAv2 on GPU): choices and numbers

### Takeaway
Fraktaler 3 (mathr; v3.1, Dec 2025) uses mathr-style BLA plus rebasing on CPU and OpenCL. It selects among number types automatically (float, floatexp, double, doubleexp, long double, float128) using benchmarked "wisdom". FractalShark (Matt Reiman) has two CUDA LA implementations ported from FractalZoomer (the Imagina/Zhuoran lineage). It uses a 2×float32 + shared-exponent type, compressed reference orbits, and GPU NTT-based reference orbits. Its headline numbers are for reference-orbit computation, not for LA per-pixel speedups.

### Cited Findings
- Fraktaler 3:
  - BLA step limit (default 1024) and "BLA skip levels" (v3, reduces memory).
  - Automatic number-type choice via wisdom. The doubleexp and float128 types arrived in v3.1.
  - Can run OpenCL on CPU and GPU at the same time. Tile size defaults to 128×128; larger tiles are faster but risk OS timeouts.
  - Hybrids cost about N× the reference and memory for N formula lines.
  - Locking max reference iterations to a known period "can give a good speedup for high iteration areas."
  - Source: [Fraktaler 3](https://fraktaler.mathr.co.uk/)
- Fraktaler 3 BLA for ABS formulas: for Burning Ship, the single-step radius is additionally capped by |X| and |Y| (both halved for safety). — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- FractalShark LA:
  - "Two CUDA implementations of linear approximation", ported from FractalZoomer and originating with Claude and Zhuoran.
  - The `HDRx32 GPU - LAv2` kernel is selected manually in the GUI.
  - LA row layout with HDR-float and 32-bit indices: 52 B per row, 16 B per stage descriptor.
  - The author is not aware of another public CUDA LA implementation.
  - Source: [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)
- FractalShark precision: "2×32 + exponent" (two float32 plus a shared exponent, about 48-bit mantissa) gives "substantially higher performance on consumer GPUs with only a modest loss of precision." On CPU, native double is preferable. — [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)
- FractalShark reference orbit and memory:
  - GPU reference orbit via fused NTT kernel: about 10× faster than multithreaded MPIR on an RTX 4090 at 16384 limbs (~158k digits). Estimated crossover where GPU beats CPU: about 1e10000 depth.
  - Orbit compression (from Imagina) with per-pixel decompression saved "multiple gigabytes" at period 6e8.
  - Source: [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)
- FractalShark gives no per-pixel LA speedup figures in its README. — [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)

### Inferences
- For Apple Silicon (no fast fp64 on the GPU), FractalShark's design is the closest template: an Imagina-style LA table on the GPU, with coefficients in float32 plus a separate exponent. Per-pixel state (dz, dzdc) in float32 with periodic exponent renormalisation is the same idea as FloatExp.
- Fraktaler's per-device wisdom approach (benchmark each number type and pick the fastest that meets precision and range) is a good fit for Metal. Plain float32 is enough while |c| > about 1e-30 (float32 normal range bottoms out near 1.2e-38, and ε·|c| must stay representable). Beyond that, use float+int-exponent.

### Gaps
- No measured LA vs BLA per-pixel GPU timings from FractalShark or Fraktaler were found.
- What "LAv2" changes relative to v1 is not documented on the README. It would need the FractalShark PDF (§12) or the source.

---

## 6. GPU storage of BLA/LA tables and how much per-pixel cost they remove

### Takeaway
LA/BLA tables are O(M) arrays of small structs, typically structure-of-arrays or packed records.
- Coefficients (Ref, A/ZCoeff, B/CCoeff) are stored in the lowest precision that has the needed range: float32 when shallow; float32 + int32 exponent (FloatExp/HDRx32) when deep. Thresholds are in the same format.
- The reference orbit itself is stored in low precision (float or FloatExp) for the per-pixel loop.
- Measured gains for BLA over SA at 1e89–1e227 and 3.5e5–2e6 iterations range from 1.5× to 36×. The biggest gains come at high iteration counts.
- The skip ratio can approach the full iteration count per pixel: about 989k of about 1.2M iterations skipped at 5.6e227.
- Quantified per-pixel numbers for Imagina LA at 1e1000 with 1e7 iterations were not found.

### Cited Findings
- Imagina stores per-stage precision flags. Stages run in double when coefficients and thresholds stay above about 2^-768 to 2^-896, else in FloatExp. The reference orbit is also downconverted to double when usable (`DoublePrecisionPT`). — [Imagina HInfLAEvaluator.cpp](https://github.com/5E-324/Imagina/blob/master/HInfLAEvaluator.cpp)
- FractalShark LA rows are 52 B (HDR-float, 32-bit indices) plus 16 B stage descriptors, on CUDA, with a 2×float32 + exponent type. — [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)
- Very Plotter uses float BLA at shallower depth and floatexp at 1e693+ (ε ≈ 1e-114 to 1e-122). — [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- Measured skips: about 989k BLA-skipped iterations per pixel at 5.62e227 with 1.2M max iterations. BLA vs SA speedup: 10.2× at 2.9e89 (350k iterations) and 36.2× at 4.99e152 (2M iterations). — [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)
- BLA "mainly makes reference orbit and period-finding computation faster" (Very Plotter v0.10.0 release notes; this is about its pipeline). — [Very Plotter releases](https://github.com/philthompson/visualize-primes/releases)
- Fraktaler 3 warns that excessive perturbation or BLA limits can cause GPU timeouts. — [Fraktaler 3](https://fraktaler.mathr.co.uk/)

### Inferences (design guidance for Metal, float32 only)
- Suggested record layout (SoA, one buffer per stage, coalesced sequential access because pixels walk a stage sequentially). 48 B per entry:
  ```
  struct LAEntry {                     // float32 + shared exponent per complex
      float2 ref;  int refExp;
      float2 zc;   int zcExp;
      float2 cc;   int ccExp;
      float  thr;  int thrExp;         // LAThreshold (‖·‖∞)
      float  thrC; int thrCExp;        // LAThresholdC
  };
  struct LAIndex { uint stepLength; uint nextStageIndex; };
  ```
  A simpler "float32 plus one exponent per record" variant does not work at depth: ZCoeff/CCoeff grow exponentially over long steps while Ref stays O(1), so each quantity needs its own exponent.
- At depth below about 1e-30 (float32 range limit), per-pixel dz and dc need float+exponent arithmetic, or a single per-image scale factor while |dz| stays in range. KF-style "scaled" iteration renormalises when the exponent drifts. LA application is only about 3 complex mul/add per step, so FloatExp overhead is paid per LA step rather than per iteration. That is a big win when the average skip is about 1e3–1e6.
- Expected cost structure at 1e5–1e7 iterations: with LA, per-pixel work becomes roughly (number of LA macro-steps + a few hundred perturbation steps near escape and near rebases). In minibrot-centred views, AT reduces the bulk to one cheap float iteration per period. Reference-orbit computation (CPU MPFR/GMP) and table build then dominate wall time at 1e1000+. This is consistent with FractalShark's focus on GPU reference orbits.
- Memory: at 1e7 iterations, 48 B × about 2 × 1e7 ≈ 1 GB worst case. Period-aligned stages and level skipping (Fraktaler v3) shrink this a lot. Unified memory on Apple Silicon avoids host-to-device copies.

### Gaps
- No source gives a measured fraction of per-pixel time removed at exactly 1e-100 or 1e-1000 with 1e5–1e7 iterations for Imagina-style LA on a GPU. The Very Plotter numbers are CPU JavaScript.
- No Metal-specific implementation write-up was found.

---

## 7. Newer (2023–2026) variants: non-conformal / ABS BLA, hybrids, derivatives, higher order

### Takeaway
Since 2022 the confirmed extensions are:
- 2×2 real-matrix BLA for non-conformal formulas (Mandelbar/Tricorn), with singular-value-based radii.
- ABS-variation BLA for Burning Ship (radius capped by |X|, |Y|).
- Per-phase references and tables for hybrids.
- Derivative propagation through LA steps for distance estimation (Imagina code).
- Engineering advances: orbit compression, GPU reference orbits (FractalShark), doubleexp/float128 types (Fraktaler 3.1).

I found no published, adopted higher-order ("quadratic BLA") method. Multiple-critical-point rebasing is described but unimplemented in Fraktaler 3.

### Cited Findings
- Non-conformal BLA:
  - A and B become 2×2 real matrices, with derivatives computed via dual numbers with two dual parts.
  - Single-step radius: R = ε·inf|A| − sup|B|·|c|/inf|A|, where sup and inf are the largest and smallest singular values.
  - Merge: R_z = max{0, min{R_x, (R_y − sup|B_x||c|)/sup|A_x|}}.
  - Source: [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- ABS variation (Burning Ship): R = max{0, min{ε·inf|A| − sup|B|·|c|/inf|A|, |X|, |Y|}}. Fraktaler 3 halves |X| and |Y| for safety, and the merge formula is unchanged. — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html). The operator-norm C++ implementation (via trace and determinant of AᵀA) is in [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html).
- Hybrid BLA: one reference and one table per loop phase. Multiple critical points: one reference per critical point; rebase to the nearest orbit. "None implemented in Fraktaler 3." — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- Distance estimation: track dz/dk via dual numbers pre-scaled by pixel spacing. Non-analytic formulas use a Jacobian. mathr's page does not describe how BLA coefficients propagate derivatives. — [mathr deep zoom page](https://mathr.co.uk/web/deep-zoom.html)
- Imagina does propagate them through LA steps: dzdc ← dzdc·2(dz+Ref)·ZCoeff + CCoeff·scale, and in AT, dzdc ← 2z·dzdc + scale. Derivatives are held in a scaled form, rescaled by 2^448 to avoid overflow. — [Imagina HInfLAEvaluator.h](https://github.com/5E-324/Imagina/blob/master/HInfLAEvaluator.h)
- Fraktaler 3.1 (2025-12-15) added doubleexp and float128 types. — [Fraktaler 3](https://fraktaler.mathr.co.uk/)
- FractalShark (2024–2026): GPU NTT reference orbits, per-pixel compressed-orbit decompression, GPU Newton-Raphson feature finder; a minibrot at about 1e650452 was found this way. — [FractalShark GitHub](https://github.com/mattsaccount364/FractalShark)
- Very Plotter BLA does not support stripe-average colouring. — [Phil Thompson 2023](https://philthompson.me/2023/Faster-Mandelbrot-Set-Rendering-with-BLA-Bivariate-Linear-Approximation.html)

### Inferences: ranking as of 2026 (best first)
1. **Imagina-style LA (period-aligned stages, non-linear first step, Chebyshev thresholds) plus AT, with rebasing.** This is the state of the art for the Mandelbrot set. It is used by Imagina, FractalZoomer and FractalShark (LA/LAv2). It is the fastest per pixel and handles minibrot periodicity natively. Derivatives come almost free.
2. **mathr-style binary-tree BLA plus rebasing** (Fraktaler 3, KF 2.15+, Very Plotter). It is simpler, generalises to Burning Ship, Mandelbar and hybrids (matrix and ABS variants), but is less efficient across period boundaries. It is the right choice if non-Mandelbrot formulas matter.
3. **SA with probe points (KF classic).** Superseded: fragile, has no error bound, uses a global skip, and breaks on folding formulas. There is one measured counter-example where SA won at 1e693 against a naïve BLA.
4. **nanoMB1/nanoMB2.** Superseded and experimental; their idea survives as LA plus AT.

Higher-order (quadratic in z) per-step approximations would enlarge validity radii but cost more per step. I found no evidence that any renderer adopted one.

### Gaps
- I found no sources for "quadratic/higher-order BLA", hierarchical approximations beyond Imagina's stages, or 2023–2026 forum innovations, because fractalforums.org was unreachable. These may exist in forum threads.
- No source confirms BLA/LA support in Kalles Fraktaler 2+ by version number or date beyond Munafo's general statement.
- I found nothing reliable on SuperMB.
