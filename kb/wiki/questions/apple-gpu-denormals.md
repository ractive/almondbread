---
title: "Do Apple GPUs flush float32 denormals to zero?"
type: question
status: open
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/luma-gl-gpu-precision]]", "[[sources/desarso-mandelbrot-webgpu-readme]]"]
tags: [float32, denormals, apple-gpu, range]
---

# Do Apple GPUs flush float32 denormals to zero?

## Question

In Metal compute kernels on Apple GPUs (M1 to M5, A-series), are float32
subnormals (1.4e-45 to 1.18e-38) preserved, flushed to zero, or does it depend on
`mathMode`?

## Why it matters

- It sets the float32 floor for deltas and the handover depth to rescaled float32
  ([Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)). mathr gives the single-precision limit as
  "below about 1e-38" (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Deep Needle).
- It decides whether double-float's low limb tapers gracefully: luma.gl notes
  precision tapers when the low limb is subnormal, for magnitudes about 2^-126 to
  2^-102 (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §luma.gl's Apple/Metal Path).
- The research notes assumed flush-to-zero as the safe case (notes: perturbation_core §7).

## What sources say

No fetched source documents Apple GPU denormal behaviour; the Metal Shading
Language specification was not read.

## How to resolve

1. Read the Metal Shading Language spec section on floating-point (denormals,
   `-fdenormal-fp-math`, math modes).
2. Test: a kernel that computes `x * 0.5` repeatedly from 1e-37 and writes the bit
   patterns, under `.fast`, `.relaxed` and `.safe`, on M1 Pro and any newer device.
   Also test `fma` and add paths, which may differ.
3. Record the result here and update [Apple GPU architecture for compute](../platform/apple-gpu-architecture.md).

Note the practical floor is higher anyway: squared norms underflow near 1e-19
(notes: perturbation_core §7), and the float32 WebGPU renderer hands over to an
exponent-carrying delta at 1e-25 (src: [mandelbrot-webgpu README (Desarso)](../sources/desarso-mandelbrot-webgpu-readme.md)).
