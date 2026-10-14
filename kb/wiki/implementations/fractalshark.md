---
title: "FractalShark"
type: implementation
status: draft
license: "GPL-3.0"
sources: ["[[sources/fractalshark-readme]]", "[[sources/desarso-mandelbrot-webgpu-readme]]"]
tags: [implementation, cuda, linear-approximation, gpu-reference-orbit, orbit-compression]
---

# FractalShark

A CUDA Mandelbrot renderer by GitHub user mattsaccount364, self-described as
"an experimental research project, not a polished end-user application"
(src: [[sources/fractalshark-readme]]). The closest existing design to a
float32-GPU deep zoomer.

## Algorithms

- Perturbation (KF as inspiration) with "perturbed perturbation": a custom
  allocator caches intermediate-resolution reference values and reuses them
  across successive zooms (src: [[sources/fractalshark-readme]]).
- Two CUDA linear-approximation implementations ported from Fractal Zoomer,
  originating with Claude and Zhuoran; kernel `HDRx32 GPU - LAv2` chosen
  manually; LA rows of 52 bytes (HDR float, 32-bit indices) plus 16-byte stage
  descriptors (src: [[sources/fractalshark-readme]]; see
  [[techniques/linear-approximation-imagina]]).
- Reference-orbit compression (Zhuoran's approach, after a sample by mathr)
  extended to on-demand decompression of orbit segments during rendering;
  saved multiple GB at period 600,000,000; compression levels 1-20; reads and
  writes Imagina-compatible files (src: [[sources/fractalshark-readme]]). See
  [[questions/reference-orbit-compression]].
- Feature Finder: finds periodic points and refines them with high-precision
  Newton in Direct, Perturbation or LA modes; the Newton loop can run on the
  GPU inside the NTT kernel (src: [[sources/fractalshark-readme]]).
- GPU supersampled AA, palette colouring, per-frame iteration statistics by
  GPU reduction (src: [[sources/fractalshark-readme]]).

## Number types and GPU

- float, double, double-float, quad-double, HDR float+exponent, and a custom
  "2x32 + exponent" type (two float32 plus a shared exponent, ~48-bit
  mantissa, CUDA only) that gives "substantially higher performance on
  consumer GPUs with only a modest loss of precision"; on CPU native double is
  preferable (src: [[sources/fractalshark-readme]]).
- CUDA compute capability 7.5+; GPU reference orbit needs RTX 20xx+. Reference
  orbits on CPU via MPIR (Windows, AVX2) or GMP (Linux); "MT3" uses three
  threads, two squaring and one coordinating. GPU reference orbit "Ref2/v2":
  persistent fused kernel with NTT multiplication
  (src: [[sources/fractalshark-readme]]).

## Performance (author's own)

- GPU reference orbit about 10x faster than multithreaded MPIR+AVX2 on an
  RTX 4090 at 16384 32-bit limbs (~158k digits), ~9x on an RTX 5090; ~70x
  versus single-threaded MPIR (v0.54). GPU likely wins only beyond ~1e10000
  (src: [[sources/fractalshark-readme]]).
- Deepest built-in example: a minibrot at ~1e650452 (View #34)
  (src: [[sources/fractalshark-readme]]).
- No per-pixel LA speedup figures are published.

## Status

Active: last push 2026-10-12 (gh api); v0.53 (2026-06) added native Linux;
over 1,090 commits. AMD support "never planned"; the CPU path "is largely a
debugging aid" (src: [[sources/fractalshark-readme]]). Algorithm details are in
FractalShark.pdf (not saved).

## Contradictions

- Author name: the research notes give "Matt Reiman"; the mandelbrot-webgpu
  credits say "Matt Renzelmann" (src: [[sources/desarso-mandelbrot-webgpu-readme]]
  §Credits). The README we saved does not state a surname. Cite as
  "FractalShark (mattsaccount364)".

## What almondbread can learn

GPL-3.0: algorithms only. The type ladder float -> 2x32+exp -> HDR float+exp
matches Apple's no-fp64 GPU ([[techniques/double-float]],
[[techniques/extended-range-floats]]); LA table layout with per-quantity
exponents; orbit compression for iPad memory ([[platform/ios-ipad-considerations]]);
MT3 threading for the CPU reference ([[platform/mpfr-gmp-on-apple-silicon]]).
A GPU bignum is not worth it on Apple GPUs given slow integer multiply
([[platform/apple-gpu-architecture]]).
