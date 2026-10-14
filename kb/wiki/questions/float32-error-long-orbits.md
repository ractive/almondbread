---
title: "How large is float32 perturbation error over long orbits?"
type: question
status: open
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/kf-manual]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/desarso-mandelbrot-webgpu-readme]]", "[[sources/fractalshark-readme]]"]
tags: [float32, precision, verification, benchmark]
---

# How large is float32 perturbation error over long orbits?

## Question

With rebasing and a float32 delta (and a float32-rounded reference), how wrong do
smooth iteration counts, interior/exterior classification and DE get as iteration
counts reach 1e5 to 1e9, at which depths, and where in the image?

## Why it matters

almondbread's GPU has no fp64 ([[platform/apple-gpu-architecture]]). If float32
fails visibly near minibrots at high iteration counts, the float-float mode
([[techniques/double-float]]) moves from "repair option" to "required path".
The synthesis calls this the biggest technical risk
([[synthesis/deep-zoom-rendering-techniques]]).

## What sources say

- mathr: single precision (24 bits) "can be used successfully" for perturbed
  orbits; the stated problem is range (underflow below about 1e-38), not precision
  (src: [[sources/mathr-deep-zoom-theory]] §Rescaling).
- Zhuoran: the rebasing method "appears to be very reliable, even when single
  precision is used", with a single-precision example at height 4e-15 and 5,000,000
  iterations (src: [[sources/fractalforums-zhuoran-rebasing-bla]]).
- KF2+ disables single precision by default because it can cause "undetected
  glitches at some locations", limited to shallow zooms (src: [[sources/kf-manual]]).
- FractalShark uses a 2x32 + exponent type for "substantially higher performance on
  consumer GPUs with only a modest loss of precision", implying plain float32 was
  not enough for its author (src: [[sources/fractalshark-readme]] §5. Custom “2×32 + exponent” numeric type).
- The float32 WebGPU renderer verifies with an approximated-vs-unapproximated
  pixel diff and a BigInt oracle (src: [[sources/desarso-mandelbrot-webgpu-readme]]).
- No 2023 to 2026 quantitative study was found (notes: perturbation_core §8).

## How to resolve

1. Pick 10 to 20 benchmark locations: shallow spirals, minibrot centres at 1e50,
   1e300, 1e1000, iteration counts 1e4 to 1e8.
2. Render each with the GPU float32 path and with a CPU path using double (or
   floatexp) deltas and the same reference ([[techniques/perturbation]]).
3. Report per location: fraction of pixels whose integer n differs, max and
   p99 smooth-count error, interior/exterior flips, DE relative error; plot error
   against n.
4. Repeat with float-float deltas and with a float-float reference to separate
   delta rounding from reference rounding ([[techniques/reference-orbit-precision]]).
5. Record results here with date, machine and build.

Related: [[techniques/rebasing]], [[techniques/extended-range-floats]], [[questions/apple-gpu-denormals]].

## Partial answer (milestone 2 harness)

Measured 2026-10-14, M1 Pro, release, with `AlmondbreadMac --verify` (float32
perturbation with rebasing against a Double reference, vs direct Double iteration on
the CPU), 400×259; full table in [[synthesis/benchmarks]].

- Shallow views (home, minibrot at 4.5e−3 width): median |Δmu| 2–10e−7, 0.2–0.5 % of
  pixels with a different escape iteration, interior/exterior mismatches ≤ 0.01 %.
- Boundary-heavy views (seahorse 1e5, feigenbaum at 2e5 iterations): 4 % and 54 % of
  pixels differ, median |Δmu| up to 6. A control render on the CPU with every sample
  moved by 1e−5 pixel differs from itself almost as much (5.1 %, 45 %), so this is
  chaotic sensitivity of boundary pixels, not a float32 failure: any oracle that is not
  bit-exact would show it.
- Near the Double limit (deep-5e-14: 5e−14 per pixel, 6 000 iterations): 14.9 % of
  pixels differ by > 0.05 against 0.7 % for the 1e−5-pixel control, so here the
  float32 error is real (likely the float32 pixel offset `dc` and the float32-rounded
  reference, not the iteration count). Classification is still right (0.004 %
  mismatches) and the heatmap shows boundary bands, no glitch clusters.
- Not yet measured: deeper than 1e−13 (needs the arbitrary-precision reference of
  milestone 3), iteration counts above 2e5 against the oracle, DE error, and the split
  between delta rounding and reference rounding (step 4 of "How to resolve").
