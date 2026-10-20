---
title: "Which extended-precision emulation is fastest and correct on Apple GPUs?"
type: question
status: answered
sources: ["sources/apple-forums-fp64-emulation", "sources/metal-benchmarks-readme", "sources/luma-gl-gpu-precision", "sources/thall-extended-precision", "sources/fractalshark-readme"]
tags: [double-float, apple-gpu, benchmark, precision]
---

# Which extended-precision emulation is fastest and correct on Apple GPUs?

## Question

On an M1 Pro, what do fma-based float-float (double-single), integer-controlled
double-single and integer-limb software floats cost per complex square and add,
and which stay numerically correct under Metal's compiler?

## Why it matters

Float-float is almondbread's repair and verification mode, and the type for the
shallow band before perturbation pays off ([Double-float (float-float) arithmetic](../techniques/double-float.md)). Choosing
the wrong one costs either speed or silent wrong answers.

## What sources say

- Native fp32 FMA is full rate and maps from `fma()`; int32 multiply is 1/8 rate;
  emulated fp64 costs 1:36 (add) to 1:68 (FMA)
  (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §Operations per Second, §Instruction Throughputs).
- Thall's df64 algorithms (TwoSum, Split, TwoProduct) are the classic
  pairs-of-floats arithmetic for GPUs (src: [Thall: Extended-precision floating-point numbers for GPU computation](../sources/thall-extended-precision.md)).
- FractalShark's 2x32 + exponent type is its fast consumer-GPU type
  (src: [FractalShark README](../sources/fractalshark-readme.md) §5. Custom “2×32 + exponent” numeric type).
- luma.gl switches to integer-controlled double-single on Apple WebGPU adapters
  because the compiler may reassociate or contract, trading throughput for reliable
  rounding (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §luma.gl's Apple/Metal Path).

## Contradictions

- Philip Turner argues integer-based fp64 emulation beats double-single on Apple
  GPUs (src: [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)). His own benchmarks show
  integer multiply at 1/8 of fp32 FMA rate (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)),
  which suggests fma-based float-float should be faster for a 48-bit mantissa
  (inference). The two claims may be about different targets (full IEEE fp64 with
  53 bits and 11-bit exponent vs a 48-bit expansion with fp32 range).
- luma.gl's choice of the integer path is about correctness under WGSL rules, not
  speed; native Metal with `.safe` math may keep the classic path valid
  (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §Why This Is Fragile In WGSL And Metal).

## How to resolve

Microbenchmark on M1 Pro (and one M3+ device): a kernel doing N iterations of
z = z^2 + c in each representation, compiled with `.fast` and `.safe`; measure
iterations/s and check a known-answer residual (TwoSum(1, 1e-8) low limb nonzero,
and a 1000-step orbit compared with CPU double). Related:
[What does Metal's default math mode allow for runtime-compiled kernels?](metal-fast-math-defaults.md), [Apple GPU architecture for compute](../platform/apple-gpu-architecture.md).

## Answer (M7, measured)

Measured 2026-10-15, M1 Pro (16-core GPU), release, `almondbread precision --kat 1`
(kernels `ffBench*` in `crates/almondbread-render/src/shaders/floatfloat.metal`), z = z² + c,
262 144 threads × 2000 steps, medians of 5–7, three runs while other agents loaded the GPU;
all three kernels in the same `.safe` library ([almondbread benchmarks and verification record](../synthesis/benchmarks.md) M7):

| type | iterations/s | vs float32 |
|---|---:|---:|
| float32 | 1.8–3.0e11 | 1 |
| fma-based float-float (Thall's TwoSum/TwoProd) | 2.58e10 | 7–12× slower |
| fx: float-float + int exponent, renormalised after each op | 3.4–6.6e9 | 27–52× slower |

- **Correct under `.safe`, broken under `.fast`.** The known-answer test passes every
  check in `.safe` (TwoSum(1, 1e-8) keeps 1e-8, a 1000-step float-float orbit is within
  9.4e-16 of f64). The same source in `.fast` returns 0 for the TwoSum and QuickTwoSum low
  parts and a float32-accurate orbit (4.5e-8): the compiler reassociates the transforms
  away, as luma.gl feared (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §Why This Is Fragile In WGSL And Metal).
  TwoProd via explicit `fma` survives both. So the classic fma path is valid on native
  Metal with `.safe` and a known-answer test; luma.gl's integer workaround is not needed here.
- **Not measured:** integer-controlled double-single and integer-limb software floats
  (Turner's suggestion). With fma float-float at 7–12× and int32 multiply at 1/8 rate
  (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)), an integer path is unlikely to win for a
  48-bit mantissa (inference), and correctness no longer argues for it. An M3+ device was
  not available.
- fx's extra cost is the renormalisation (`frexp` + two `ldexp` per complex value per
  operation); a kernel that rescales only when the exponent drifts, like the float32 deep
  kernel, would sit closer to plain float-float (inference, not built: M7 found no
  artefacts to repair, [How large is float32 perturbation error over long orbits?](float32-error-long-orbits.md)).

# Citations

- [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)
- [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)
- [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md)
- [Thall: Extended-precision floating-point numbers for GPU computation](../sources/thall-extended-precision.md)
- [FractalShark README](../sources/fractalshark-readme.md)
