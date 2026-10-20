---
title: "Double-float (float-float) arithmetic"
type: technique
status: draft
sources: ["sources/thall-extended-precision", "sources/luma-gl-gpu-precision", "sources/fractalshark-readme", "sources/metal-benchmarks-readme", "sources/apple-forums-fp64-emulation", "sources/apple-metal-compile-options", "sources/swift-mandelbrot-readme"]
tags: [numerics, double-float, df64, float32, metal]
---

# Double-float (float-float) arithmetic

A value is an unevaluated sum of two float32s, `hi + lo` with
`|lo| ≤ ulp(hi)/2`. It gives about 48 bits of significand with float32's
exponent range. It is a precision fix, not a range fix
(src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §How Classic Double-Single Arithmetic Works).
On Apple GPUs, which have no fp64, it is the cheap way to get more than 24
bits.

## Where almondbread would use it

- **Direct rendering of the shallow band** before perturbation pays off.
  A direct double-single renderer reaches about 1e13 zoom (notes:
  gpu_apple_silicon, luma.gl Mandelbrot example). swift-mandelbrot steps
  Float → double-float → perturbation automatically
  (src: [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)).
- **Repair or verification mode** if long-orbit float32 rounding shows as
  speckle near minibrots: deltas or the stored reference in float-float plus
  a floatexp exponent, like FractalShark's "2×32 + exponent" type, which the
  author says gives "substantially higher performance on consumer GPUs with
  only a modest loss of precision" (src: [FractalShark README](../sources/fractalshark-readme.md)).
  See [How large is float32 perturbation error over long orbits?](../questions/float32-error-long-orbits.md).
- Not for deltas at depth: they need range, see
  [Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md).

## Error-free transforms

From Dekker/Knuth as implemented for GPUs by Thall (src: [Thall: Extended-precision floating-point numbers for GPU computation](../sources/thall-extended-precision.md) §Algorithm 1 Fast-Two-Sum; §Algorithm 2 Two-Sum; §Algorithm 3 Split):

```text
quickTwoSum(a, b):            // requires |a| >= |b|
    s = a + b;  e = b - (s - a);  return (s, e)
twoSum(a, b):
    s = a + b;  v = s - a;  e = (a - (s - v)) + (b - v);  return (s, e)
twoProd(a, b):                // with hardware fma
    p = a * b;  e = fma(a, b, -p);  return (p, e)
split(a):                     // only without fma; c = 4097 = 2^12 + 1 for float32
    t = c * a;  hi = t - (t - a);  lo = a - hi
```

df64 operations built from them (our own sketch):

```text
add(x, y):  (s, e) = twoSum(x.hi, y.hi); (t, f) = twoSum(x.lo, y.lo)
            e += t; (s, e) = quickTwoSum(s, e); e += f; return quickTwoSum(s, e)
mul(x, y):  (p, e) = twoProd(x.hi, y.hi); e += x.hi*y.lo + x.lo*y.hi
            return quickTwoSum(p, e)
```

Rough costs (inference): add 11 to 20 fp32 ops, mul with fma 3 to 8; a
complex df square, the Mandelbrot hot path, 30 to 50 ops against 3 to 4 for
plain float32, so 10 to 20× slower. Emulated extended precision costs
"more than an order of magnitude" over fp32 (notes: gpu_apple_silicon,
luma.gl fp64 module). The `fma` in MSL maps to native full-rate FFMA32 on
Apple GPUs (src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)).

## Pitfalls

- **Compiler reassociation.** The transforms depend on exact rounding
  events; "a compiler that reassociates the expression can therefore erase
  the information". Parentheses, temporaries or `fma` spelling are not
  portable guarantees (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §Why This Is Fragile In WGSL And Metal).
  luma.gl switches to an integer-controlled double-single path on Apple
  adapters for this reason (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §luma.gl's Apple/Metal Path).
- **Metal math mode.** `MTLCompileOptions.mathMode` (`.safe`, `.relaxed`,
  `.fast`) replaces `fastMathEnabled`; whether the default permits
  reassociation or contraction is unverified
  (src: [Apple docs: MTLCompileOptions mathMode](../sources/apple-metal-compile-options.md)). Compile the float-float
  library with `.safe`, keep `.fast` for float32 kernels, and run a startup
  known-answer test (does `twoSum(1, 1e-8)` keep a non-zero low part?).
  See [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md).
- **Split overflow.** `4097·a` overflows for large finite inputs
  (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §How Classic Double-Single Arithmetic Works);
  use fma-based `twoProd` instead.
- **Tapering precision.** When `lo` becomes subnormal (results near 2^-126
  to 2^-102) precision tapers towards 24 bits
  (src: [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md) §How Classic Double-Single Arithmetic Works).
- **Register pressure.** Two registers per real, so a complex df kernel
  uses about twice the registers; on M1/M2 that lowers occupancy for the
  whole dispatch. Keep it in its own pipeline ([Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)).

## Contradictions

Philip Turner argued on Apple's forums that integer-based fp64 emulation
beats double-single on Apple GPUs (src: [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)),
while his own benchmarks show int32 multiply at 1/8 of fp32 FMA rate
(src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)), which favours fma-based
float-float for speed (inference). luma.gl picks the integer path on Apple
for correctness, not speed. Unresolved: [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md).

## For almondbread

Roadmap step 10 (M7): an optional float-float (+ exponent) pipeline under
`.safe` math with a startup known-answer test, used for the shallow direct
band and as a repair/verification mode. Measure its speed on M1 Pro first.

## As built (M7)

M7 first checked whether there is anything to repair. There is not: at 23
views from 3e-7 to 4e-1003 per pixel (minibrots, spirals, Feigenbaum, the
period-32412 spiral, 1e6-iteration orbits) the float32 GPU field differs from
the CPU oracle by less than the oracle differs from itself with its pixel grid
moved by 0.001 px at 17 views and 0.01 px at 5 (0.1–0.2 px at the catalogue
spiral at 1e1000), and no crop shows speckle (measured 2026-10-15, M1 Pro, release;
[How large is float32 perturbation error over long orbits?](../questions/float32-error-long-orbits.md) "M7", [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M7).
So float-float is built as scaffolding with no runtime cost, not as a repair
pass:

- **Own library** `crates/almondbread-render/src/shaders/floatfloat.metal`,
  compiled through the backend hook `GpuBackend::library_pipelines(source,
  functions, MathMode::Safe)` (Metal: `MTLCompileOptions.mathMode = .safe`
  where the OS has it, macOS 15 / iOS 18, else `fastMathEnabled = false`).
  The fast float32 library is untouched (fields bit-identical, `bench` geomean
  1.014 under load), and the float-float library compiles only when a
  `floatfloat::FloatFloat` is created.
- **Known-answer test**, run by `FloatFloat::new` before it hands out
  pipelines; inputs come from a buffer so nothing constant-folds. Under
  `.safe` every check passes; the same source under `.fast` loses the low
  parts of TwoSum and QuickTwoSum (0 instead of 1e-8 and 2^-30) and a
  1000-step float-float orbit is only float32-accurate (4.5e-8). TwoProd
  through an explicit `fma` survives fast math. On failure `new` returns the
  report as an error and callers stay on float32 (the fallback). Settles
  [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md) for `.fast`.
- **fx type**: a complex float-float mantissa with one `int` exponent,
  `(re + i im) 2^e`, renormalised after every operation, so one kernel
  (`ffIterate`) covers every depth (`fx a^2 + a` at 2^-1000 is exact). It
  iterates a *list* of pixels (the hook for a repair pass of flagged pixels)
  against a float-float + exponent reference with rebasing, in budgeted
  command buffers (~50 ms each). Used by `verify --ff 1` and
  `precision --ff 1`: against the f64 oracle it differs on 0.001–0.3 % of
  escape counts where float32 differs on 3–18 % (6.3 % against 52.8 % at the
  chaotic period-32412 spiral, where 48 and 53 bits still disagree).
- **Speed** (z² + c, M1 Pro, under load): float-float 2.58e10 it/s, 7–12×
  slower than float32 (better than the 10–20× guessed above); fx 3.4–6.6e9,
  27–52×. A whole fx field costs 10–300× the float32 renderer, which also
  skips iterations (BLA, local SA): a repair pass would have to stay on a few
  flagged pixels ([Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)).
- **Not built** (no artefacts to remove): the repair pass and its flagging
  (Pauldelbrot test, AA-sample or neighbour disagreement, precision-loss
  estimate), the "max precision" setting, the shallow direct float-float band.

# Citations

- [Thall: Extended-precision floating-point numbers for GPU computation](../sources/thall-extended-precision.md)
- [luma.gl: GPU floating-point precision techniques](../sources/luma-gl-gpu-precision.md)
- [FractalShark README](../sources/fractalshark-readme.md)
- [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md)
- [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](../sources/apple-forums-fp64-emulation.md)
- [Apple docs: MTLCompileOptions mathMode](../sources/apple-metal-compile-options.md)
- [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)
