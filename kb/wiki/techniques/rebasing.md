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
  (src: [[sources/mathr-deep-zoom-page]] §2.1 Rebasing).

Zhuoran's original post (fractalforums.org topic 4360, 3 August 2021) gives
exactly this loop: step the delta, increment the reference index, form
`z = Reference[m] + dz`, test bailout, then reset when `|z| < |dz|` or the
reference index hits its last valid iteration. The reference "MUST START
WITH ZERO". He reported it "very reliable, even when single precision is
used", with a test location rendered correctly in float32 at height 4e-15
with 5,000,000 iterations (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Another solution to perturbation glitches).

## Why it is exact

Perturbation holds for any reference: `Z_pixel = Z_ref1 + z_1 = Z_ref2 + z_2`,
so `z_2 = Z_ref1 + z_1 − Z_ref2`. Choosing the reference value 0, which every
Mandelbrot reference has at iteration 0, gives `z_2 = Z_pixel`: just reset
the reference counter. `C` is unchanged so `c` is unchanged
(src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #6, mathr).
mathr: "nothing about it depends on anything special about the Mandelbrot
set, just 2 things are needed: basic property of perturbation, and an
iteration at which the reference orbit is 0" (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #10).

Why glitches vanish (inference, consistent with [[techniques/glitch-detection]]):
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
(src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).

## Consequences

- **One reference** per critical point. Mandelbrot and Burning Ship need
  one (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing).
- **Short references are fine.** A pixel that outruns the reference wraps
  to the start. With a nucleus reference of period p, `Z_p = 0`, so wrapping
  at `m = p` does not change the value (inference); see
  [[techniques/reference-selection]].
- **`n ≠ m`**: the kernel must carry both counters. Colouring uses `n`.
- **Speed**: in a KF test at Dinkydau's "Flake", 1 reference took 18.5 s
  versus several minutes for 97 references, with smaller image differences
  than the usual 22-reference speed setting (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #9).

## Multiple critical points and hybrids

- Several critical points: keep one reference per critical point; at the
  rebase test pick the orbit `o` minimising `|(Z − Z_o) + z|`, comparing the
  current reference at its current index with each critical-point orbit at
  index 0 (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing).
- Hybrids: one reference per phase of the hybrid loop; "(M,BS,M,M)" needs
  four, its four rotations. A pre-periodic part adds references for each
  pre-periodic iteration. References are independent and can be computed in
  parallel (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing). See
  [[formulas/hybrids]].
- One BLA table per reference (src: [[sources/mathr-deep-zoom-page]] §4.3 Hybrid BLA).
- As of 2022 mathr had implemented it only for a single critical point at 0
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation);
  Fraktaler 3 implements no formula with multiple critical points
  (src: [[sources/mathr-deep-zoom-page]] §4.4 Multiple Critical Points).

## In existing renderers

- KF 2.15.5 shipped the single-reference method, on by default ("thanks
  Zhuoran"). Known issues: hybrids broken with it (a proper fix needs a
  reference per phase), and "single reference method does not cure all
  glitches" (src: [[sources/kf-manual]] §Known Bugs; [[sources/kf-manual]] §kf-2.15.5).
- Imagina rebases inside each LA stage when `‖dz‖ > ‖z‖`, using Chebyshev
  norms, and also when a stage runs out (src: [[sources/imagina-hinfla-evaluator]] §EvaluateLA).
- Fractal Zoomer's author reported using it for Mandelbrot powers 2 to 5,
  Burning Ship, lambda, Nova, magnet, Newton parameter space and Julia sets
  (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #8; §Reply #11).

## Pitfalls

- **Extended-range comparisons.** A WebGPU float32 renderer stored a value
  with magnitude in the mantissa and exponent 0, and its rebase test compared
  exponents first; at a minibrot nucleus rebasing never fired and the frame
  went black (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Deltas that carry their own exponent). Compare
  normalised magnitudes.
- **Squared norms underflow** in float32 near 1e-19; deeper, use
  Chebyshev/max-abs norms or compare in floatexp
  ([[techniques/extended-range-floats]]).
- **History.** The research notes cite "August 2021" (a nostr post) and
  Mu-Ency says Zhuoran introduced the rebasing tests "in December 2021"
  (src: [[sources/muency-bla]]). The archived forum topic settles it:
  rebasing was posted 3 August 2021; BLA followed later in the same thread.

## For almondbread

Implement rebasing in the first GPU kernel, with norm tests that do not
underflow. Drop glitch detection except as a debug overlay
([[techniques/glitch-detection]]). Use one reference per critical point per
phase when other formulas arrive ([[formulas/hybrids]]).
