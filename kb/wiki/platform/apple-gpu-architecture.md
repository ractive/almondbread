---
title: "Apple GPU architecture for compute"
type: platform
status: draft
sources: ["[[sources/metal-benchmarks-readme]]", "[[sources/apple-dynamic-caching-talk]]", "[[sources/apple-forums-fp64-emulation]]", "[[sources/luma-gl-gpu-precision]]"]
tags: [apple-gpu, metal, fp32, occupancy, hardware]
---

# Apple GPU architecture for compute

What the hardware means for the almondbread kernels. Most numbers are for the
Apple 7/8 GPU family (A14/A15, M1/M2), measured by Philip Turner; later
generations are less well measured.

## Arithmetic rates

- **fp32 is the native type.** M1/A15: 256 fp32 FMA per core per cycle (A14:
  128); fp32 add 128 per core-cycle (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Operations per Second).
- **fp16 FMA is also 256 per core-cycle**, so fp16 brings no ALU throughput gain
  on M1, only register and bandwidth savings (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Operations per Second).
  It is useless for perturbation deltas (range 6e-5 to 6.5e4, 11-bit
  mantissa) but fine for colour outputs (inference).
- **No fp64.** Emulated IEEE-like "FP59" (e11m48) costs about 1:36 for add, 1:52
  for mul and 1:68 for FMA versus fp32; a non-IEEE e8m48 format could roughly
  triple that (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Operations per Second).
- **Integer multiply is slow.** IMUL32 has a throughput of 4 cycles, i.e. 32 per
  core-cycle against 256 fp32 FMAs (1/8); IMUL(32x32=64) and IMULHI32 take about 8
  cycles; 64-bit integer add is emulated (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Instruction Throughputs, §ALU Layout).
  Consequence: a GPU bignum built from u32 limbs is slow, and float-float beats
  integer-limb arithmetic in the delta loop unless correctness forces the integer
  path (inference; see [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)).
- `fma()` in MSL maps to a native full-rate FFMA32 (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Instruction Throughputs),
  which is what TwoProd in [Double-float (float-float) arithmetic](../techniques/double-float.md) needs.

## Execution model and occupancy

- SIMD-group width is 32 threads. Each core has four schedulers, each issuing one
  instruction from one SIMD-group per cycle (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §ALU Layout).
- Per core: register file about 208 KB, threadgroup ("shared") memory about 60 KB,
  8 KB L1 data cache, 12 KB instruction cache, 384 to 3072 resident threads
  depending on register use; global cache line 128 B (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §On-Chip Memory).
- "ALU utilization maxes out at 24 simds/core". The compiler prefers to spill
  registers to device memory rather than drop occupancy below that (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Overview).
- Divergence: a SIMD-group of 32 pixels runs until its slowest pixel finishes.
  Escape-time kernels are badly divergent near the boundary, which is why
  [GPU watchdog and chunked, resumable kernels](gpu-watchdog-and-chunking.md) compacts live pixels between chunks (inference).
- The 12 KB instruction cache is small; one fat kernel with every feature
  compiled in can thrash it (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §On-Chip Memory shows performance vs executable size)
  — another reason for specialised pipelines in [Metal compute pipeline for a deep-zoom renderer](metal-compute.md).

## Dynamic caching (M3 and later)

- Before M3, a shader's worst-case register allocation set occupancy for the
  whole dispatch. M3 (A17 Pro) allocates registers from on-chip storage by live
  use, and registers, threadgroup and tile memory share one flexible pool; Apple
  says no app changes are needed (src: [Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)](../sources/apple-dynamic-caching-talk.md)).
- The M5-generation talk describes a second-generation dynamic caching with lower
  register latency and an occupancy management unit, and warns that too many
  SIMD-groups can exceed on-chip storage and cause spills (src: [Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)](../sources/apple-dynamic-caching-talk.md)).
- Consequence for almondbread: on M1/M2 a register-hungry floatexp or
  float-float path must not be compiled into the fp32 fast path; on M3+ the
  penalty is smaller but still worth avoiding (inference).

## Reference machine

- M1 Pro: 16 GPU cores (a 14-core bin also exists) at 1.296 GHz, about 5.3 TFLOPS
  fp32 (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Operations per Second).
  At 256 FMA/core-cycle one perturbation step (about 3 to 4 FMA-equivalents for a
  complex square plus add, plus a norm test) gives a rough ceiling of order
  1e12 iterations/s across the GPU, before divergence and memory (inference, unmeasured).

## Denormals, fast math

- Denormal handling on Apple GPUs is not documented in any fetched source; assume
  flush-to-zero, so the float32 floor is about 1.18e-38 (unverified; see [Do Apple GPUs flush float32 denormals to zero?](../questions/apple-gpu-denormals.md)).
- Double-float error-free transforms are fragile under compiler reassociation on
  Metal (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §Why This Is Fragile In WGSL And Metal);
  see [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md).

## Contradictions

- **FFMA32 latency.** The instruction table gives FFMA32 an adjusted latency of
  about 2.21 cycles (raw 3.50 to 3.91), while the ALU bottlenecks section says later
  benchmarks reduced absolute latency to 6 cycles for FFMA32 (3 for FFMA16, FADD32,
  FMUL32) and that the tables "reflect a sub-optimal shader setup"
  (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §ALU Bottlenecks, §Instruction Throughputs).
  The two figures measure different things (adjusted vs absolute), but the page
  does not reconcile them. Tracked in [What is the FFMA32 latency on Apple GPUs?](../questions/ffma32-latency.md).
- **Emulation strategy.** Turner argues integer-based fp64 emulation beats
  double-single (src: [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)), while the 1/8 integer
  multiply rate suggests fma-based float-float is faster for 48-bit mantissas
  (inference). Tracked in [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md).

## Gaps

- No microbenchmarks for M3/M4/M5 fp32 throughput per core or int32 multiply.
- No occupancy-versus-register-count table for Apple GPUs.

# Citations

- [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)
- [Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)](../sources/apple-dynamic-caching-talk.md)
- [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)
- [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md)
