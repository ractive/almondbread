---
title: "Extended range: floatexp, rescaled iterations, depth thresholds"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/kf-manual]]", "[[sources/muency-floatexp]]", "[[sources/desarso-mandelbrot-webgpu-readme]]", "[[sources/fraktaler-3-site]]", "[[sources/fractalshark-readme]]", "[[sources/imagina-hinfla-evaluator]]", "[[sources/fractalforums-interior-detection]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]"]
tags: [numerics, floatexp, rescaling, float32, extended-range]
---

# Extended range: floatexp, rescaled iterations, depth thresholds

For perturbation, the binding limit of float32 is range, not precision.
24 bits of mantissa are enough for deltas: "double precision ... is more
than enough for computing perturbed orbits: even single precision (with 24
bits) can be used successfully" (src: [[sources/mathr-deep-zoom-theory]] §Rescaling).
But deltas start near the pixel spacing, so they underflow once the zoom
passes the type's exponent range.

## Range limits

| Type | Normal min | Underflow of deltas | Squared norms underflow |
| --- | --- | --- | --- |
| float32 | 1.18e-38 (denormals to 1.4e-45) | ~1e-38 | ~1e-19 |
| float64 | 2.2e-308 | ~1e-308 | ~1e-154 |

Single precision "underflows below 1e-38 or so"; double below about 1e-308
(src: [[sources/mathr-deep-zoom-theory]] §Deep Needle; §Rescaling). Tests
on `|z|^2` (bailout, rebase, BLA radius) break at the square root of the
floor (inference). Apple GPU denormal handling is unknown, so assume
flush-to-zero and a floor of 1.18e-38 ([[questions/apple-gpu-denormals]]).

## floatexp

A mantissa normalised near 1 plus a separate integer exponent. Works at any
depth, but "the performance is terrible because it needs to handle every
arithmetic operation in software" (src: [[sources/mathr-deep-zoom-theory]] §Rescaling).
Munafo: a normal float, usually 32-bit, plus an explicit integer exponent
(src: [[sources/muency-floatexp]]).

```text
struct fe { float m; int e; }          // value = m * 2^e, 0.5 <= |m| < 1 or m = 0
mul(a, b): r = fe(a.m*b.m, a.e+b.e); renormalise(r)
add(a, b): if a.e < b.e swap; d = a.e - b.e
           if d > 30: return a                     // b below a's precision
           r = fe(a.m + ldexp(b.m, -d), a.e); renormalise(r)
renormalise(r): (m, k) = frexp(r.m); r.m = m; r.e += k
```

Cost (inference): multiply is one fp32 multiply and an integer add plus
renormalisation; add needs exponent alignment, compare and select, about 5
to 10 fp32 operations, branchy. Complex values can share one exponent
between real and imaginary parts.

Measured on mathr's desktop CPU, 2022 (iterations per second): float 270M,
double 256M, x87 long double 143M, softfloat (uint32 mantissa + int
exponent) 8.2M, floatexp (float + int) 4.3M
(src: [[sources/fractalforums-interior-detection]] §Reply #7). He found
all-integer software floats about 2× faster than float32 + int32 for basic
arithmetic on CPU (src: [[sources/fractalforums-interior-detection]] §Reply #9).
Whether that holds on a GPU, where int multiply is slow, is open
([[questions/double-float-speed-on-m1]]).

## Rescaled iterations

Substitute `z = S w` and `c = S d` (an "unevaluated product"):

```text
S w → 2 Z S w + S² w² + S d      ⇒      w → 2 Z w + S w² + d
```

Choose S so |w| ≈ 1; redo the scaling when |w| risks over- or underflow,
"typically a few hundred iterations as |Z| is bounded by 2 except at final
escape". If S underflows to 0 in hardware precision, skip the `S w²` term
when Z is not small; likewise skip `d` if it underflowed
(src: [[sources/mathr-deep-zoom-theory]] §Rescaling).

**Full iterations.** When |Z| is very small it underflows in the scaled
iteration and |w| can change dramatically. Then compute `z = S w` in
floatexp (S kept as floatexp), do the plain perturbed step in floatexp,
and rescale: `S = |z|`, `w = z/S`, `d = c/S` (rounded to hardware precision),
and recompute the hardware `s`. This is Pauldelbrot's "Rescaled Iterations
in Nanoscope" (src: [[sources/mathr-deep-zoom-theory]] §Full Iterations).

Float32 kernel sketch (our own; GPU details are inference):

```text
// per pixel: c_fe floatexp; S_fe floatexp; w, d, s float32
S_fe = |c_fe|; w = float(c_fe / S_fe); d = w; s = float(S_fe)   // s may be 0
loop:
    if smallZ[m] or !(2^-60 < |w| < 2^60):        // rare: full floatexp step
        z_fe = S_fe * w
        z_fe = 2*Zref_fe[m]*z_fe + z_fe*z_fe + c_fe
        S_fe = |z_fe|; w = float(z_fe/S_fe); d = float(c_fe/S_fe); s = float(S_fe)
    else:
        w = (2*Zref[m] + s*w) * w + d              // pure float32 FMA path
    m += 1
    // escape/rebase need Zref[m] + S*w; when s == 0, |S w| << |Z| unless
    // |Zref[m]| is itself tiny, which is the floatexp branch
```

Why rebasing and rescaling fit (inference): rebasing can only fire when
`|Z_m|` is comparable to `|z|`, i.e. tiny at depth, which is exactly when
the floatexp branch runs. The hot path stays float32.

KF results: rescaled perturbation (2.15.3) for "arbitrarily deep zooms",
"usually faster than old long double and floatexp", for Mandelbrot powers
2 and 3, Burning Ship power 2 and hybrids; single precision disabled by
default "due to some locations having undetected glitches"; rescaled SA
about 30% faster than all-floatexp (src: [[sources/kf-manual]] §kf-2.15.3).
Known issue: "Rescaled Double is slow in some locations"
(src: [[sources/kf-manual]] §Known Bugs). Rebasing works with rescaled
iterations in KF after a fix credited to superheal
(src: [[sources/kf-manual]] §kf-2.15.5.1).

**Deep needle.** For Burning Ship and other abs variations, full iterations
are needed when either X or Y is small, not only both; near the needle that
is nearly every iteration, so plain floatexp is faster than rescaling. KF
never solved detecting such locations automatically
(src: [[sources/mathr-deep-zoom-theory]] §Deep Needle). See
[[formulas/burning-ship]].

**Derivatives.** Initialise dual parts with the pixel spacing instead of 1
and do not rescale at the end; this avoids derivative overflow and lets the
same scale factor serve value and derivative
(src: [[sources/mathr-deep-zoom-theory]] §Hybrid Fractals). See
[[techniques/distance-estimation]].

## Exponent-carrying deltas renormalised every iteration

The webgpu renderer carries the delta as a mantissa/exponent pair
renormalised every iteration. Two silent-failure bugs it found
(src: [[sources/desarso-mandelbrot-webgpu-readme]] §Deltas that carry their own exponent):

- renormalising with a single `exp2(-shift)` overflows when the mantissa
  drops below 2^-128 (whenever the reference passes near zero); split the
  rescale into two halves;
- a comparison that looked at exponents first broke the rebase test
  ([[techniques/rebasing]]).

## Skip-table coefficients

BLA/LA coefficients grow over long skips (`A` reaches about 1e700 over 4096
iterations), so they need explicit exponents even in double
(src: [[sources/desarso-mandelbrot-webgpu-readme]] §Skipping most of the work).
Imagina stores a stage in double only when its radii and thresholds stay
above about 2^-896 / 2^-768, else FloatExp (src: [[sources/imagina-hinfla-evaluator]] §ConvertStageToDouble).
In float32 every coefficient and radius needs its own exponent once the
zoom passes about 1e-30 (inference).

## Depth thresholds for a float32 GPU

| Pixel spacing | Delta type | Skip-table storage | Source |
| --- | --- | --- | --- |
| > ~1e-5 | direct float32, no reference | none | webgpu table |
| ~1e-5 to ~1e-25 | float32 perturbation, Chebyshev/max-abs norms in tests | float32 | webgpu table; inference |
| < ~1e-25, unbounded | rescaled float32 (floatexp S) or exponent-carrying deltas; floatexp full steps near \|Z\|≈0 and at rebases | floatexp per quantity | webgpu; mathr |

The webgpu renderer hands over to exponent-carrying deltas at 1e-25 units
per pixel, "still thirteen decades above the f32 floor", because the skip
table carries the frame from there and plain deltas lose (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Choosing a method by depth).
KF limited plain single precision to shallow zooms "until zoom e20"
(src: [[sources/kf-manual]] §kf-2.15.3), and its SIMD double path to about
1e300, scaled double to about 1e600 (power 2) and 1e400 (power 3)
(src: [[sources/kf-manual]] §kf-2.14.8).

Type ladders elsewhere:

- Fraktaler 3: float, double, floatexp, doubleexp (3.1), long double (x86),
  float128 (3.1), chosen per location by benchmarking ("wisdom")
  (src: [[sources/fraktaler-3-site]]).
- FractalShark: float, double, double-float, "2×32 + exponent" (two floats
  and a shared exponent, about 48-bit mantissa) and HDR float + exponent;
  2×32+exp gives "substantially higher performance on consumer GPUs with only
  a modest loss of precision" (src: [[sources/fractalshark-readme]]).
- Zhuoran's single-precision rebasing test: a location at height 4e-15
  with 5M iterations rendered correctly in float32
  (src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #2).

## Pitfalls

- Squared-norm tests underflow 19 decades before deltas do.
- Compare normalised magnitudes on extended-range types.
- Keep separate Metal pipelines per number type, so floatexp register
  pressure does not slow the float32 path on M1/M2 ([[platform/apple-gpu-architecture]]).
- Float32 errors are silent: verify against a CPU double/floatexp render
  ([[questions/float32-error-long-orbits]]).

## For almondbread

Roadmap step 6: float32 perturbation to about 1e-25, then rescaled float32
with floatexp scale and full steps, floatexp per-quantity skip tables,
selected by depth (later by measured speed, like Fraktaler's wisdom).
[[techniques/double-float]] is a separate precision fix, not a range fix.

## As built (M3b, 2026-10-14)

Our own design on the pattern above (mathr's rescaling, KF's "rescaled" types); code:
`iterate` with function constant `kDeep` in `crates/almondbread-render/src/shaders/mandelbrot.metal`,
CPU twin `cpu_render_deep` in `crates/almondbread-core/src/oracle.rs`.

- **Frame exponent.** The texel is `texelScale · 2^scaleExp` with `texelScale` in [1, 2);
  `refOffset` is in the same units, so the pixel's `c` is a float32 mantissa (as in the plain
  kernel) plus one integer exponent per view.
- **Two regimes per pixel.** P: `|z| ≥ 2^-50`, the plain float32 loop (squared-norm escape and
  rebase tests are safe). T: `z = w · 2^e` with `|w|` kept in [2^-16, 2^16] by renormalising every
  step; the step is `w ← (2Z + s w) w + c·2^-e` with `s = 2^e` (0 below float32's range). Enter T
  below 2^-50, leave it at 2^-41 (hysteresis). In T no escape or rebase test runs while
  `|Z| ≥ 2^-32`: a rebase needs `|Z| < 2|z| < 2^-40`, an escape needs the reference's last point.
- **Full steps** in floatexp (complex mantissa, shared int exponent) where `|Z| < 2^-32`, at `m = 0`
  and at the reference's end: the step, the exact rebase test `|Z + z| < |z|` and the escape test
  use the reference point's own exponent from a second buffer (`FloatExpComplex`, 16 B per
  point). For a nucleus reference these are exactly the near-0 returns of each period.
- **Derivatives** carry exponents: DE `d = dz/dc · texel = dm · 2^dExp`, interior
  `w' = dz/dz₁ = zw · 2^wExp`, renormalised every 8 steps and whenever `|z_full| < 2^-16`; the
  interior test uses the true magnitude `|zw| n² 2^wExp`, and runs after every full step (the one
  place an interior pixel's derivative is small: with periods ≢ 0 mod 8 the every-8th-step test
  never saw it).
- **Kernel choice by depth.** f64 references (unchanged M1–M4 path) above 1e-12 per point; big
  references with the plain kernel down to 1e-17 (~30 % faster than the deep kernel per
  iteration there); the deep kernel below.

Pitfall found: a reference's precision must cover the *minibrot* it serves, not the search
radius: near the needle a period-255 nucleus found in a 1e-152 disc has size 1e-303; computed at
640 bits it was no nucleus at all (`|z_255|` = 1e-160 only after refining to 1152 bits).
`almondbread locate` refines until bits ≥ −log2(size) + 96.
