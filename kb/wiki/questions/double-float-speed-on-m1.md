---
title: "Which extended-precision emulation is fastest and correct on Apple GPUs?"
type: question
status: open
sources: ["[[sources/apple-forums-fp64-emulation]]", "[[sources/metal-benchmarks-readme]]", "[[sources/luma-gl-gpu-precision]]", "[[sources/thall-extended-precision]]", "[[sources/fractalshark-readme]]"]
tags: [double-float, apple-gpu, benchmark, precision]
---

# Which extended-precision emulation is fastest and correct on Apple GPUs?

## Question

On an M1 Pro, what do fma-based float-float (double-single), integer-controlled
double-single and integer-limb software floats cost per complex square and add,
and which stay numerically correct under Metal's compiler?

## Why it matters

Float-float is almondbread's repair and verification mode, and the type for the
shallow band before perturbation pays off ([[techniques/double-float]]). Choosing
the wrong one costs either speed or silent wrong answers.

## What sources say

- Native fp32 FMA is full rate and maps from `fma()`; int32 multiply is 1/8 rate;
  emulated fp64 costs 1:36 (add) to 1:68 (FMA)
  (src: [[sources/metal-benchmarks-readme]] §Operations per Second, §Instruction Throughputs).
- Thall's df64 algorithms (TwoSum, Split, TwoProduct) are the classic
  pairs-of-floats arithmetic for GPUs (src: [[sources/thall-extended-precision]]).
- FractalShark's 2x32 + exponent type is its fast consumer-GPU type
  (src: [[sources/fractalshark-readme]] §5. Custom “2×32 + exponent” numeric type).
- luma.gl switches to integer-controlled double-single on Apple WebGPU adapters
  because the compiler may reassociate or contract, trading throughput for reliable
  rounding (src: [[sources/luma-gl-gpu-precision]] §luma.gl's Apple/Metal Path).

## Contradictions

- Philip Turner argues integer-based fp64 emulation beats double-single on Apple
  GPUs (src: [[sources/apple-forums-fp64-emulation]]). His own benchmarks show
  integer multiply at 1/8 of fp32 FMA rate (src: [[sources/metal-benchmarks-readme]]),
  which suggests fma-based float-float should be faster for a 48-bit mantissa
  (inference). The two claims may be about different targets (full IEEE fp64 with
  53 bits and 11-bit exponent vs a 48-bit expansion with fp32 range).
- luma.gl's choice of the integer path is about correctness under WGSL rules, not
  speed; native Metal with `.safe` math may keep the classic path valid
  (src: [[sources/luma-gl-gpu-precision]] §Why This Is Fragile In WGSL And Metal).

## How to resolve

Microbenchmark on M1 Pro (and one M3+ device): a kernel doing N iterations of
z = z^2 + c in each representation, compiled with `.fast` and `.safe`; measure
iterations/s and check a known-answer residual (TwoSum(1, 1e-8) low limb nonzero,
and a 1000-step orbit compared with CPU double). Related:
[[questions/metal-fast-math-defaults]], [[platform/apple-gpu-architecture]].
