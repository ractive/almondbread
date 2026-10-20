---
title: "Smooth (continuous) iteration count"
type: technique
status: draft
sources: ["[[sources/quilez-smooth-iteration]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/kf-manual]]", "[[sources/zoomasm]]"]
tags: [coloring, smooth-iteration, bailout, precision]
---

# Smooth (continuous) iteration count

The integer escape count `n` gives visible bands. The smooth count adds a
fractional part from how far past the bailout the last iterate landed. It is
the near-free baseline for every colouring method
([Colouring methods](coloring-methods.md)).

## Formula

For `z → z^p + c` with bailout radius `B`:

- `μ = n − log_p( log|z_n| / log B )` (Wikipedia's normalised iteration count;
  for p = 2 often written `n + 1 − log₂(log|z_n|)` with an offset)
  (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Continuous (smooth) coloring).
- Quilez: `μ = n − log(log(length(z))/log(B))/log(2)`. A bailout "in the order of
  50" is enough for visual smoothness; his examples use B = 256. Compare
  `dot(z,z)` against `B²` to skip the square root; for B = 2⁸ the optimised form
  is `n − log2(log2(dot(z,z))) + 4`
  (src: [Quilez: Smooth iteration count for generalized Mandelbrot sets](../sources/quilez-smooth-iteration.md) §Smooth iteration count).
- Potential view: `V = log|z_n| / 2ⁿ` (Chéritat uses `log|z|²/2ⁿ`, stopping at
  `|z|² > 10⁶`); relative error about `1/R`
  (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §The potential).
  `μ` and `log V` differ only by sign and constants.

```text
// colouring input, computed once at escape (float32)
r2   = dot(zz, zz)                       // zz = Zref[m] + z, r2 > B*B
frac = 1.0 - log2(0.5 * log2(r2))        // B-independent form when B is large
store n (uint32), frac (float32)          // mu = n + frac
```

With a large bailout the B-dependent term is a constant offset; dropping it
just shifts the palette (inference). KF 2.15.1+ offsets the log smooth count
by `−log(log(BailoutRadius))/log(Power)` (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

## Bailout choice

- Larger B makes `μ` smoother and more accurate (error ~1/R) at a cost of a few
  extra iterations (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §The potential).
- KF2+ offers High, Bailout = 2, Low bailout and Custom radii, plus custom norm
  power; its "Log" smooth method is the renormalised count and can show seams,
  "Linear" interpolates the last two iterations and suits small bailouts
  (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Under perturbation the escape test is on `|Z+z|²`, fine in float32 (inference).
- Use R ≥ 256 (inference from Quilez's B = 256; Chéritat uses 1000 for DE).

## Precision trap at depth

At deep zoom `μ` reaches 1e6 to 1e9. A float32 has 24 bits, so at `μ ≈ 1e8` the
spacing between representable values is 8: the fractional part is gone and the
image bands (inference). Existing tools split the count:

- zoomasm stores N in split EXR channels N0/N1/NF (with an IterationsBias) and
  `getN()` returns it "at higher precision than single-precision float"
  (src: [zoomasm (zoom video assembler)](../sources/zoomasm.md)).
- KF's GLSL colouring uses a `float49` type (libqd-derived, ~49 bits) so smooth
  counts survive large N (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

So store `n` as uint32 and `frac` as float32 separately, and form colours from
`(n − n_ref) + frac` or from `log(μ)` computed in mixed form (inference).

## Cost

One `log2` of `dot(z,z)` and one more `log2` per pixel at escape. Negligible.

## For almondbread

Per pixel store `uint32 n` and `float frac` (plus flags) in the iteration
buffer; never a single float `μ`. Bailout 2⁸ (squared test against 2¹⁶); go
higher only if DE needs it. Do colouring in a separate pass from stored data so
palette changes never recompute the fractal. Related:
[Distance estimation](distance-estimation.md), [Anti-aliasing and supersampling](anti-aliasing.md).
