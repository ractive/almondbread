---
title: "Linear approximation with period-aligned stages and AT (Imagina)"
type: technique
status: draft
sources: ["[[sources/imagina-hinfla-evaluator]]", "[[sources/imagina-readme]]", "[[sources/fractalshark-readme]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/mathr-deep-zoom-theory]]"]
tags: [la, bla, iteration-skipping, imagina, minibrot, state-of-the-art]
---

# Linear approximation with period-aligned stages and AT (Imagina)

Zhuoran's evaluator in Imagina (`HInfLAEvaluator`) is the fastest known
skip method for the Mandelbrot set. It differs from the binary-tree
[Bivariate linear approximation (BLA)](bla.md) in four ways: the first iteration of each step is done
non-linearly, step boundaries follow the reference's returns towards 0 (its
period structure), lookup is a sequential walk with no search, and a final
"AT" step iterates an ordinary low-precision Mandelbrot in rescaled
coordinates near a minibrot. Fractal Zoomer and FractalShark's CUDA
"LA/LAv2" descend from it (src: [FractalShark README](../sources/fractalshark-readme.md)).

Imagina is AGPL-3.0. Everything below is a description in our own words
of what the code does (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)), not code.
The repository's last push was 2024-01-23 and the README says the project
"is being rewritten" (src: [Imagina README](../sources/imagina-readme.md)).

## LA step

A step record holds `Ref` (reference value at its start), `ZCoeff`,
`CCoeff` (complex), and two real thresholds `LAThreshold`, `LAThresholdC`.
Applying it to a pixel delta `dz` with offset `dc`:

```text
t = dz * (2*Ref + dz)                 // the first iteration, exactly (non-linear part)
if ‖t‖∞ >= LAThreshold:  step unusable
dz = t * ZCoeff + dc * CCoeff         // the remaining iterations, linearly
z  = Ref_next + dz                    // for bailout and rebase tests
```

`‖·‖∞` is the Chebyshev norm `max(|re|, |im|)`. A stage is usable only while
`‖dc‖∞ < LAThresholdC` (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §EvaluateLA).

Why the non-linear first step matters (inference): at the reference's near
returns to 0 the binary-tree radius `ε|Z|` collapses to 0, so a BLA step
cannot start there. Testing `dz(2Z + dz)` instead of `dz` moves the
threshold onto the quantity that actually enters the linear part, so steps
can begin at and span those points.

## Building steps

Extending a step by one reference iteration with value `z`
(src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §LAInfo::Step):

```text
LAThreshold  = min(LAThreshold,  ‖z‖∞ / ‖ZCoeff‖∞ * 2^-24)
LAThresholdC = min(LAThresholdC, ‖z‖∞ / ‖CCoeff‖∞ * 2^-24)
ZCoeff = 2 z ZCoeff
CCoeff = 2 z CCoeff + 1
```

Composing step `a` with a following step `b`
(src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §LAInfo::Composite):

```text
tighten a's thresholds with b.Ref as above
ZCoeff = 2 b.Ref a.ZCoeff;  CCoeff = 2 b.Ref a.CCoeff
LAThreshold  = min(LAThreshold,  b.LAThreshold  / ‖ZCoeff‖∞)
LAThresholdC = min(LAThresholdC, b.LAThreshold  / ‖CCoeff‖∞)
ZCoeff = ZCoeff * b.ZCoeff
CCoeff = CCoeff * b.ZCoeff + b.CCoeff
```

The threshold scale is `2^-24` for both thresholds
(`LAThresholdScale = LAThresholdCScale = 2^-24`).

**Period detection.** While extending, a step "detects a period" when the
new `LAThreshold` collapses below `2^-10` of the old one, or when `‖z‖∞`
falls below a fraction of the step's minimum magnitude so far (`2^-6` in
stage 0, `2^-3` above). Either signals a near return of the reference
towards 0, and the step is cut there.

## Stages

- **Stage 0** walks the reference, extending a step until a period is
  detected, then starts a new step. Each step's length is one "macro
  iteration" of the stage.
- **Stage k+1** composes stage k's steps the same way.
- If no period is found, or the period exceeds 64× the previous stage's
  step length, a synthetic step length is chosen so stages coarsen by about
  16×: `Ratio^(1/round(log16 Ratio))` times the previous step length.
- Stages are built until no further period is found; at most 256 stages
  (`MaxLAStages = 256`).
- An index record per step holds its `StepLength` and `NextStageLAIndex`
  (where to continue in the finer stage), and a per-stage descriptor holds
  its first index, macro-iteration count and a "use double precision" flag
  (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §CreateLAFromOrbit).

Total entries are below 2M for an M-iteration reference (inference).

## Lookup per pixel

```text
stage = coarsest
loop:
    if ‖dc‖∞ >= stage.LAThresholdC: stage -= 1; continue
    j = position in this stage
    while j < stage.macroCount:
        if step j unusable for dz: break
        apply step j; n += StepLength(j)
        if bailout: done
        if ‖dz‖∞ > ‖z‖∞: dz = z; j = 0      // rebase within the stage
        else: j += 1
    if j == stage.macroCount: j = 0          // ran out: wrap (rebase)
    position in finer stage = NextStageLAIndex(j); stage -= 1
// below stage 0: ordinary perturbation steps with rebasing
```

There is no search over levels: try the next step of the current stage, and
if it is invalid descend one stage (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §EvaluateLA).
Imagina processes 4 pixels at a time with SIMD masks. Because steps are
period-aligned, a pixel near a minibrot can repeat "skip one period,
rebase" at the coarse stage instead of falling back to fine levels at every
period boundary, as an unaligned power-of-two tree does (inference).

## AT: the minibrot shortcut

Near a minibrot of period P, P delta iterations act like one quadratic
iteration in rescaled coordinates (the renormalisation idea behind knighty's
nanoMB, see [Series approximation (legacy)](series-approximation.md)). From the top-stage step
that covers one whole period (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §LAInfo::CreateAT):

```text
ZCoeff_AT = ZCoeff
CCoeff_AT = ZCoeff * CCoeff
RefC      = Ref_next * ZCoeff
SqrEscapeRadius = min(|ZCoeff|^2 * LAThreshold, 2^256)
ThresholdC      = min(LAThresholdC, 2^256 / ‖CCoeff_AT‖∞)
usable if  SqrRadius * |CCoeff_AT|^2 * 2^32 > |RefC|^2  and  SqrEscapeRadius > 4
```

Evaluation: map the pixel into AT coordinates, then iterate plain
`z ← z^2 + c` (and `dzdc ← 2 z dzdc + scale`) in ordinary precision; each
iteration counts as `StepLength` (one period) reference iterations, until
`|z|^2 > SqrEscapeRadius`. Then map back and continue with LA and
perturbation. AT is tried from the coarsest stage down and the first usable
one is taken; it is used when the reference is periodic or when maxIter is
below the reference length (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §EvaluateAT).

The name is often expanded as "approximation terminator" in forum usage;
not confirmed from a saved source (unverified).

## Precision handling

- Coefficients and thresholds are stored as double where the range allows
  (radius above about 2^-896, thresholds above about 2^-768), otherwise as
  FloatExp (mantissa plus separate exponent). The decision is per stage, so
  coarse stages with huge or tiny coefficients stay FloatExp while fine
  stages run in double. The reference is also converted to double when
  usable (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §ConvertStageToDouble).
- Derivatives ride along: `dzdc ← dzdc · 2(dz + Ref) · ZCoeff + CCoeff · scale`,
  the same for `dzdz`; held scaled (by 2^448) to avoid overflow. Distance
  estimation and interior detection through LA steps are therefore nearly
  free (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §EvaluateLA); see
  [Distance estimation](distance-estimation.md), [Interior detection](interior-detection.md).
- FractalShark's CUDA LA rows are 52 B with HDR float and 32-bit indices,
  plus 16 B per stage descriptor (src: [FractalShark README](../sources/fractalshark-readme.md)).

## Costs

- Build: O(M) on the CPU, serial per stage (inference).
- Per pixel: roughly the number of macro-steps actually taken plus a few
  hundred perturbation steps near escape and rebases; in a minibrot-centred
  view AT reduces the bulk to one cheap iteration per period (inference).
- No published benchmark compares Imagina LA with binary-tree BLA
  ([How much faster is Imagina-style LA than binary-tree BLA on a GPU?](../questions/bla-vs-la-benchmark.md)). FractalShark's author expects Imagina
  "will probably beat it on performance in the long term"
  (src: [FractalShark README](../sources/fractalshark-readme.md)).

## Limits

- Mandelbrot-specific as written (complex coefficients, critical point 0).
  For Burning Ship, Mandelbar and hybrids keep the matrix and fold-capped
  [Bivariate linear approximation (BLA)](bla.md) variants (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4 BLA Extensions).
- The thresholds use a fixed `2^-24` scale, i.e. float32-level accuracy even
  when evaluating in double.

## For almondbread

Roadmap step 9, after BLA, rescaling and the verification harness. A Metal
layout needs per-quantity exponents: `Ref`, `ZCoeff`, `CCoeff` as float2 +
int32 exponent each, thresholds as float + int32 (about 48 B per entry),
plus `{StepLength, NextStageLAIndex}` (inference). Pixels walk a stage
sequentially, so access is coalesced. Implement from this description only;
no code from Imagina, Fractal Zoomer or FractalShark. See
[Imagina](../implementations/imagina.md), [FractalShark](../implementations/fractalshark.md),
[Fractal Zoomer](../implementations/fractal-zoomer.md).

## As built (M6, 2026-10-15)

Code: `crates/almondbread-core/src/la.rs` (stages, chains, AT, CPU model `iterate_model`),
`crates/almondbread-render/src/la_gpu.rs` (upload, binding, mode), `iterate` in
`crates/almondbread-render/src/shaders/mandelbrot.metal` (function constants `kLA` = 8,
`kAT` = 9, buffers 25–29, plain and deep paths), the scheduler's `job_la` / `prefer_la`. Study
tool: `cargo run --release -p almondbread-core --example la_check`. Our design unless cited.

**Steps in BLA terms.** We keep the BLA validity of [Bivariate linear approximation (BLA)](bla.md) "As built" (per-pixel
`c`, `|u| + K|c| < R`, ε = 2⁻²⁴) instead of Imagina's two Chebyshev thresholds. A step from a
boundary `s` to `s'` is one exact perturbation step from `s` (Imagina's non-linear first
iteration, `u = dz(2Z_s + dz) + dc`) followed by an ordinary BLA block over the inner points
`s+1 … s'−1`. Composing steps of a stage linearises the boundaries in between: each becomes a
single BLA step `A = 2Z_b` merged with the BLA rule. So validity, derivatives (`d ← A d + B·texel`,
`w ← A w`) and the kernel's block code are BLA's; only where blocks start and end differs.

```text
stage 0:   units = single iterations; cut at k when |Z_k| < 2^-6 min|Z| of the step so far,
           or when adding k collapses R by 2^10 (the first inner point is always taken)
stage k+1: compose stage-k steps, linearising each boundary b; cut where |Z_b| < 2^-3 of the
           smallest boundary linearised so far, or R collapses by 2^10
synthetic: a natural step of more than 16 sub-steps is split into pieces of c^(1/round(log4 c))
           (stages coarsen about 4x where the orbit has no period structure)
stop:      when a stage no longer merges anything
```

**Lookup (deterministic in `m`).** Boundaries of stage 0 are *natural* (a near-return: the exact
step comes first, lookup index `L = b + 1`) or *synthetic* (linearised too: `L = b`, the block
includes `single(b)`). Each lookup index carries a chain, stage 0 upwards, of the blocks starting
there; validity is monotone along it (each block is a prefix of the next: `R` falls, `K` grows),
blocks of length 0 are dropped, and `L` grows strictly. A pixel looks up only at `m == L_j`:

```text
j = laNext[m] (resume) ; after a rebase j = 0 (L_0 = 1)
at m == L_j:  first entry invalid -> none (one test, the common case past the linear range)
              else last entry valid -> take it (two tests, deep in the linear range)
              else binary search (a function of the state only: chunk-independent)
   valid:   dz = A dz + B c, m += len, n += len, j = endJ   (endJ: the next L >= the block's end)
   invalid: j += 1
between lookups: the plain exact loop (it breaks at m == L_j)
```

Rebasing brings a pixel back to `m = 0`, so every period of a nucleus reference is skipped again
("skip one period, rebase"). Seeds (M5) keep BLA blocks; pixels starting from a leaf pick up the
LA chain at the leaf's `m`.

**AT.** For a nucleus reference (`|Z_P| < 2^-20`) the top stage composed into the period step
gives `y = A z`, `y ← y² + c'`, `c' = A(A+B)dc + A Z_P`, valid while
`|y|² < |A|²(R − (K+1)|dc|)` (bound capped at 2^100), each AT step `P` iterations, `D = A·dz/dc·texel`
and `w` (`w = A` after the first step from `z_0 = 0`, then `w ← 2y w`) in floatexp. A pixel
inside the AT between chunks keeps `y` itself in its state (`e = AT_MODE`), so results do not
depend on chunk cuts. Built and verified, **off by default**:
it saved 4–10 % of the steps but ran between 2 % faster and 17 % slower on the GPU. **Removed
2026-10-17** (switch `ALMONDBREAD_AT`, `--at`, the kAT kernel path and `AtInfo`), never enabled.

**Tables** are small and fast to build on a thread (needle-1e1004: 277 entries, 17 KB, 0.3 ms;
sea-8007: 2 913 entries in 4 stages, 187 KB, 2.6 ms; 40 000-point centre references ~10 ms).
Plain-kernel entries are float32 (`|A|, |B| ≤ 2^40`, an invalid entry invalidates the rest of
its chain), deep entries `BlaEntryX` plus `len`, `endJ` (48 B).

### Result (measured 2026-10-15, M1 Pro, release; [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M6)

- **Needle minibrots (nucleus references of period 79, 255, 834, limits of 120–250 periods):
  1.9–2.4× less GPU time than BLA** (1600 × 1000, interleaved medians), 1.5–2.1× fewer loop
  steps. Here the period-aligned blocks pay: BLA's power-of-two blocks break at every
  near-return of the period, LA starts its blocks right after them.
- **sea-8007 (period 8007, 5 periods in the limit): 1.10×.** The cost there is the non-linear
  tail of each orbit (deltas of 2^-30 against |Z| ~ 1, linear for 3–10 steps), which no linear
  method skips.
- **Centre references (sea-1e20, the dendrites, the spiral): no gain.** In the CPU model LA takes
  0.9–1.2× BLA's steps there; on the GPU it is even or slower. At sea-1e20 and the spiral the
  M5 seeds already took the linear start and the pixels' own loops have no valid block at all
  (pixel iterations = steps); at the spiral the deltas reach the orbit's size within ~2 000
  iterations (M5), so nothing linear is left to skip — LA does not change that.
- **Plain-kernel views: 1.4–5× slower if forced** (home-100k 5×, Feigenbaum 1.7×): a lookup at
  every near-return of a non-linear orbit costs a dependent load and a branch out of the tight
  loop every few steps, for nothing.

So `auto` (the default) uses LA only in the deep kernel on a nucleus reference whose limit spans
at least 4 periods; everywhere else BLA (or nothing) as before, and those pipelines are
unchanged. **LA does not replace BLA**: BLA stays for centre references, for the seeds and for
future formulas ([Burning Ship and abs variations](../formulas/burning-ship.md) needs its fold-capped variant).

### Pitfalls

- A lookup rule on per-pixel state that does not survive chunk boundaries breaks "shown frame =
  fresh render" (as for BLA). Ours depends on `m` alone; the AT keeps `y` in the state; the
  chain search depends on the state only, never on a remembered stage hint (a hint-based search
  gives the same answer only under exact monotonicity, which float rounding of `R`, `K` does not
  promise).
- Two lookup indices may not coincide: a synthetic boundary right after a natural one gets the
  same `L`; it is made natural.
- Synthetic boundaries must be linearised (lookup at `b`, not `b + 1`): an exact step at every
  synthetic boundary doubled the loop steps on non-periodic orbits (dendrite model 1.5× worse
  than BLA).
- Fresh pixels may start at `z_0 = 0` only when the AT takes them (the deep full step would
  zero `w` there and "prove" interior).
- Coarse stages (Imagina-like 16×) ran needle-1e1004 about 1.5× slower than 4× stages (one A/B: 37 vs 24 ms medians).
