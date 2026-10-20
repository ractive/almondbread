---
title: "Distance estimation"
type: technique
status: draft
sources: ["sources/cheritat-mandelbrot-rendering", "sources/mathr-deep-zoom-theory", "sources/mathr-deep-zoom-page", "sources/wikipedia-plotting-algorithms", "sources/kf-manual", "sources/imagina-hinfla-evaluator", "sources/zoomasm"]
tags: [distance-estimation, derivative, anti-aliasing, lighting]
---

# Distance estimation

The exterior distance estimate (DE) gives, per escaped pixel, an estimate of the
distance from `c` to the set. One derivative buys: thin-filament rendering,
analytic anti-aliasing, boundary detection, adaptive-sampling triggers and
slope/normal-map lighting. Interior DE is covered in
[Interior detection](interior-detection.md).

## Formula (Milnor–Thurston)

Iterate the `c`-derivative alongside `z`:

- `ρ₀ = 0`, `ρ_{n+1} = 2 z_n ρ_n + 1` (with `z₀ = 0`)
- on escape (`|z_n| > R`): `d_n = 2|z_n| ln|z_n| / |ρ_n|`

(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Exterior distance estimation;
[Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Boundary detection methods via distance estimators).

- Koebe 1/4: no point of the set is closer than `d/4`
  (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Exterior distance estimation).
- `d_n` does not converge to the true distance; only `1/5 < d/lim d_n < 1` holds
  (for `|z| < 5.7`) (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Principle).
- Use a large bailout (Chéritat uses R = 1000 for DE, 100 for the normal map); a
  few extra iterations past bailout improve the estimate
  (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Exterior distance estimation).
- Update `ρ` before `z` (or via temporaries)
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Implementation).

## Under perturbation

Use the full orbit value in the recurrence: `der ← 2(Z_m + z_n)·der + 1` (the
form is derived from the dual-number formulation, not quoted). mathr's rule for
deep zooms: use dual numbers for automatic differentiation and "initialize the
dual parts with the pixel spacing and don't scale at the end". This prescales
the derivative into pixel units, avoids overflow, and lets the same rescaling
factor serve the value and dual parts (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Hybrid Fractals;
[mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §5 Distance Estimation). For non-conformal
formulas (Mandelbar, Burning Ship) use two dual parts and combine them into a
Jacobian for the directional DE (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §5 Distance Estimation).

Through iteration skipping: a BLA step `z → A z + B c` maps the c-derivative as
`der → A·der + B·(pixel spacing)` (inference). Imagina propagates `dzdc` through
each LA step as `dzdc·2(dz+Ref)·ZCoeff + CCoeff·scale`, and in AT as
`2z·dzdc + scale`, holding it pre-scaled (by 2^448 in double) to avoid overflow
(src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)).

```text
// float32, derivative in pixel units: der = d(Z+z)/dk, k = pixel coordinate
der = (0, 0)
per perturbation step:  der = 2 * (Zref[m] + z) * der + 1      // dc/dk = 1 in pixel units
per BLA skip:           der = A * der + B                      // B times dc/dk
on escape:              zz = Zref[m] + z
                        de_px = 2 * |zz| * log(|zz|) / |der|   // distance in pixels
```

Only the derivative is measured in pixel units (`dc/dk = 1`); the iteration
itself still uses the true `c`. Exterior `|der|` grows with
iteration count; in float32 (max ~3.4e38) it can overflow for pixels very close
to the set after many iterations, so keep a separate exponent or rescale it
like the delta (inference; see [Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md)).

## Uses

- **Boundary test without sqrt/division:** `rsq·(log rsq)² < |thickness·pixel_size·ρ|²`
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Implementation).
- **AA without oversampling:** `t = clamp(d/(thickness·pixel_size), 0, 1)`, lerp
  boundary colour → exterior colour, thickness ≈ 1.414
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Implementation). See [Anti-aliasing and supersampling](anti-aliasing.md).
- **Henriksen's variant (pixel as a disk):** iterate `der ← 2z·der + dc` with
  `dc = pixel_size·thickness`; boundary if `|z|² < |der|²`. Thickness above 0.25
  looks bad; R = 10 works. Variants use `z+c` instead of `z` (avoids marking
  component centres) or the spherical metric
  `rsq·(1 + rsq/4) < |der+dc|²` (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Variant).
- **Exterior banding:** parity of `floor(log(d)/k)` (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Boundary detection methods via distance estimators).
- **Normal map / slopes:** `u = z/der` normalised; see [Colouring methods](coloring-methods.md).
- **Adaptive sampling:** extra samples only where `de_px < ~2` (inference).
- **Guessing safety:** if any tile-border pixel has DE smaller than the tile,
  refine (Koebe bound) (inference; see [Guessing, border tracing and rectangle subdivision](guessing-and-subdivision.md)).

## Numerical DE (no derivative)

KF2+ offers DE computed from neighbouring smooth iteration counts: Traditional,
Forward 3×3, Central 3×3, Diagonal 2×2, Least Squares 2×2/3×3, Laplacian 3×3.
Analytic DE needs derivatives enabled, which costs speed; directional DE is
stored as DEX/DEY in EXR and drives analytic slopes. Known issue: analytic DE
breaks at some power-3 locations unless SA is disabled (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
zoomasm rescales DE to screen space from the EXR channels
(src: [zoomasm (zoom video assembler)](../sources/zoomasm.md)). Numerical DE is a 3×3 post-pass: cheap fallback
for slopes and AA guidance when derivatives are off (inference).

## Cost

One extra complex multiply-add per iteration (about +30–50% ALU per
perturbation step), one complex multiply-add per skip, plus a log at escape
(inference).

## For almondbread

Track `der` in pixel units (mathr's prescale) in the perturbation and BLA/LA
kernels behind a function constant, so pipelines without DE stay lean. Store
`de_px` (float16 is enough) and the direction of `u = z/der` per pixel for the
colouring pass. Use DE for filament AA, slope lighting, and as the adaptive
sampling trigger. Add exponent tracking for `der` together with the rescaled
float32 path of [Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md). Related:
[Perturbation](perturbation.md), [Bivariate linear approximation (BLA)](bla.md).

# Citations

- [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
- [zoomasm (zoom video assembler)](../sources/zoomasm.md)
