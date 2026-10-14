---
title: "Arbitrary precision on Apple Silicon (GMP, MPFR)"
type: platform
status: draft
sources: ["[[sources/gmp-m1-discuss]]", "[[sources/gmp-fixed-size-mul]]", "[[sources/fractalshark-readme]]", "[[sources/fractalforums-interior-detection]]", "[[sources/kf-manual]]", "[[sources/metal-benchmarks-readme]]"]
tags: [cpu, gmp, mpfr, reference-orbit, multithreading, apple-silicon]
---

# Arbitrary precision on Apple Silicon (GMP, MPFR)

The reference orbit ([[techniques/reference-orbit-precision]]) is computed on the
CPU in arbitrary precision. This page covers what the Apple CPU does well and how
to parallelise one inherently serial orbit.

## GMP on M1

- The GMP developers found "almost every inner loop runs at < 1 cycle/limb" on a
  3.2 GHz M1 Mac mini, including `mpn_mul_1`, before any M1-specific tuning; the
  M1 took #2 on the GMPbench list behind a 4.4 GHz Ryzen (src: [[sources/gmp-m1-discuss]]).
- Weak spot: `mpn_addmul_1`, the most important loop. ARM64 has one carry flag
  (x86 has two via adcx/adox) and no high-word multiply-add instruction, so the
  three-way accumulation must save and restore carries (src: [[sources/gmp-m1-discuss]]).
- Hand-written fixed-size multiplication kernels `mpn_mul_N` for N = 1..15 limbs
  run 80% faster than `mpn_mul_basecase` at N = 8 on M1, and move the Toom22/Toom33
  crossover to about 480 limbs (src: [[sources/gmp-fixed-size-mul]]). Single-author
  result on one machine.
- MPFR sits on GMP's mpn layer, so these numbers carry over roughly (inference; no
  MPFR-specific M1 benchmark was found). MPFR limits precision to about 2 gigabits
  (src: [[sources/kf-manual]]).

## Sizes involved

- Precision needed grows with depth: about log2(magnification) + log2(width) +
  guard bits, e.g. about 380 bits at 1e100 (inference; see [[questions/precision-bits-formula]]).
  That is 6 limbs at 1e100, about 55 limbs at 1e1000, about 520 limbs at 1e10000.
  Most practical zooms sit in basecase or low Toom sizes, exactly where the
  fixed-size kernels win (inference).

## Parallelising one iteration: the MT3 pattern

The orbit is serial, but one iteration has independent products:

```text
// Z = X + iY, C = Cx + iCy, all arbitrary precision
// three independent products, one per thread:
P1 = (X + Y) * (X - Y)      // thread A
P2 = X * Y                  // thread B
// coordinator: X' = P1 + Cx ; Y' = 2*P2 + Cy ; also round X', Y' to float for storage
```

- FractalShark's "MT3" uses three threads, two squaring and one coordinating, and
  was fastest among the CPU paths its author tried, on an AMD 5950X (MPIR+AVX2 on
  Windows, GMP on Linux); "MT5" currently falls back to MT3
  (src: [[sources/fractalshark-readme]] §4. Multithreaded high-performance CPU reference orbit calculation).
- On an M1 Pro, run these on performance cores. Per-iteration thread handoff costs
  microseconds, so the split only pays when one product costs more than that,
  roughly from a few hundred limbs up (inference, unmeasured). Below that, a
  single-threaded fixed-limb implementation is likely faster.
- Other parallelism that is free: one reference per critical point or hybrid phase
  can run on separate cores ([[techniques/rebasing]]), and Newton refinement of the
  nucleus ([[techniques/reference-selection]]) is a separate job.

## Custom fixed point vs MPFR

- The orbit stays bounded by |Z| <= 2 before escape, so a fixed-point format with
  a constant binary point and no exponent handling is sufficient for Mandelbrot
  (inference). It avoids MPFR's allocation and exponent work, in the spirit of the
  fixed-size kernel result.
- Software float on CPU can beat semi-software floatexp: mathr measured his
  Fraktaler 3 number types (higher is faster): float 270M, double 256M, long
  double 143M, softfloat (uint32 + uint32) 8.2M, floatexp (float + int) 4.3M; and
  wrote that uint32 + uint32 software float was about 2x faster than float32 + int32
  for basic arithmetic (src: [[sources/fractalforums-interior-detection]]). This is
  about the per-pixel delta type on a CPU, not the reference, but it warns against
  assuming floatexp is the cheapest extended-range type everywhere.

## No Metal bignum

- FractalShark's GPU NTT reference orbit beats multithreaded MPIR only beyond about
  1e10000 on an RTX 4090 (src: [[sources/fractalshark-readme]] §1. GPU-accelerated reference orbit computation).
  Apple GPUs multiply integers at 1/8 the fp32 FMA rate
  (src: [[sources/metal-benchmarks-readme]] §Instruction Throughputs). Keep the
  reference on the CPU (inference).

## NEON and AMX

- NEON has no 64x64->128 multiply; scalar `mul`/`umulh` (what GMP uses) is the
  right path for limbs. AMX/SME do not help carry chains (inference, background knowledge).
- A NEON CPU fallback for pixels would process 4 fp32 or 2 fp64 lanes; on the CPU,
  fp64 deltas are natural (inference).

## AlmondbreadMath: own arithmetic vs GMP (measured)

Our implementation per [[decisions/arbitrary-precision]] (`Sources/AlmondbreadMath`):
floating point with UInt64 limbs, Int exponent, correctly rounded (nearest-even) add,
sub, mul, square, div; reference orbit on preallocated registers, loop specialised for
1...16 limbs. Reference-orbit iterations per second, one P-core, release, our build vs
GMP 6.3 `mpf` doing the same iteration (`mpf` keeps prec+1 limbs and truncates)
(measured 2026-10-14, M1 Pro, release; machine shared with a parallel build, best of 2):

| bits | ours | GMP mpf | ratio | GMP mpn floor |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 16.2 M | 10.5 M | 1.55 | 66 M |
| 128 | 13.6 M | 9.8 M | 1.39 | 37 M |
| 256 | 11.6 M | 8.4 M | 1.39 | 25 M |
| 512 | 7.0 M | 5.2 M | 1.35 | 11 M |
| 1024 | 2.65 M | 2.44 M | 1.08 | 3.8 M |
| 2048 | 0.97 M | 0.97 M | 1.0 | 1.2 M |
| 4096 | 0.30 M | 0.37 M | 0.81 | 0.37 M |
| 16384 | 23 k | 41 k | 0.56 | 40 k |

"mpn floor" is 3 `mpn_sqr` + 6 `mpn_add_n` per iteration with no rounding, signs or
exponents: an unreachable lower bound for any correct orbit. Ours includes writing the
float32 and floatexp copy of every point.

What made the difference (all measured on the same machine):

- **Short products with exact rounding.** Only columns >= n-2 of the product are
  computed; the skipped columns add less than n units to limb n-1, so the rounding
  decision is certain unless the bits below the round bit are within n of a boundary
  (probability ~n/2^62), in which case the full product is computed. Results stay
  bit-identical to a correctly rounded full product. Saves ~40 % at 16 limbs.
- **Two independent accumulators in the Comba column loop** (lows and highs summed
  separately): 16-limb product 305 ns -> 190 ns versus one three-word accumulator.
- **Loop exits are the basecase cost.** The column loop's variable trip count costs a
  branch mispredict per column; fully unrolled straight-line kernels (generated, 3...8
  limbs) cut an 8-limb short square from 29 to 16 ns. At 16 limbs unrolling lost
  (register spills), so loops stay above 8 limbs.
- **Karatsuba loses to the short Comba product up to at least 256 limbs** in the orbit:
  at 64 limbs short Comba 0.29 M it/s against 0.18 M with full Karatsuba products. Our
  Karatsuba only serves full products (fallback, Newton's BigFloat ops).
- Two multiplications per iteration (`(x+y)(x-y)`, `xy`) beat three squarings up to 16
  limbs, because the squaring form needs six additions instead of four; above 16 limbs
  squarings win (2048 bits: 0.98 M vs 0.93 M).
- GMP's hand-scheduled mpn loops reach ~1.5 cycles per limb product at 16 limbs; our
  Swift loops ~2.5. GMP keeps the edge from ~4096 bits (Toom, asm). Closing it would
  need a short-product Karatsuba (Mulders) or MT3 threading
  ([[techniques/reference-orbit-precision]]).

BigFloat operators that allocate their result: mul 98 ns and add 102 ns at 128 bits,
1.8 µs mul at 4096 bits; division 0.6 µs (128 bits) to 8 µs (4096 bits); decimal
formatting and parsing 13 µs (128 bits) to 160 µs (4096 bits) (measured 2026-10-14,
M1 Pro, release). The orbit, period and Newton loops never use them.

## Gaps

- No MPFR M1 benchmark; no published M1 Pro reference-orbit timing for any renderer.
