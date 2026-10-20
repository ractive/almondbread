---
title: "Thall: Extended-precision floating-point numbers for GPU computation"
type: source
authors: ["Andrew Thall"]
year: 2007
raw: "papers/thall-extended-precision.pdf"
status: digested
tags: [double-float, df64, qf128, error-free-transforms, gpu, fma]
---

# Thall: Extended-precision floating-point numbers for GPU computation

**URL:** <https://andrewthall.org/papers/df64_qf128.pdf> · **Saved:** `raw/papers/thall-extended-precision.pdf` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Andrew Thall's (Alma College) technical report CIM-007-01, dated March 2007, expanding a SIGGRAPH 2006 poster. It defines GPU "double-float" (df64, two f32s) and "quad-float" (qf128, four f32s) types with Cg shader code for arithmetic and transcendentals. It reports df64 accuracy on an Nvidia G80 and outlines a df64 FFT. A July 2009 addendum notes that native GPU doubles reduce the need, and the FFT timing tables were deleted.

## What it covers

- Representation: df64 `A = a_hi + a_lo`, two non-overlapping f32s. That gives ~48 bits of significand (2 × 24) at the f32 exponent range. qf128 is four terms, ~96 bits. Thall warns that "48-bit float" is misleading: values like `2^60 + 2^-60` are exactly representable.
- Error-free building blocks (after Dekker, Knuth, Shewchuk, Hida et al.):
  - quickTwoSum (needs `|a| ≥ |b|`): `s = a⊕b`, `e = b ⊖ (s ⊖ a)`. 3 flops.
  - twoSum: `s = a⊕b`, `v = s⊖a`, `e = (a ⊖ (s⊖v)) ⊕ (b ⊖ v)`. 6 flops.
  - split (Dekker/Veltkamp) for f32: `t = 4097·a` (4097 = 2^12 + 1), `hi = t ⊖ (t ⊖ a)`, `lo = a ⊖ hi`. 4 flops.
  - twoProd without FMA: `p = a⊗b`, split both, `err = ((a_hi b_hi − p) + a_hi b_lo + a_lo b_hi) + a_lo b_lo`. About 17 flops (inference from the listed steps).
  - twoProd with FMA: `p = a⊗b`, `err = fma(a, b, −p)`. 2 ops.
- df64 add: twoSum on the high parts and on the low parts, fold, quickTwoSum, add, quickTwoSum. About 20 flops (inference). It satisfies the IEEE-style bound `a ⊕ b = (1+δ)(a+b)` (Briggs, Kahan).
- df64 multiply: `p = twoProd(a_hi, b_hi)`, `p_lo += a_hi·b_lo + a_lo·b_hi`, then quickTwoSum. About 24 flops without FMA; with an FMA twoProd about 9, or 7 if the cross terms are fused too (inference). The `a_lo·b_lo` term is dropped. Squaring has a cheaper special case; scaling by powers of two acts on each part separately.
- Vector GPUs of the time ran two twoSums or two splits at once in `float2`/`float4`, a 2x saving, e.g. adding complex numbers or both df64 halves together.
- FMA caveat: Thall did not use the FMA twoProd. On G70 with Cg 1.5 the compiler optimised `a*b − x` so that the error term came out wrong (0). His library uses the split version to be safe.
- Division and square root: Newton–Raphson seeded by the f32 result, using Karp–Markstein. Division needs one f32 reciprocal, one df64 x f32 multiply, one df64 subtract and a few f32 operations. Square root uses `rsqrt` as the seed in the same pattern.
- Comparisons: compare high parts first, low parts on a tie.
- exp: Taylor series after range reduction (`|x| < ln2/128`); ln: one Newton step on exp from an f32 seed; sin/cos: Taylor series, cos from `sqrt(1 − sin²)`. Newton-based methods hold up better than series on inexact hardware.
- qf128: renormalisation of five terms into four, n-input sums (threeSum etc.); add bound `(1+δ1)a + (1+δ2)b` with `|δ| ≤ ε_qf128`; an IEEE-style bound costs 2–3.5x (Hida). The multiply uses 10 twoProds plus 3 plain products.
- Hardware: GPU Paranoia results show NV35/NV40/G70 rounding was not IEEE (e.g. multiply error up to −0.989 ulp, division up to −2.87 ulp); G80 (8800) adds, subtracts and multiplies within ±0.5 ulp (exact rounding). Error-free transforms need correct rounding; the 6800 series was the minimum, and accurate log/sin/cos needed the 8800.
- df64 accuracy on G80 (Table II, max |error| / RMS in ulps of a 48-bit result, vs CPU double): add 1.1/0.12, subtract 1.1/0.12, multiply 2.5/0.33, divide 4.1/0.48, reciprocal 3.1/0.40, rsqrt 4.4/0.55, sqrt 4.5/0.46, exp 10.6/1.7, ln(1+x) 11.0/1.7, sin 7.8/0.94, cos 241.3/6.0.
- A Mandelbrot set example ran under Cg 1.5 with df64 add/mult/div/sqrt (mentioned only).
- Case study: a transposed Stockham FFT in df64 with precomputed twiddle factors. The timing results were removed in 2009.

## Reliability

- Primary and widely cited, but old (Cg, 2006–2007 Nvidia hardware). The accuracy figures depend on G80 rounding; Apple GPU rounding must be checked separately ([Do Apple GPUs flush float32 denormals to zero?](../questions/apple-gpu-denormals.md)).
- The paper gives no per-operation costs or GPU timings for df64 add/mul (and the FFT timings were deleted). The flop counts above are our own tally from its algorithms.
- Draft marks remain ("[xxAT …]" placeholders, "Major hassles" for modulo). The cos error (241 ulps) comes from the `sqrt(1 − sin²)` shortcut.
- Year: the manifest says 2006 (the poster). The saved document is the March 2007 report with a 2009 addendum.
- The FMA warning still applies: a compiler that contracts or reassociates can zero the error terms. See [luma.gl: GPU floating-point precision techniques](luma-gl-gpu-precision.md) and [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md).

## Useful sections

- §II-A Extended-Precision Computation — Fast-Two-Sum, Two-Sum, Split, Two-Product (Algorithms 1–4) and theorems.
- §II-C Numerical Capabilites and Limitations of GPUs: GPU Paranoia — Table I, rounding behaviour per GPU.
- §II-D FMAD or Just Angry? — FMA twoProd and the compiler hazard.
- §III-A GPU Implementation of df64 Numbers — conversion, add, split/twoProd/mult, Karp division and sqrt (Algorithms 6–7), comparisons, transcendentals.
- §III-B GPU Implementation of qf128 Numbers — renormalise, add, multiply.
- §III-C Compilation Issues vis-a-vis the Cg Language — what needed Cg 2.0 / G80.
- Table II — df64 accuracy per operation.

## Used by

- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)
- [What does Metal's default math mode allow for runtime-compiled kernels?](../questions/metal-fast-math-defaults.md)
- [How large is float32 perturbation error over long orbits?](../questions/float32-error-long-orbits.md)

# Citations

- <https://andrewthall.org/papers/df64_qf128.pdf>
- [luma.gl: GPU floating-point precision techniques](luma-gl-gpu-precision.md)
