---
title: "Imagina HInfLAEvaluator (source code)"
type: source
authors: ["Zhuoran"]
year: 2024
raw: "web/imagina-hinfla-evaluator.cpp"
status: skimmed
tags: [imagina, linear-approximation, bla, rebasing, period-detection, floatexp, agpl]
---

# Imagina HInfLAEvaluator (source code)

**URL:** <https://raw.githubusercontent.com/5E-324/Imagina/master/HInfLAEvaluator.cpp> (and `HInfLAEvaluator.h`) · **Saved:** `raw/web/imagina-hinfla-evaluator.cpp` + `raw/web/imagina-hinfla-evaluator.h` · **Terms:** AGPL-3.0 code; do not copy into almondbread, read for facts only

## What it is

The main Mandelbrot evaluator of Imagina (pre-rewrite `master`, last pushed 2024-01-23), by Zhuoran, the originator of rebasing and of this "LA" scheme. The header defines the LA record (`LAInfo`), the period-transform record (`ATInfo`) and the evaluator; the `.cpp` builds the reference orbit, the LA stages and the AT, and evaluates pixels in 4-wide SIMD. This page describes the algorithm in our own words and notation. No code is reproduced (AGPL).

Notation below: `‖x‖` is the Chebyshev norm `max(|Re x|, |Im x|)`, used for every threshold test. `Z_k` is the stored reference orbit, `δz` the pixel's delta, `δc` its parameter offset, `z = Z + δz`.

## What it covers

**LA record.** Each record starts at a reference iteration with value `Z_s` and stores: `Ref = Z_s`, coefficients `A` (z-coefficient) and `B` (c-coefficient), a z-threshold `R_z`, a c-threshold `R_c`, a running minimum `MinMag` (used for period detection), plus a step length and the index to resume at in the next lower stage. A fresh record has `A = B = 1`, `R_z = R_c = 1`, `MinMag = 4`.

- Applying a record: `w = δz·(2Z_s + δz)` (the exact non-linear part of the first step, without `δc`). The record is usable if `‖w‖ < R_z`. Then `δz ← A·w + B·δc` and `z = δz + Ref(next record)`. A fresh record is therefore exactly one perturbation step. This differs from textbook BLA (`A·δz + B·δc`): the first squaring is kept exact and only later steps are linear.
- Step rule (extend a record by one iteration whose reference value is `Z_n`): `R_z ← min(R_z, ε·‖Z_n‖/‖A‖)`, `R_c ← min(R_c, ε·‖Z_n‖/‖B‖)`, then `A ← 2Z_n·A`, `B ← 2Z_n·B + 1`, `MinMag ← min(MinMag, ‖Z_n‖)`, with `ε = 2^-24` for both thresholds. Thresholds use the coefficients before the update.
- Composite rule (merge record X with the following record Y, where `Y.Ref = z`): first apply X's own bound at the join, `R_z ← min(R_z^X, ε‖z‖/‖A_X‖)` and likewise `R_c` with `B_X`. Form `A_t = 2z·A_X`, `B_t = 2z·B_X`. Then `R_z ← min(R_z, R_z^Y/‖A_t‖)` and `R_c ← min(R_c, R_z^Y/‖B_t‖)` (Y's z-threshold bounds both). Result: `A = A_t·A_Y`, `B = B_t·A_Y + B_Y`, `MinMag = min(MinMag_X, ‖z‖, MinMag_Y)`. Across the join Y's first squaring is linearised (`w_Y ≈ 2z·δz`) (inference from the formulas).
- Period detection while building (the active "method 2"): a step reports a period when the new `MinMag` drops below `2^-6 ×` the old one (stage 0); a composite reports one when `min(MinMag_X, ‖z‖) < 2^-3 · MinMag_X`. A separate check (`DetectPeriod`, `‖Z‖ < 2^-3 · MinMag`) decides whether a new record should start fresh or with one step. The disabled "method 1" instead compared the shrinking `R_z` against `2^-10` of its previous value.

**Reference orbit.**

- Computed in MPIR arbitrary precision; stored as double if the view half-height is above `2^-896`, else as FloatExp (double mantissa + separate exponent).
- Views wider than `2^-32` (radius), or orbits shorter than 8 iterations, skip perturbation entirely and iterate `z² + c` directly in double.
- The orbit also tracks `dZ/dc` and stops early when `2·radius·‖dZ/dc‖ > ‖Z‖`: a first-order ball test saying a nucleus (`Z = 0`) lies inside the view. The reference is then marked periodic, its length is the period, and a Newton step `−Z/(dZ/dc)` is offered as a better centre. Otherwise it stops at `|Z|² > 5` or the iteration limit. Pixels later wrap around a periodic orbit by rebasing.

**Stage construction** (up to 256 stages).

- Stage 0 is built from orbit points with the Step rule. Walk from iteration 1 until a period is detected; that first record's length is the period `P`. Then cut the rest of the orbit into records, closing a record when a period is detected or when it reaches length `P`.
- If no period appears and the orbit is longer than 64, or `P > 64`, an artificial chunk length is used: `P = round(N^(1/k))` with `k = round(log2(N)/4)` (a "log16" root), so records are roughly 16 iterations long (e.g. `N = 10^6` gives `k = 5`, `P ≈ 16`) (inference from the formula). If the orbit is 64 or shorter with no period, one record covers it and no further stages are built.
- Stage n+1 is built from stage n's records with the Composite rule and the same logic. Step lengths add; the artificial chunk is `len0 × round(ratio^(1/round(log2(ratio)/4)))`, where `len0` is the length of stage n's first record. Building stops when a stage finds no period. Each stage ends with a sentinel record holding the final orbit value, so the last record has a "next Ref".
- The c-threshold is only checked once per stage, against that stage's first record: if `‖δc‖ ≥ R_c`, the whole stage is skipped for that pixel. FractalShark keeps the same rule ("each stage retains its first row's threshold", [FractalShark README](fractalshark-readme.md) §Build instructions).

**Evaluation order per pixel.**

1. If the AT applies (below), iterate it first.
2. For each stage from the highest down (`EvaluateLA`): at record `j`, compute `w`. If `‖w‖ ≥ R_z`, the lane leaves this stage and resumes in the next lower stage at the stored index. Otherwise apply the record, add its step length to the iteration count, and stop at the iteration limit. After a record, if `j` reached the end of the stage, rebase to the start (`δz ← z`, `j = 0`). Else rebase when `‖δz‖ > ‖z‖`.
3. Finally plain perturbation (`EvaluatePT`): `δz ← 2Z_j·δz + δz² + δc`. Rebase when `64|δz|² > |z|²` (that is `|z| < 8|δz|`, a looser trigger than the LA stages' `‖z‖ < ‖δz‖`) or at the end of the orbit. Escape at `|z|² > 4096`.
4. SIMD bookkeeping: four pixels share a pass; lanes sitting at different reference indices are gathered so the lowest index runs first, and rebased lanes wait until the others return to index 0.

**Double vs FloatExp per stage.** In FloatExp mode, a stage is converted to plain double when the view radius is above `2^-896`, or when the next stage's first record has `R_z > 2^-768` and `R_c > 2^-768`. Thresholds only shrink going up, so this marks the lower stages. If stage 0 qualifies, the orbit is also stored in double and PT runs in double. Evaluation starts in FloatExp at the top and switches the pixel state to double at the first double stage, staying in double down through PT.

**AT (period transform).**

- After building, walk the stages from the top. Take each stage's first record `L` (one period from the start, length `P`) and the next record's reference value `Z_N`. Form `a = A_L`, `b = A_L·B_L`, `c₀ = Z_N·a`, escape radius² `= min(|a|²·R_z, 2^256)` and c-threshold `= min(R_c, 2^256/‖b‖)`.
- Usable if `radius²·|b|²·2^32 > |c₀|²` (the view, mapped into the new coordinates, is not tiny next to the offset `c₀`) and escape radius² > 4. The first usable stage wins.
- Use (if `‖δc‖ ≤` the c-threshold): in double, iterate the plain quadratic map `u ← u² + c'` with `c' = b·δc + c₀`, from `u = 0`, for at most `⌈maxIter/P⌉` steps, until `|u|²` exceeds the escape radius². Then map back: `z = u/a`, `δz = z` (rebased), iteration count `= n·P`, and `dz/dc = (du/dc')·b/a`. Evaluation continues with the LA stages.
- Why it works: from a periodic reference with `Z_0 = 0`, one period gives `δz_P ≈ A·δz² + B·δc`. With `u = A·δz` this is `u_P ≈ u² + A·B·δc`. The period map is a rescaled copy of the Mandelbrot map, and a whole period costs one double step (inference; the code gives no derivation).

**Derivatives.**

- PT step: `dz/dc ← 2(Z_j + δz)·dz/dc + s` and `dz/dz ← 2(Z_j + δz)·dz/dz`.
- LA step: with `D = 2(Z_s + δz)·A` at entry, `dz/dz ← D·dz/dz` and `dz/dc ← D·dz/dc + B·s`. This is the exact derivative of the record formula.
- `s` is a scale factor. When state moves from FloatExp to double, derivatives are pre-scaled by `1/(max(‖dz/dc‖, 1)·2^448)` to avoid overflow, and unscaled at the end.
- Output: distance estimate from `2|dz/dc| / (|z|·ln|z|)`. Smooth iteration count `n + log2(log2 4096) − log2(log2 |z|²)`.

**Other features (skimmed).**

- An optional high-quality mode samples a precomputed log-polar mipmap of escape data near small `|z|` instead of iterating to the end.
- Orbit save files store compressed reference orbits: way-points where a double re-iteration drifts from the stored orbit by more than a relative threshold `2^quality`, plus rebase markers. This is the format FractalShark reads.

## Reliability

- Primary: the actual implementation by the inventor of the method. Comments are sparse, constants are unexplained, and the repo is marked "being rewritten".
- Read in full: the header, orbit, stage, AT, LA/PT/direct/AT evaluation and the main `Evaluate`. Skimmed: the multithreaded stage-0 builder (appears to do the same work in parallel), Julia mipmap rendering, orbit save/load.
- ε = `2^-24` is used for both thresholds even when records run in double. That is far larger than the forum values for double-precision BLA (`2e-53`, see [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](philthompson-bla.md)). The two ε's sit in different inequalities (Chebyshev, entry `w`), so they are not directly comparable (inference).
- The c-threshold of a composite is bounded by Y's z-threshold divided by `‖B_t‖`, a per-term split of `‖A_t w + B_t δc‖ < R_z^Y` with no factor for the sum. This is slightly optimistic (inference).
- A header constant `Order = 4` is never used in the code read.

## Useful sections

Code files have no headings; § gives the function names.

- §LAInfo::Step, §LAInfo::Composite (header) — record update rules, thresholds, period detection.
- §LAInfo::Prepare / Evaluate / EvaluateDerivatives (header) — applying a record and its derivatives.
- §LAInfo::CreateAT, §ATInfo::Usable (header) — the period transform.
- §CalculateOrbit — reference orbit, ball-test early stop, Newton centre prediction.
- §CreateLAFromOrbit, §CreateNewLAStage — stage construction, artificial chunk length.
- §GenerateApproximationData, §ConvertStageToDouble — double vs FloatExp per stage.
- §EvaluateLA, §EvaluatePT, §EvaluateAT — per-pixel loops and rebase conditions.
- §HInfLAEvaluationTask::Evaluate — overall order (direct / AT / stages / PT), DE and smooth colouring output.
- §SaveCompressedOrbit — orbit compression.

## Used by

- [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)
- [Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md)
- [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)
- [Distance estimation](../techniques/distance-estimation.md)
- [Imagina](../implementations/imagina.md)
- [How much faster is Imagina-style LA than binary-tree BLA on a GPU?](../questions/bla-vs-la-benchmark.md)
- [How does Imagina/FractalShark reference-orbit compression work?](../questions/reference-orbit-compression.md)
- [Is a higher-order (second-order) BLA worth it?](../questions/higher-order-bla.md)

# Citations

- <https://raw.githubusercontent.com/5E-324/Imagina/master/HInfLAEvaluator.cpp>
- [FractalShark README](fractalshark-readme.md)
- [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](philthompson-bla.md)
