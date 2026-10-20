---
title: "mandelbrot-webgpu README (Desarso)"
type: source
authors: ["Desarso"]
year: 2026
raw: "web/desarso-mandelbrot-webgpu-readme.md"
status: digested
tags: [mandelbrot-webgpu, webgpu, gpu-reference-orbit, rebasing, bla, extended-range-floats, gpl]
---

# mandelbrot-webgpu README (Desarso)

**URL:** <https://raw.githubusercontent.com/Desarso/mandelbrot-webgpu/main/README.md> · **Saved:** `raw/web/desarso-mandelbrot-webgpu-readme.md` · **Terms:** GPL-3.0 repository

## What it is

The README of a browser Mandelbrot renderer (WebGPU, with a WebGL2 fallback), live at mandelbrot.gabrielmalek.com. It is GPL-3.0 and was pushed 2026-10-01. It computes the reference orbit in fixed-point arbitrary precision on the GPU, carries per-pixel deltas with their own exponent, rebases, and uses a second-order bilinear skip table. It is unusually frank about bugs found, measurements and rejected ideas.

## What it covers

- Perturbation `w ← 2·X·w + w² + δ`; rebasing ("whenever `|z| < |w|`, restart against the start of the same orbit") instead of glitch detection or extra references.
- GPU reference orbit in fixed point: `|z| < 16` and `|c| < 2` hold, so no exponent is needed. Two's-complement integers over `LIMBS` u32 words scaled by `2^-(32·(LIMBS−1))`; add/sub are plain carry chains.
- WGSL has no 64-bit integers: a 32x32→64 multiply is built from four 16-bit partial products.
- One workgroup runs the whole orbit (WebGPU has no grid-wide sync). Multiplication spreads columns over 256 threads; the carry pass is serial. One dispatch advances hundreds of iterations; the CPU reads a 4-word status buffer per batch. Samples come out as (mantissaHi, mantissaLo, exponent).
- Limb count from zoom: 8 limbs (67 decimal digits) down to about `1e-42`; top profile 256 limbs.
- Deltas carry an explicit mantissa/exponent pair, renormalised every iteration. The old WebGL path failed because f32 deltas underflowed.
- Two silent bugs: (1) renormalising with one `exp2(−shift)` overflows once the mantissa is below `2^-128` (when the reference passes near zero), so the rescale is split in two halves; (2) a value wrapped as `Hdr(x, 0)` keeps its size in the mantissa, so an exponent-first comparison got `|z| ≫ |w|` wrong at a nucleus (reference returns within `1e-24` of zero). Rebasing never fired and frames came out solid black.
- Skip table: `w_out = A·w + B·δ + C·w² + D·w·δ + E·δ²`. Keeping C, D, E pushes the first neglected term to cubic, so the allowed entry delta grows by about its square root. Steps come in doubling levels (8, 16, 32, … iterations); a pixel greedily takes the largest step it fits.
- Measured at 2.8e40x zoom: 21x faster, 96.9 % of iterations skipped, bit-identical to the unapproximated render.
- Over a 4096-iteration step `A` reaches about 10^700, so coefficients and radii need explicit exponents; as plain doubles they become `Infinity` and radii `NaN`.
- Method by depth (units per pixel): `> 1e-5` direct f32; `> 1e-25` perturbation with plain f32 delta; otherwise exponent-carrying delta plus skip table. At the home view the rebase test fires about 60 million times, so perturbation is wrong there.
- Timings at 720x480: `4e-3` from 39 ms + 37 ms orbit to 15 ms; `1e-11` from 90 to 25 ms; `2e-21` from 193 to 132 ms.
- Compute and shade are separate passes. The stored field is keyed on what changes the numbers, so a palette change recomputes nothing. At `1e-25` and 20,000 iterations: full frame 247 ms, recolour 1.0 ms. The distance-mode gradient reads the stored field: 20.6 ms vs ~90 ms at `1e-8`.
- Responsiveness: frames split into horizontal bands sized to ~50 ms per submission to avoid the driver watchdog (which kills the device and tab). Fencing between bands is slow: eight fenced bands 569 ms vs 215 ms unsplit. Last frame reprojected under the new view while computing; resolution (never iterations) reduced while interacting, restored 160 ms after input stops.
- Find minibrot: Newton on `f_p(c) = 0`, then the standard size estimate to frame it, and raise iterations to the period.
- Verification: 70 CPU checks against a BigInt oracle; 18 GPU self-tests (`mul32` over 4102 cases; big add/sub/mul at 8/16/64 limbs; orbits sample by sample; batch sizes 1, 7, 128). `&verify=1` diffs with and without approximation. `&sweep=1` calibrates the auto-iteration curve.
- Orbit generation time on the same machine ranged from 292 ms to 15 s within minutes; single measurements misled three times.
- Rejected: NTT multiplication (schoolbook wins at 8 limbs); orbit compression (24 bytes/sample, 200k iterations ≈ 4.8 MB); series approximation (bilinear is more general); a Web Worker split (only 17–25 ms main-thread work for the skip table).
- WebGL2 path runs out of precision around `1e-34`. Only tested on Chrome on macOS.

## Reliability

- Primary for this implementation, with the author's own measurements on unnamed hardware (Chrome on macOS). Claims are specific and checkable, and several bugs are documented.
- "Bit-identical" is claimed for one view at 2.8e40.
- Credits FractalShark's author as "Matt Renzelmann"; the manifest has "Matt Reiman" (see [FractalShark README](fractalshark-readme.md)).
- GPL-3.0: algorithms only, no code.

## Useful sections

- §Perturbation: iterate once, reuse everywhere — rebasing rule.
- §Arbitrary precision on the GPU — fixed-point limbs, 16-bit partial products, single-workgroup orbit, limb counts.
- §Deltas that carry their own exponent — the two silent bugs.
- §Skipping most of the work — second-order skip table, 21x result, exponent overflow of coefficients.
- §Choosing a method by depth — thresholds and timings.
- §Computing a frame and colouring it are separate — compute/shade split, recolour cost.
- §Staying responsive — banding, watchdog, fencing cost, reprojection.
- §Finding somewhere worth looking — Newton nucleus finder.
- §Verification — oracle tests, verify/sweep/bench flags.
- §Measured and rejected — NTT, compression, SA, workers.

## Used by

- [mandelbrot-webgpu (Desarso)](../implementations/mandelbrot-webgpu.md)
- [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)
- [Reference orbit: precision, cost, storage, compression](../techniques/reference-orbit-precision.md)
- [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)
- [Automatic maximum-iteration control](../techniques/auto-iterations.md)
- [GPU watchdog and chunked, resumable kernels](../platform/gpu-watchdog-and-chunking.md)
- [Is a higher-order (second-order) BLA worth it?](../questions/higher-order-bla.md)
