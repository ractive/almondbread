---
title: "Reference orbit: precision, cost, storage, compression"
type: technique
status: draft
sources: ["[[sources/kf-manual]]", "[[sources/fractalshark-readme]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/gmp-m1-discuss]]", "[[sources/gmp-fixed-size-mul]]", "[[sources/desarso-mandelbrot-webgpu-readme]]", "[[sources/fraktaler-3-site]]"]
tags: [reference, arbitrary-precision, mpfr, gmp, memory]
---

# Reference orbit: precision, cost, storage, compression

The reference `Z_{m+1} = Z_m^2 + C` is the only part computed in arbitrary
precision. It is inherently serial, so speed comes from fewer iterations,
faster squaring and less memory. CPU details for Apple Silicon are on
[[platform/mpfr-gmp-on-apple-silicon]].

## How many bits

No fetched source states the formula KF or Fraktaler uses
([[questions/precision-bits-formula]]). Rule of thumb (inference):

```text
bits ≈ log2(magnification) + log2(image width in pixels) + guard (32..64)
```

At zoom 1e100 that is about 332 + 11 + 32 ≈ 380 bits. The reference
coordinate must resolve the pixel spacing; the orbit stays within |Z| ≤ 2
before escape, so fixed point or MPFR with a near-constant exponent works.
Example from the webgpu renderer: `6e-42` needs 67 decimal digits, which
it holds in eight 32-bit limbs (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Measured and rejected).

Limits in KF: MPFR caps precision at 2 gigabits (~646 million digits); KF
caps periods at 2 giga-iterations (src: [[sources/kf-manual]] §Limits).

## Cost per iteration

One complex square plus add: `X' = X² − Y² + Cx`, `Y' = 2XY + Cy`, i.e. two
or three full-precision multiplications, e.g. `(X+Y)(X−Y)` and `XY`
(inference). At 350 to 35k bits these are basecase or Toom sizes.

- **Threading one iteration**: FractalShark's "MT3" uses three threads, two
  squaring and one coordinating; fastest the author measured on an AMD
  5950X (src: [[sources/fractalshark-readme]]).
- **GMP on M1**: "almost every inner loop runs at < 1 cycle/limb"; M1
  took second place on GMPbench in 2020 (src: [[sources/gmp-m1-discuss]]). Fixed-size
  kernels beat `mpn_mul_basecase` by up to 80% at 8 limbs on M1
  (src: [[sources/gmp-fixed-size-mul]]).
- **GPU reference orbit**: FractalShark's fused NTT CUDA kernel is about 10×
  faster than multithreaded MPIR at 16,384 32-bit limbs (~158k digits) on an
  RTX 4090, but the GPU only "may be faster" beyond about 1e10000; at
  shallower depths "the CPU is more likely to win" (src: [[sources/fractalshark-readme]]).
  The webgpu renderer runs its 8-limb reference in one workgroup and found
  NTT useless at hundreds of bits (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Measured and rejected).
- **Fewer iterations**: a nucleus reference needs only its period p
  ([[techniques/reference-selection]]); Fraktaler 3 notes locking maximum
  reference iterations to a known period can speed up high-iteration areas
  (src: [[sources/fraktaler-3-site]]).
- **Reuse**: KF's "reuse reference" keeps the orbit while zooming further
  (src: [[sources/kf-manual]] §kf-2.15.3). FractalShark's "perturbed
  perturbation" caches intermediate-resolution orbits across zooms
  (src: [[sources/fractalshark-readme]]).

## Storage for the GPU

- Store the orbit rounded to the delta type: float32 (8 B per complex) while
  |Z| has no range issue, which holds since |Z| ≤ 2 except at tiny
  values near 0 (inference). The rare tiny values matter for rescaled
  iterations, which need the full-range Z at those points
  (src: [[sources/mathr-deep-zoom-theory]] §Full Iterations), so keep a
  floatexp copy or flag for small |Z|.
- KF needs up to about 40 B per reference iteration depending on number type
  (src: [[sources/kf-manual]] §Limits). At 1e7 iterations, float32 storage
  is 80 MB; with floatexp copies and skip tables it can approach 1 GB
  ([[platform/ios-ipad-considerations]]).
- Rounding the reference to float32 adds a relative error of about 6e-8 per
  step to the delta update, harmless for colouring but compounding over very
  long orbits (inference; see [[questions/float32-error-long-orbits]]).

## Compression

- First implemented in Imagina for saving and loading; FractalShark extends
  it to decompress segments on demand while rendering. A period-600,000,000
  case saved several GB. Files are Imagina-compatible, with levels 1 to 20
  (src: [[sources/fractalshark-readme]]).
- Imagina's own save format (read in its source): re-iterate the orbit in
  double and store an exact waypoint whenever the relative drift exceeds a
  quality threshold `2^-q`; after the first iteration with `‖Z‖∞ < 2^-4`,
  regenerate the rest as a perturbation of the orbit's own beginning (with
  rebasing), storing waypoints on drift and a list of rebase iterations
  (src: [[sources/imagina-hinfla-evaluator]] §SaveCompressedOrbit).
  Details on [[questions/reference-orbit-compression]]. FractalShark's
  on-demand segment scheme (its PDF §14) is unread.
- The webgpu renderer rejected compression at its depths: 24 B per sample,
  200k iterations is 4.8 MB (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Measured and rejected).

## For almondbread

MPFR (or GMP mpn fixed point) on the P-cores with an MT3-style split, a
nucleus reference, double-buffered shared buffers so the next orbit and
table are built while the GPU renders, and reuse while the view stays
inside the reference's useful region. No Metal bignum: Apple's int32
multiply is 1/8 of fp32 FMA rate ([[platform/apple-gpu-architecture]]) and
GPU orbits only win past about 1e10000. Compression is an iOS-memory item
(roadmap step 12).

## In almondbread (milestone 3a, measured)

`ReferenceOrbitComputer` in `Sources/AlmondbreadMath` (Swift; since R2b only the Rust port
in `crates/almondbread-math` exists, [[decisions/rust-core]]; see
[[platform/mpfr-gmp-on-apple-silicon]] for the arithmetic and the GMP comparison):

- **Number format**: own binary floating point (UInt64 limbs, Int exponent),
  correctly rounded, not fixed point. The orbit alone would fit fixed point, but the same
  registers serve Newton (|dz| up to 4^p) and tiny |Z_n| near a nucleus keeps relative
  precision.
- **Formulation**: `X' = (X+Y)(X-Y) + Cx`, `Y' = 2XY + Cy` up to 1024 bits, three
  squarings above (faster there). Both are emulated bit-exactly by an independent
  Python oracle in the tests.
- **Precision rule as implemented**:
  `bits = ceil(-log2(spacing) + log2(image px) + log2(max(|C|,1)) + 1) + 48`, rounded up to
  64-bit limbs; e.g. 448 bits for 1e-100 spacing at 2048 px (inference; see
  [[questions/precision-bits-formula]]).
- **Error growth measured**: final Z against a 256-bit-more orbit after N iterations:
  within 1 to 3 bits of the precision on periodic or slowly varying orbits (192 to 4096
  bits, 119 to 2000 iterations), 24 bits lost after 476 iterations at a chaotic 64-bit
  point near -1.25, 6 bits lost after 3000 iterations at the seahorse point (128 bits)
  (measured 2026-10-14, M1 Pro, release). Supports a guard of 32 to 64 bits for typical
  orbits; long chaotic orbits need more, as the question page suspects.
- **Output**: each Z_n as `SIMD2<Float>` and as `FloatExpComplex` (two float
  mantissas, one shared Int32 exponent, larger part normalised to [0.5, 1), 16-byte
  stride), written into caller buffers, with cancellation polled and progress reported
  every 4096 iterations. The shared exponent follows [[techniques/extended-range-floats]]
  ("complex values can share one exponent"); Z_p of a nucleus orbit (|Z| ~ 1e-300 and
  below) survives in the floatexp copy while float32 flushes it to zero.
- **Speed**: 16 M iterations/s at 64 bits, 7 M at 512, 2.6 M at 1024, 0.3 M at 4096
  (measured 2026-10-14, M1 Pro, release, one thread). A 1e7-iteration orbit at 1e-100
  (448 bits) takes about 1.2 s; a nucleus reference needs only p iterations.
