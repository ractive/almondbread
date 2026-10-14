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
  128); fp32 add 128 per core-cycle (src: [[sources/metal-benchmarks-readme]] §Operations per Second).
- **fp16 FMA is also 256 per core-cycle**, so fp16 brings no ALU throughput gain
  on M1, only register and bandwidth savings (src: [[sources/metal-benchmarks-readme]] §Operations per Second).
  It is useless for perturbation deltas (range 6e-5 to 6.5e4, 11-bit
  mantissa) but fine for colour outputs (inference).
- **No fp64.** Emulated IEEE-like "FP59" (e11m48) costs about 1:36 for add, 1:52
  for mul and 1:68 for FMA versus fp32; a non-IEEE e8m48 format could roughly
  triple that (src: [[sources/metal-benchmarks-readme]] §Operations per Second).
- **Integer multiply is slow.** IMUL32 has a throughput of 4 cycles, i.e. 32 per
  core-cycle against 256 fp32 FMAs (1/8); IMUL(32x32=64) and IMULHI32 take about 8
  cycles; 64-bit integer add is emulated (src: [[sources/metal-benchmarks-readme]] §Instruction Throughputs, §ALU Layout).
  Consequence: a GPU bignum built from u32 limbs is slow, and float-float beats
  integer-limb arithmetic in the delta loop unless correctness forces the integer
  path (inference; see [[questions/double-float-speed-on-m1]]).
- `fma()` in MSL maps to a native full-rate FFMA32 (src: [[sources/metal-benchmarks-readme]] §Instruction Throughputs),
  which is what TwoProd in [[techniques/double-float]] needs.

## Execution model and occupancy

- SIMD-group width is 32 threads. Each core has four schedulers, each issuing one
  instruction from one SIMD-group per cycle (src: [[sources/metal-benchmarks-readme]] §ALU Layout).
- Per core: register file about 208 KB, threadgroup ("shared") memory about 60 KB,
  8 KB L1 data cache, 12 KB instruction cache, 384 to 3072 resident threads
  depending on register use; global cache line 128 B (src: [[sources/metal-benchmarks-readme]] §On-Chip Memory).
- "ALU utilization maxes out at 24 simds/core". The compiler prefers to spill
  registers to device memory rather than drop occupancy below that (src: [[sources/metal-benchmarks-readme]] §Overview).
- Divergence: a SIMD-group of 32 pixels runs until its slowest pixel finishes.
  Escape-time kernels are badly divergent near the boundary, which is why
  [[platform/gpu-watchdog-and-chunking]] compacts live pixels between chunks (inference).
- The 12 KB instruction cache is small; one fat kernel with every feature
  compiled in can thrash it (src: [[sources/metal-benchmarks-readme]] §On-Chip Memory shows performance vs executable size)
  — another reason for specialised pipelines in [[platform/metal-compute]].

## Dynamic caching (M3 and later)

- Before M3, a shader's worst-case register allocation set occupancy for the
  whole dispatch. M3 (A17 Pro) allocates registers from on-chip storage by live
  use, and registers, threadgroup and tile memory share one flexible pool; Apple
  says no app changes are needed (src: [[sources/apple-dynamic-caching-talk]]).
- The M5-generation talk describes a second-generation dynamic caching with lower
  register latency and an occupancy management unit, and warns that too many
  SIMD-groups can exceed on-chip storage and cause spills (src: [[sources/apple-dynamic-caching-talk]]).
- Consequence for almondbread: on M1/M2 a register-hungry floatexp or
  float-float path must not be compiled into the fp32 fast path; on M3+ the
  penalty is smaller but still worth avoiding (inference).

## Reference machine

- M1 Pro: 16 GPU cores (a 14-core bin also exists) at 1.296 GHz, about 5.3 TFLOPS
  fp32 (src: [[sources/metal-benchmarks-readme]] §Operations per Second).
  At 256 FMA/core-cycle one perturbation step (about 3 to 4 FMA-equivalents for a
  complex square plus add, plus a norm test) gives a rough ceiling of order
  1e12 iterations/s across the GPU, before divergence and memory (inference, unmeasured).

## Denormals, fast math

- Denormal handling on Apple GPUs is not documented in any fetched source; assume
  flush-to-zero, so the float32 floor is about 1.18e-38 (unverified; see [[questions/apple-gpu-denormals]]).
- Double-float error-free transforms are fragile under compiler reassociation on
  Metal (src: [[sources/luma-gl-gpu-precision]] §Why This Is Fragile In WGSL And Metal);
  see [[questions/metal-fast-math-defaults]].

## Contradictions

- **FFMA32 latency.** The instruction table gives FFMA32 an adjusted latency of
  about 2.21 cycles (raw 3.50 to 3.91), while the ALU bottlenecks section says later
  benchmarks reduced absolute latency to 6 cycles for FFMA32 (3 for FFMA16, FADD32,
  FMUL32) and that the tables "reflect a sub-optimal shader setup"
  (src: [[sources/metal-benchmarks-readme]] §ALU Bottlenecks, §Instruction Throughputs).
  The two figures measure different things (adjusted vs absolute), but the page
  does not reconcile them. Tracked in [[questions/ffma32-latency]].
- **Emulation strategy.** Turner argues integer-based fp64 emulation beats
  double-single (src: [[sources/apple-forums-fp64-emulation]]), while the 1/8 integer
  multiply rate suggests fma-based float-float is faster for 48-bit mantissas
  (inference). Tracked in [[questions/double-float-speed-on-m1]].

## Gaps

- No microbenchmarks for M3/M4/M5 fp32 throughput per core or int32 multiply.
- No occupancy-versus-register-count table for Apple GPUs.
