---
title: "Colouring methods"
type: technique
status: draft
sources: ["[[sources/wikipedia-plotting-algorithms]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/wikibooks-triangle-inequality]]", "[[sources/wikibooks-mandelbrot-interior]]", "[[sources/kf-manual]]", "[[sources/zoomasm]]", "[[sources/philthompson-bla]]"]
tags: [coloring, histogram, stripe-average, lighting, palette]
---

# Colouring methods

Everything here runs on top of the smooth count ([[techniques/smooth-coloring]])
and, for lighting, the derivative ([[techniques/distance-estimation]]). Two
families: **post-process** methods that need only per-pixel outputs (n, frac,
DE, final z, flags) and **per-iteration averages** that add work inside the
loop.

## Post-process methods

### Transfer function and palette

Map `μ` through a transfer function, then into a palette.

- KF2+ transfer functions: Standard, Square root, Cubic root, Logarithm,
  Stretched, distance-based, LogLog, ATan, Fourth root (the last three ported
  from Mandel Machine); plus palette waves, infinite waves, texture, and
  dithering at the end to reduce banding (src: [[sources/kf-manual]]).
- Deep-zoom log-potential palette (Chéritat): `x = log(V)/K` with `K = log 2`,
  colour `= g(x)` periodic, e.g. `g(x) = (1 + cos 2πx)/2`. Larger K for "furry"
  deep zooms (src: [[sources/cheritat-mandelbrot-rendering]] §The potential).
  Taking the log keeps colour density usable as iteration counts grow.
- Exponentially mapped and cyclic iterations, HSV and LCH gradients
  (src: [[sources/wikipedia-plotting-algorithms]] §Exponentially mapped and cyclic iterations).

### Histogram colouring

1. Count pixels per integer iteration: `hist[i]`.
2. `total = Σ hist[i]` over escaped pixels.
3. `hue(pixel) = Σ_{i ≤ n} hist[i] / total`, optionally interpolated with `frac`.

It spreads colours by area independently of maxIter
(src: [[sources/wikipedia-plotting-algorithms]] §Histogram coloring; that section
is flagged as needing citations). On GPU: atomic histogram over the frame, then
a prefix sum, then a lookup pass (inference). With counts up to 1e9, bin by
`log(μ)` or by `n − n_min` rather than raw `n` (inference). It flickers across
zoom frames unless the CDF is smoothed over time (inference).

### Phase, binary decomposition, field lines

KF2+ stores a phase channel `T = arg(z_final)` with a "phase strength" control
(src: [[sources/kf-manual]]); zoomasm keyframes carry it as T
(src: [[sources/zoomasm]]). Binary decomposition is the sign of `Im(z_final)`
and field lines come from `arg(z_final)` against the smooth count (standard,
formulas not retrieved) (unverified). Both are free once `z_final` is stored.

### Slope / normal-map lighting

Chéritat defines a normal from the potential: `u = z/der` with
`der = dz/dc`, normalised. Light from direction `v = exp(i·angle)` at height
`h2`: `t = (Re u·Re v + Im u·Im v + h2)/(1 + h2)`, clamped at 0, then
`colour = lerp(black, white, t)`. He uses `h2 = 1.5`, angle 45°, `R = 100`
(src: [[sources/cheritat-mandelbrot-rendering]] §Normal map effect). A
Milnor-DE variant adds a second derivative `der2 ← 2(der2·z + der²)` and uses
`u = z·der·((1 + lo)·conj(der²) − lo·conj(z·der2))`, `lo = ½ log|z|²`
(src: [[sources/cheritat-mandelbrot-rendering]] §Variation). The normals match
no real height field (src: [[sources/cheritat-mandelbrot-rendering]] §Normal map effect).
The scale of `der` cancels in `u/|u|`, so slopes are nearly free once DE is
tracked (inference). KF2+ slopes became zoom- and iteration-independent in
2.15.1; directional DE (DEX/DEY) fixes noisy slopes with jitter
(src: [[sources/kf-manual]]).

### Interior colouring

Closest-approach index (`√min|z|` scaled), argument of the last iterate
(Fractint INSIDE=ATAN), capture time (src: [[sources/wikibooks-mandelbrot-interior]]).
These need per-iteration tracking of `min|z|` for interior pixels.

## Per-iteration averages (branching averages)

All accumulate a quantity `t(z_n)` each iteration, divide by the count, and
interpolate between the last two partial averages with the smooth fraction so
bands vanish.

**Triangle inequality average (TIA, Kerry Mitchell).** Per iteration with
`w = z_old²` and `z_new = w + c`: the triangle inequality bounds `|z_new|` between
`lo = | |w| − |c| |` and `hi = |w| + |c|`, and
`sum += (|z_new| − lo)/(hi − lo)`. Divide by i. Smooth with the previous average
`sum2`: `f = (1/ln p)·ln(ln(B)/2) − (1/ln p)·ln(ln|z|)`,
`index = sum2 + (sum − sum2)·(f + 1)`. TIA is discontinuous where points map to
zero (src: [[sources/wikibooks-triangle-inequality]]).
KF2+ has TIA only under OpenCL, replacing the final angle in the phase channel
(src: [[sources/kf-manual]]).

**Curvature average.** `a += |arg((z_n − z_{n−1})/(z_{n−1} − z_{n−2}))|`,
interpolated the same way; continuous where TIA is not
(src: [[sources/wikibooks-triangle-inequality]]).

**Stripe average (Jussi Härkönen).** Accumulate `½ sin(s·arg z_n) + ½` with stripe
density `s`. Chéritat prefers it to TIA because it follows external rays
(src: [[sources/cheritat-mandelbrot-rendering]] §Radial strands). The exact
formula was not retrieved from a fetched source (unverified).

**Iteration skipping breaks averages.** A BLA/LA skip does not produce the
intermediate `z_n`, so averages need per-step values. Very Plotter's BLA does
not support stripe colouring (src: [[sources/philthompson-bla]]). Options:
accumulate only over the non-skipped tail (the last k iterations before
escape, which dominate visually), or disable skipping for those pixels
(inference).

Cost: TIA ~2 extra `length` + a divide per iteration; stripe one `atan2` and one
`sin` per iteration; curvature an `atan2` of a complex divide (inference).

## For almondbread

v1: smooth `(n, frac)` → log transfer → periodic cosine palette, plus DE
darkening and slope lighting from stored `u` direction. All in a separate
colouring pass over stored per-pixel data (n, frac, de_px, u, arg z, flags), so
palette edits are instant. Histogram colouring as an option via GPU
reduction. Per-iteration averages later, restricted to the post-skip tail.
Related: [[techniques/anti-aliasing]], [[techniques/exponential-map]].
