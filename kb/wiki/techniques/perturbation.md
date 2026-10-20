---
title: "Perturbation"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/ultrafractal-perturbation-calculations]]", "[[sources/ultrafractal-perturbation-equations]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/kf-manual]]"]
tags: [perturbation, deep-zoom, core]
---

# Perturbation

Iterate one point (the reference) in arbitrary precision, and every pixel as
a small difference from it in hardware floats. This is the foundation of
every deep-zoom renderer since 2013; everything else in this wiki is built
on it.

## Formula

Notation (mathr's): upper case = reference, high precision; lower case =
per-pixel delta, low precision and low range
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Perturbation).

- Reference orbit, high precision: `Z_{m+1} = Z_m^2 + C`, `Z_0 = 0`.
- Pixel orbit is `Z_m + z_n`, pixel parameter `C + c`.
- Delta, low precision: `z_{n+1} = 2 Z_m z_n + z_n^2 + c`, `z_0 = 0`
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §2 Perturbation).

Derivation: substitute `Z + z` and `C + c` into `z^2 + c` as unevaluated
sums, then cancel the reference terms symbolically. Computing
`(Z+z)^2 + (C+c)` numerically and subtracting `Z^2 + C` loses everything to
catastrophic absorption ("1 million plus 1 is still 1 million with 3
significant digits") (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Perturbation).
The symbolic delta form is mandatory.

Equivalent factored form, one complex multiply cheaper:
`z_{n+1} = (2 Z_m + z_n) z_n + c` (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Glitches).

Wikipedia writes the same thing as `ε_{n+1} = 2 z_n ε_n + ε_n^2 + δ` and
notes that ε rarely needs more than about 16 significant figures, so
hardware floats suffice (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Perturbation theory and series approximation).

## Why it works

The map is continuous, so nearby points stay nearby for a long time: one
high-precision orbit plus cheap low-precision differences describes a whole
image (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Introduction). The
reference is computed in high precision and rounded to double (or float) for
storage; this "works fine most of the time" (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Perturbation).
The cost of high precision moves from `pixels × iterations` to
`iterations` (one orbit).

## Per-pixel loop (plain, no skipping)

```text
// Zref[0..M] : reference orbit rounded to the delta type (float32 on Metal)
// c          : pixel offset from the reference, computed as
//              (pixel - refPixel) * pixelSpacing in the low-precision type
m = 0; z = 0
for n in 0 ..< maxIter:
    z = (2*Zref[m] + z) * z + c
    m += 1
    Zfull = Zref[m] + z
    if |Zfull|^2 > bailout^2: return escaped(n+1, Zfull)
    if |Zfull| < |z| or m == M:          // rebasing, see techniques/rebasing
        z = Zfull; m = 0
return maxIterReached
```

The rebase lines are [Rebasing (Zhuoran, 2021)](rebasing.md); without them this loop
glitches ([Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](glitch-detection.md)). Iteration skipping
([Bivariate linear approximation (BLA)](bla.md), [Linear approximation with period-aligned stages and AT (Imagina)](linear-approximation-imagina.md)) wraps
around this loop.

## Cost

- Per iteration: one complex multiply, one complex add, a squared norm for
  bailout and a comparison for rebasing. About 10 fp32 operations.
- The reference: `M` high-precision iterations, serial
  ([Reference orbit: precision, cost, storage, compression](reference-orbit-precision.md)).
- Memory: one complex per reference iteration in the delta type, 8 B in
  float32. KF needs "up to ~40 bytes per reference iteration" depending on
  number type (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §Limits).

## Pitfalls

- **Pixel offset `c`**: compute it in the low-precision type from integer
  pixel indices and the pixel spacing, never by subtracting two
  high-precision coordinates on the GPU (inference).
- **Range**: float32 deltas underflow near 1e-38 and squared norms near
  1e-19; see [Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md).
- **Reference far from the view**: if `|c|` is many pixel spacings, float32
  keeps fewer bits of sub-pixel offset (inference). Keep the reference near
  the view or use a nucleus close by ([Reference selection: nucleus, period detection, Newton](reference-selection.md)).
- **Two counters**: with rebasing, the pixel iteration `n` and reference
  index `m` differ; colouring uses `n`.

## History

- K.I. Martin's SuperFractalThing and its white paper `sft_maths.pdf` (2013)
  popularised perturbation together with series approximation
  (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Introduction); see
  [SuperFractalThing](../implementations/superfractalthing.md).
- Kalles Fraktaler industrialised it ([Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md)).
- Ultra Fractal 6 added perturbation with one reference at the centre and
  double offsets, adding references when it detects precision problems
  (src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md) §Perturbation calculations).
  Formula authors there hand-write the perturbed step, e.g.
  `#dz = 2*#z*#dz + sqr(#dz) + #dpixel` (src: [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md) §Perturbation equations).
- Pauldelbrot's glitch test (pre-2021) and Zhuoran's rebasing (3 Aug 2021)
  solved the glitch problem (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Another solution to perturbation glitches).

## For almondbread

High-precision reference on the CPU ([Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md)),
stored as float32 (or floatexp) in a shared buffer; per-pixel delta loop in a
Metal kernel with rebasing from day one; BLA on top. See the roadmap in
[Deep zoom Mandelbrot rendering techniques](../synthesis/deep-zoom-rendering-techniques.md).
Other formulas: [Mandelbrot set (power 2)](../formulas/mandelbrot.md), [Multibrot (integer power p)](../formulas/multibrot.md),
[Burning Ship and abs variations](../formulas/burning-ship.md), [Hybrid formulas](../formulas/hybrids.md).
