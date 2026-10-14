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
[[techniques/bla]] in four ways: the first iteration of each step is done
non-linearly, step boundaries follow the reference's returns towards 0 (its
period structure), lookup is a sequential walk with no search, and a final
"AT" step iterates an ordinary low-precision Mandelbrot in rescaled
coordinates near a minibrot. Fractal Zoomer and FractalShark's CUDA
"LA/LAv2" descend from it (src: [[sources/fractalshark-readme]]).

Imagina is AGPL-3.0. Everything below is a description in our own words
of what the code does (src: [[sources/imagina-hinfla-evaluator]]), not code.
The repository's last push was 2024-01-23 and the README says the project
"is being rewritten" (src: [[sources/imagina-readme]]).

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
`‖dc‖∞ < LAThresholdC` (src: [[sources/imagina-hinfla-evaluator]] §EvaluateLA).

Why the non-linear first step matters (inference): at the reference's near
returns to 0 the binary-tree radius `ε|Z|` collapses to 0, so a BLA step
cannot start there. Testing `dz(2Z + dz)` instead of `dz` moves the
threshold onto the quantity that actually enters the linear part, so steps
can begin at and span those points.

## Building steps

Extending a step by one reference iteration with value `z`
(src: [[sources/imagina-hinfla-evaluator]] §LAInfo::Step):

```text
LAThreshold  = min(LAThreshold,  ‖z‖∞ / ‖ZCoeff‖∞ * 2^-24)
LAThresholdC = min(LAThresholdC, ‖z‖∞ / ‖CCoeff‖∞ * 2^-24)
ZCoeff = 2 z ZCoeff
CCoeff = 2 z CCoeff + 1
```

Composing step `a` with a following step `b`
(src: [[sources/imagina-hinfla-evaluator]] §LAInfo::Composite):

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
  (src: [[sources/imagina-hinfla-evaluator]] §CreateLAFromOrbit).

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
if it is invalid descend one stage (src: [[sources/imagina-hinfla-evaluator]] §EvaluateLA).
Imagina processes 4 pixels at a time with SIMD masks. Because steps are
period-aligned, a pixel near a minibrot can repeat "skip one period,
rebase" at the coarse stage instead of falling back to fine levels at every
period boundary, as an unaligned power-of-two tree does (inference).

## AT: the minibrot shortcut

Near a minibrot of period P, P delta iterations act like one quadratic
iteration in rescaled coordinates (the renormalisation idea behind knighty's
nanoMB, see [[techniques/series-approximation]]). From the top-stage step
that covers one whole period (src: [[sources/imagina-hinfla-evaluator]] §LAInfo::CreateAT):

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
below the reference length (src: [[sources/imagina-hinfla-evaluator]] §EvaluateAT).

The name is often expanded as "approximation terminator" in forum usage;
not confirmed from a saved source (unverified).

## Precision handling

- Coefficients and thresholds are stored as double where the range allows
  (radius above about 2^-896, thresholds above about 2^-768), otherwise as
  FloatExp (mantissa plus separate exponent). The decision is per stage, so
  coarse stages with huge or tiny coefficients stay FloatExp while fine
  stages run in double. The reference is also converted to double when
  usable (src: [[sources/imagina-hinfla-evaluator]] §ConvertStageToDouble).
- Derivatives ride along: `dzdc ← dzdc · 2(dz + Ref) · ZCoeff + CCoeff · scale`,
  the same for `dzdz`; held scaled (by 2^448) to avoid overflow. Distance
  estimation and interior detection through LA steps are therefore nearly
  free (src: [[sources/imagina-hinfla-evaluator]] §EvaluateLA); see
  [[techniques/distance-estimation]], [[techniques/interior-detection]].
- FractalShark's CUDA LA rows are 52 B with HDR float and 32-bit indices,
  plus 16 B per stage descriptor (src: [[sources/fractalshark-readme]]).

## Costs

- Build: O(M) on the CPU, serial per stage (inference).
- Per pixel: roughly the number of macro-steps actually taken plus a few
  hundred perturbation steps near escape and rebases; in a minibrot-centred
  view AT reduces the bulk to one cheap iteration per period (inference).
- No published benchmark compares Imagina LA with binary-tree BLA
  ([[questions/bla-vs-la-benchmark]]). FractalShark's author expects Imagina
  "will probably beat it on performance in the long term"
  (src: [[sources/fractalshark-readme]]).

## Limits

- Mandelbrot-specific as written (complex coefficients, critical point 0).
  For Burning Ship, Mandelbar and hybrids keep the matrix and fold-capped
  [[techniques/bla]] variants (src: [[sources/mathr-deep-zoom-page]] §4 BLA Extensions).
- The thresholds use a fixed `2^-24` scale, i.e. float32-level accuracy even
  when evaluating in double.

## For almondbread

Roadmap step 9, after BLA, rescaling and the verification harness. A Metal
layout needs per-quantity exponents: `Ref`, `ZCoeff`, `CCoeff` as float2 +
int32 exponent each, thresholds as float + int32 (about 48 B per entry),
plus `{StepLength, NextStageLAIndex}` (inference). Pixels walk a stage
sequentially, so access is coalesced. Implement from this description only;
no code from Imagina, Fractal Zoomer or FractalShark. See
[[implementations/imagina]], [[implementations/fractalshark]],
[[implementations/fractal-zoomer]].
