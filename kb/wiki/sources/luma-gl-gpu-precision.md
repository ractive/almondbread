---
title: "luma.gl: GPU floating-point precision techniques"
type: source
authors: ["luma.gl contributors"]
year: 2026
raw: "web/luma-gl-gpu-precision.html"
status: digested
tags: [double-float, fp64-emulation, webgpu, metal, error-free-transforms, fixed-point]
---

# luma.gl: GPU floating-point precision techniques

**URL:** <https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision> · **Saved:** `raw/web/luma-gl-gpu-precision.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

A guide page in the luma.gl (vis.gl, WebGL/WebGPU) docs on ways to get more than f32 precision on GPUs: double-single (float-float), integer-assisted double-single, exact binary64 deltas, software binary64, fixed point and native f64. It explains why classic double-single is fragile under WGSL and Metal compilers and why luma.gl switches to an integer path on Apple WebGPU adapters. It embeds a live Mandelbrot f32 vs double-single comparison and benchmark (not captured in the saved copy).

## What it covers

- Precision table: f32 = 24 bits (~7 decimal digits); binary64 = 53 bits (~15–16 digits); double-single = up to ~48 bits (~14 digits) with essentially the f32 exponent range.
- A double-single value is an expansion of two non-overlapping f32s; it adds significand bits, not exponent range, and does not reproduce binary64 rounding, subnormals, infinities or NaN.
- Storage is not arithmetic: two 32-bit words can hold binary64 bits exactly, but every operation still needs an implementation. luma.gl keeps two forms: `fp64f32` (hi + lo `vec2f`) and `fp64u32` (raw binary64 words).
- Error-free transforms: TwoSum (Møller–Knuth) `s = fl(a+b); v = fl(s−a); e = fl(fl(a − fl(s−v)) + fl(b−v))`; FastTwoSum (Dekker) `s = fl(a+b); e = fl(b − fl(s−a))`, needs `|a| ≥ |b|`.
- Dekker split for f32 without FMA: `c = fl(4097·a)` (4097 = 2^12 + 1), `aHigh = fl(c − fl(c − a))`, `aLow = fl(a − aHigh)`. The `4097·a` step can overflow for large finite `a`. With a true FMA the product residual is `fma(a, b, −fl(a·b))`.
- Exactness needs round-to-nearest, no reassociation, no unintended contraction, no overflow, and care with tiny values. Parentheses alone do not guarantee this.
- WGSL permits reassociation and fusion and does not fix one rounding mode; its `fma` lacks a portable correctly rounded guarantee. Source tricks are not portable barriers: storing to `let`/`var`, adding a runtime zero, multiplying by a runtime one, helper functions, parentheses, identity bitcasts, writing the residual with `fma`.
- Apple path: on Apple WebGPU adapters `fp64arithmetic` decodes each f32 limb into sign/exponent/significand and does TwoSum, TwoProd and renormalisation in `u32` with explicit round-to-nearest-even packing. A 24x24-bit product needs only 48 exact bits. Selected automatically; override with `LUMA_FP64_INTEGER_ARITHMETIC`.
- Apple path range: ~48 bits while both limbs are normal; for results between about 2^-126 and 2^-102 the low limb is subnormal and precision tapers from ~48 toward 24 bits. Division and square root use an f32 estimate plus integer-controlled corrections.
- Cost ranking (qualitative only): native f64 lowest when present; classic double-single lower than integer emulation; integer-assisted double-single higher; full software binary64 high (division, sqrt, transcendentals worst); fixed point very low for add, multiply needs wide intermediates.
- Exact-delta helper: `round_f32(exact(world − origin))` via integer subtraction of binary64 fields keeps low bits that converting both inputs to f32 first would lose. Double rounding (`round_f32(round_f64(a−b))`) can differ from this in rare cases. It does not help an iteration that must keep extra bits every step.
- Fixed point: integer `n` read as `n·2^-k`; add/sub exact until overflow; multiply needs a wide intermediate and a rounding shift. Named as a good fit for deep-zoom fractals over a bounded domain.
- Workload table: deep iterative calculation over a bounded interval → fixed point or integer-assisted double-single.
- Validation advice: compare bits against an independent correctly rounded oracle; test cancellation, ties, exponent gaps, signed zero, subnormal boundary, overflow; run on real hardware, especially Metal; check that the low limb is actually non-zero (an expansion can silently collapse to f32); benchmark the real shader since instruction counts miss register pressure and occupancy.
- The GLSL-only `fp64` module has exp/log/trig; WGSL gets only add, sub, mul, div, sqrt from `fp64arithmetic`.

## Reliability

- Secondary and recent (manifest year 2026, retrieved 2026-10-14). Careful and well argued; the claim that compilers on Apple/Metal may break classic double-single is stated as a WGSL-spec issue plus their experience, without a named failing case.
- No numeric timings in the saved page; the benchmark is interactive and not captured.
- Its statements concern WebGPU/WGSL. For native Metal, whether `fastMathEnabled`/`mathMode` settings break error-free transforms is a separate question (see [Apple docs: MTLCompileOptions mathMode](apple-metal-compile-options.md)).

## Useful sections

- §Start With The Required Contract — precision/range table, the three questions.
- §Storage Is Not Arithmetic — `fp64f32` vs `fp64u32`.
- §How Classic Double-Single Arithmetic Works — TwoSum, FastTwoSum, split, FMA residual, preconditions.
- §Why This Is Fragile In WGSL And Metal — reassociation, fusion, failed source-level barriers.
- §luma.gl's Apple/Metal Path — integer-controlled double-single, subnormal taper.
- §The Main Alternatives — cost/portability table; §Origin Rebasing And An Exact Delta; §Integer-Assisted Double-Single; §Full Software Binary64; §Fixed Point; §Native Binary64.
- §Choosing By Workload — table including deep iterative calculation.
- §Validation Is Part Of The Technique — test checklist.

## Used by

- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)
- [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md)
- [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)
- [Do Apple GPUs flush float32 denormals to zero?](../questions/apple-gpu-denormals.md)

# Citations

- <https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision>
- [Apple docs: MTLCompileOptions mathMode](apple-metal-compile-options.md)
