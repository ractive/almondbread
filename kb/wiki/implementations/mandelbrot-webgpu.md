---
title: "mandelbrot-webgpu (Desarso)"
type: implementation
status: draft
license: "GPL-3.0"
sources: ["[[sources/desarso-mandelbrot-webgpu-readme]]"]
tags: [implementation, webgpu, float32, bla, rebasing, verification]
---

# mandelbrot-webgpu (Desarso)

A browser Mandelbrot renderer (WebGPU, WebGL2 fallback) by GitHub user
Desarso, live at mandelbrot.gabrielmalek.com. The best-documented float32 GPU
deep zoomer, and the source of most known float32 pitfalls
(src: [mandelbrot-webgpu README (Desarso)](../sources/desarso-mandelbrot-webgpu-readme.md)). All facts below are from
that README.

## Algorithms

- Perturbation `w <- 2Xw + w^2 + delta` with rebasing when `|z| < |w|`; no
  glitch list, no second reference (§Perturbation: iterate once, reuse everywhere).
- Reference orbit computed **on the GPU** in fixed point: two's-complement
  u32 limbs (all samples satisfy `|z| < 16`, `|c| < 2`, so no exponent);
  32x32->64 multiply emulated with four 16-bit partial products; the whole
  serial recurrence runs in one workgroup with multiplication columns spread
  over 256 threads; 8 limbs (67 digits) reach ~1e-42, up to 256 limbs
  (§Arbitrary precision on the GPU). NTT was measured and rejected: views run
  at ~8 limbs where schoolbook wins (§Measured and rejected).
- Deltas carry their own exponent, renormalised every iteration. Two bugs that
  gave silently wrong images: a single `exp2(-shift)` overflows once the
  mantissa drops below 2^-128 (fixed by rescaling in two halves), and an
  exponent-first comparison in the rebase test on a value stored as
  `(magnitude in mantissa, exponent 0)` never fired at a minibrot nucleus, so
  frames came out black (§Deltas that carry their own exponent).
- Skip table with **second-order** terms:
  `w_out = A w + B delta + C w^2 + D w delta + E delta^2`; levels of 8, 16, 32...
  iterations; greedy largest valid step. At 2.8e40: 21x faster, 96.9% of
  iterations skipped, bit-identical to the unapproximated render. `A` over a
  4096-step reaches ~1e700, so coefficients and radii carry exponents
  (§Skipping most of the work). See [Is a higher-order (second-order) BLA worth it?](../questions/higher-order-bla.md).
- Method by depth (units/pixel): > 1e-5 direct f32; > 1e-25 perturbation
  with plain f32 delta; deeper, exponent-carrying delta plus skip table. The
  handover is "thirteen decades above the f32 floor"; the WebGL path's plain
  f32 deltas stopped at ~1e-34 (§Choosing a method by depth, §Layout).
- Compute and colour are separate passes; the stored field (count, |z|^2) is
  keyed on everything that changes the numbers. At 1e-25 with 20,000
  iterations a frame is 247 ms and a recolour 1.0 ms
  (§Computing a frame and colouring it are separate).
- Responsiveness: four bands per frame sized to ~50 ms per submission to stay
  under the driver watchdog; fencing between bands drained the pipeline (8
  fenced bands 569 ms vs 215 ms unsplit); reprojection of the last complete
  frame from a separate history texture; reduced resolution (never reduced
  iterations) while interacting (§Staying responsive).
- Find minibrot: Newton on `f_p(c) = 0`, size estimate, iterations raised to
  match the period (§Finding somewhere worth looking).
- Verification: CPU BigInt oracle tests, GPU self-test page running the real
  WGSL against the oracle, `verify=1` diff with/without approximation (caught
  two real bugs), `sweep=1` to calibrate auto-iterations, `bench=1` because
  single timings were noise (orbit time ranged 292 ms to 15 s)
  (§Verification).

## Status

Active: last push 2026-10-01 (gh api); GPL-3.0. Chrome on macOS is the only
browser tested; lost-device recovery not implemented.

## What almondbread can learn

Nearly everything is directly relevant to a float32 Metal renderer: the depth
handover table ([Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)), the magnitude-compare
rule for rebase tests ([Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)), per-quantity exponents in
skip tables ([Bivariate linear approximation (BLA)](../techniques/bla.md)), the verification modes, banded submission
under the watchdog ([GPU watchdog and chunked, resumable kernels](../platform/gpu-watchdog-and-chunking.md)), reprojection
([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)), and sweep-calibrated
auto-iterations ([Automatic maximum-iteration control](../techniques/auto-iterations.md)). GPL code must not be copied.
