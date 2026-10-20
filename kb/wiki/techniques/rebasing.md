---
title: "Rebasing (Zhuoran, 2021)"
type: technique
status: draft
sources: ["[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/kf-manual]]", "[[sources/desarso-mandelbrot-webgpu-readme]]", "[[sources/muency-bla]]", "[[sources/imagina-hinfla-evaluator]]"]
tags: [perturbation, rebasing, glitches, core]
---

# Rebasing (Zhuoran, 2021)

When the pixel orbit comes closer to the critical point than its own delta
is large, restart the pixel against the start of the same reference orbit.
Glitches are then avoided rather than detected, and one reference serves the
whole image for power-2 Mandelbrot.

## Rule

After every perturbation step (or skip step), with `Zfull = Z_m + z_n`:

- if `|Zfull| < |z_n|`, or the reference has run out (`m` reached its last
  valid iteration),
- then `z_n ← Zfull` and `m ← 0`; continue iterating (`n` is not reset)
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §2.1 Rebasing).

Zhuoran's original post (fractalforums.org topic 4360, 3 August 2021) gives
exactly this loop: step the delta, increment the reference index, form
`z = Reference[m] + dz`, test bailout, then reset when `|z| < |dz|` or the
reference index hits its last valid iteration. The reference "MUST START
WITH ZERO". He reported it "very reliable, even when single precision is
used", with a test location rendered correctly in float32 at height 4e-15
with 5,000,000 iterations (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Another solution to perturbation glitches).

## Why it is exact

Perturbation holds for any reference: `Z_pixel = Z_ref1 + z_1 = Z_ref2 + z_2`,
so `z_2 = Z_ref1 + z_1 − Z_ref2`. Choosing the reference value 0, which every
Mandelbrot reference has at iteration 0, gives `z_2 = Z_pixel`: just reset
the reference counter. `C` is unchanged so `c` is unchanged
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #6, mathr).
mathr: "nothing about it depends on anything special about the Mandelbrot
set, just 2 things are needed: basic property of perturbation, and an
iteration at which the reference orbit is 0" (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10).

Why glitches vanish (inference, consistent with [Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](glitch-detection.md)):
a glitch needs the delta to be comparable to or larger than the full value,
so that pixel and neighbours "stick together". Rebasing keeps
`|z| ≤ |Z + z|` at all times, and the reset itself has no cancellation
because the new reference value is exactly 0.

## Pseudocode

```text
m = 0; z = 0
for n in 0 ..< maxIter:
    (z, m, steps) = skipOrStep(z, m, c)     // BLA/LA skip, else one step
    n += steps - 1
    Zfull = Zref[m] + z
    r2 = norm2(Zfull)
    if r2 > bailout2: escape
    if r2 < norm2(z) or m == M:            // rebase
        z = Zfull; m = 0
```

After a rebase `m = 0`, where the BLA radius is 0 (the reference is at the
critical point), so one ordinary perturbation step always follows
(src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear Approximation).

## Consequences

- **One reference** per critical point. Mandelbrot and Burning Ship need
  one (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing).
- **Short references are fine only where they end exactly.** A pixel that
  outruns the reference wraps to the start. With a nucleus reference of
  period p, `Z_p = 0`, so wrapping at `m = p` does not change the value
  (inference); see [Reference selection: nucleus, period detection, Newton](reference-selection.md). After the reference
  escaped, an unescaped pixel's `|z|` is comparable to `|Z|` and the reset
  loses nothing either. A centre orbit cut off before it escaped is not such
  a point: see "Truncated references" below.
- **`n ≠ m`**: the kernel must carry both counters. Colouring uses `n`.
- **Speed**: in a KF test at Dinkydau's "Flake", 1 reference took 18.5 s
  versus several minutes for 97 references, with smaller image differences
  than the usual 22-reference speed setting (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #9).

## Multiple critical points and hybrids

- Several critical points: keep one reference per critical point; at the
  rebase test pick the orbit `o` minimising `|(Z − Z_o) + z|`, comparing the
  current reference at its current index with each critical-point orbit at
  index 0 (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing).
- Hybrids: one reference per phase of the hybrid loop; "(M,BS,M,M)" needs
  four, its four rotations. A pre-periodic part adds references for each
  pre-periodic iteration. References are independent and can be computed in
  parallel (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing). See
  [Hybrid formulas](../formulas/hybrids.md).
- One BLA table per reference (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.3 Hybrid BLA).
- As of 2022 mathr had implemented it only for a single critical point at 0
  (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear Approximation);
  Fraktaler 3 implements no formula with multiple critical points
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.4 Multiple Critical Points).

## In existing renderers

- KF 2.15.5 shipped the single-reference method, on by default ("thanks
  Zhuoran"). Known issues: hybrids broken with it (a proper fix needs a
  reference per phase), and "single reference method does not cure all
  glitches" (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §Known Bugs; [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §kf-2.15.5).
- Imagina rebases inside each LA stage when `‖dz‖ > ‖z‖`, using Chebyshev
  norms, and also when a stage runs out (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md) §EvaluateLA).
- Fractal Zoomer's author reported using it for Mandelbrot powers 2 to 5,
  Burning Ship, lambda, Nova, magnet, Newton parameter space and Julia sets
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #8; §Reply #11).

## Pitfalls

- **Extended-range comparisons.** A WebGPU float32 renderer stored a value
  with magnitude in the mantissa and exponent 0, and its rebase test compared
  exponents first; at a minibrot nucleus rebasing never fired and the frame
  went black (src: [mandelbrot-webgpu README (Desarso)](../sources/desarso-mandelbrot-webgpu-readme.md) §Deltas that carry their own exponent). Compare
  normalised magnitudes.
- **Squared norms underflow** in float32 near 1e-19; deeper, use
  Chebyshev/max-abs norms or compare in floatexp
  ([Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md)).
- **History.** The research notes cite "August 2021" (a nostr post) and
  Mu-Ency says Zhuoran introduced the rebasing tests "in December 2021"
  (src: [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)). The archived forum topic settles it:
  rebasing was posted 3 August 2021; BLA followed later in the same thread.

### Truncated references

The reset `z ← Z_m + z` is exact in real numbers but rounds to the delta
type. At the end of a centre orbit that has *not* escaped, a pixel that got
there typically has `|z| ≪ |Z_m|` (it follows the reference closely); in
float32 the sum keeps only `|Z_m| · 2^-24` of it, so the pixel's offset from
the reference is rounded away and every such pixel continues as one orbit.
Our measurement (2026-10-16, M1 Pro, release): next to the Misiurewicz point
of `spiral-1e1000` at 1e-302 per pixel, a 6,251-point prefix of a centre
orbit that escapes at 20,899 made all 256,000 pixels escape at ~6,250-6,660
(the prefix end plus the ~400 iterations float32 noise needs to escape near a
repelling cycle) instead of 12,000-20,500: one flat colour, `escape-n differs
100 %`, median |Δmu| 1.1e4 against the CPU oracle. With the whole orbit the
same view agrees with the oracle as well as a 0.1-pixel grid shift does
(inference from `almondbread precision`: chaotic at this sampling).

Rule (inference): a reference may end only where the reset is exact: an
escape, a nucleus period (`Z_p = 0`), or the iteration limit (a pixel's `m`
never exceeds its `n`). A prefix of a longer orbit is fine for pixels that
escape before its end and wrong for the rest, so a frame drawn with one is
provisional. KF and Fraktaler compute the reference to the iteration limit
before rendering, so they do not meet this (unverified).

In almondbread: `Reference::truncated_before(limit)` names such a reference;
the scheduler renders a view drawn with one again once the whole orbit is in,
keeps the learner off its statistics, waits for the whole orbit offscreen,
and never extends pixels past a truncated orbit's end. Before this
(R6 progressive delivery, [Faster deep references: three-thread orbit, progressive delivery, cheap Newton](parallel-reference-orbit.md)) nothing
polled the worker while a job's key stood, so the provisional frame stayed:
mini-maps between ~1e300 and ~1e670 near `spiral-1e1000` (and the 1e2007 level
at `rock-bottom`) settled flat or as a tiled pattern, and so did a main view
after a jump there (`verify --history jump --place spiral-1e1000 --scale
1e-302 --iterations 50000`, now in `make regress`).

**The iteration limit is an exact end only up to the limit.** Extension passes
([Automatic maximum-iteration control](auto-iterations.md)) iterate unresolved pixels to 16× the limit, so
a centre orbit cut *at* the limit is truncated for them. The guard above only
covered big references; the f64 orbit (views of 1e-12 per point and shallower)
was computed to exactly the limit. User report (2026-10-16): near the −3/4 neck
at `−0.749999947197051 + 3.69e-4 i` the centre escapes at 17 019 (≈ π/ε). After
zooming in to 1e-13 and back out, the depth limit was ~4 400–4 750, and the
passes to 18 000–70 000 restarted every pixel at the 4 400-point cut. The frame
turned into blocky noise with black wedges with straight edges (the user's "black
clover with a straight-edged wedge"; frames at slightly smaller limits came out
grainy). Measured (2026-10-16, M1 Pro, release, `verify --history zoom ... --then
0.05,...`): median |Δmu| against the oracle 1.9–6.5, class mismatch up to 25 %. A
fresh `render` of the view looked right because it extends nothing. Fix:
`Reference::exact_reach()` (unbounded after an escape or at a nucleus, else the
last index) caps the extension for every reference, and the scheduler computes
the f64 orbit as far as the passes may go (limit × extension factor, at most the
cap; under a millisecond, and BLA builds off the main thread). Afterwards the same
frames have a median ≤ 3e-4 and bench iteration counts are unchanged. `make regress`
runs the path with `--max-cpu-median 0.05`.

## For almondbread

Implement rebasing in the first GPU kernel, with norm tests that do not
underflow. Drop glitch detection except as a debug overlay
([Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](glitch-detection.md)). Use one reference per critical point per
phase when other formulas arrive ([Hybrid formulas](../formulas/hybrids.md)).
