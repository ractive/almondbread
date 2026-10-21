---
title: "Colouring methods"
type: technique
status: draft
sources: ["sources/wikipedia-plotting-algorithms", "sources/cheritat-mandelbrot-rendering", "sources/wikibooks-triangle-inequality", "sources/wikibooks-mandelbrot-interior", "sources/kf-manual", "sources/zoomasm", "sources/philthompson-bla", "sources/mathr-interior-distance", "sources/muency-atom-domain"]
tags: [coloring, histogram, stripe-average, lighting, palette, interior-coloring]
---

# Colouring methods

Everything here runs on top of the smooth count ([Smooth (continuous) iteration count](smooth-coloring.md))
and, for lighting, the derivative ([Distance estimation](distance-estimation.md)). Two
families: **post-process** methods that need only per-pixel outputs (n, frac,
DE, final z, flags) and **per-iteration averages** that add work inside the
loop.

## Post-process methods

### Transfer function and palette

Map `μ` through a transfer function, then into a palette.

- KF2+ transfer functions: Standard, Square root, Cubic root, Logarithm,
  Stretched, distance-based, LogLog, ATan, Fourth root (the last three ported
  from Mandel Machine); plus palette waves, infinite waves, texture, and
  dithering at the end to reduce banding (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Deep-zoom log-potential palette (Chéritat): `x = log(V)/K` with `K = log 2`,
  colour `= g(x)` periodic, e.g. `g(x) = (1 + cos 2πx)/2`. Larger K for "furry"
  deep zooms (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §The potential).
  Taking the log keeps colour density usable as iteration counts grow.
- Exponentially mapped and cyclic iterations, HSV and LCH gradients
  (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Exponentially mapped and cyclic iterations).

### Histogram colouring

1. Count pixels per integer iteration: `hist[i]`.
2. `total = Σ hist[i]` over escaped pixels.
3. `hue(pixel) = Σ_{i ≤ n} hist[i] / total`, optionally interpolated with `frac`.

It spreads colours by area independently of maxIter
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Histogram coloring; that section
is flagged as needing citations). On GPU: atomic histogram over the frame, then
a prefix sum, then a lookup pass (inference). With counts up to 1e9, bin by
`log(μ)` or by `n − n_min` rather than raw `n` (inference). It flickers across
zoom frames unless the CDF is smoothed over time (inference).

### Phase, binary decomposition, field lines

KF2+ stores a phase channel `T = arg(z_final)` with a "phase strength" control
(src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)); zoomasm keyframes carry it as T
(src: [zoomasm (zoom video assembler)](../sources/zoomasm.md)). Binary decomposition is the sign of `Im(z_final)`
and field lines come from `arg(z_final)` against the smooth count (standard,
formulas not retrieved) (unverified). Both are free once `z_final` is stored.

### Slope / normal-map lighting

Chéritat defines a normal from the potential: `u = z/der` with
`der = dz/dc`, normalised. Light from direction `v = exp(i·angle)` at height
`h2`: `t = (Re u·Re v + Im u·Im v + h2)/(1 + h2)`, clamped at 0, then
`colour = lerp(black, white, t)`. He uses `h2 = 1.5`, angle 45°, `R = 100`
(src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Normal map effect). A
Milnor-DE variant adds a second derivative `der2 ← 2(der2·z + der²)` and uses
`u = z·der·((1 + lo)·conj(der²) − lo·conj(z·der2))`, `lo = ½ log|z|²`
(src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Variation). The normals match
no real height field (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Normal map effect).
The scale of `der` cancels in `u/|u|`, so slopes are nearly free once DE is
tracked (inference). KF2+ slopes became zoom- and iteration-independent in
2.15.1; directional DE (DEX/DEY) fixes noisy slopes with jitter
(src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

### Interior colouring

Closest-approach index (`√min|z|` scaled), argument of the last iterate
(Fractint INSIDE=ATAN), capture time (src: [Wikibooks: Mandelbrot set interior](../sources/wikibooks-mandelbrot-interior.md)).
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
zero (src: [Wikibooks: Triangle inequality average (TIA) colouring](../sources/wikibooks-triangle-inequality.md)).
KF2+ has TIA only under OpenCL, replacing the final angle in the phase channel
(src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

**Curvature average.** `a += |arg((z_n − z_{n−1})/(z_{n−1} − z_{n−2}))|`,
interpolated the same way; continuous where TIA is not
(src: [Wikibooks: Triangle inequality average (TIA) colouring](../sources/wikibooks-triangle-inequality.md)).

**Stripe average (Jussi Härkönen).** Accumulate `½ sin(s·arg z_n) + ½` with stripe
density `s`. Chéritat prefers it to TIA because it follows external rays
(src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md) §Radial strands). The exact
formula was not retrieved from a fetched source (unverified).

**Iteration skipping breaks averages.** A BLA/LA skip does not produce the
intermediate `z_n`, so averages need per-step values. Very Plotter's BLA does
not support stripe colouring (src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)). Options:
accumulate only over the non-skipped tail (the last k iterations before
escape, which dominate visually), or disable skipping for those pixels
(inference).

Cost: TIA ~2 extra `length` + a divide per iteration; stripe one `atan2` and one
`sin` per iteration; curvature an `atan2` of a complex divide (inference).

## As built in almondbread (FL: looks)

Built 2026-10-14 (`crates/almondbread-core/src/palette.rs`, the `shade` function of the
kernels). Our design; the OKLab matrices are Björn Ottosson's (unverified, not ingested).

- **Palettes in OKLab.** Stops are sRGB colours joined by a periodic Catmull-Rom spline in
  OKLab, then converted to linear RGB and clamped: perceptually even ramps, saturated hue
  blends, no kinks at the stops. Ten curated palettes (Aurora, Classic refined, Ember,
  Lagoon, Nebula, Dusk, Moss, Ice, Graphite, Spectrum on a dark base) plus the old simple
  ones. Swatches in the UI sample the real table.
- **Looks.** A look bundles a palette with density, light angle and height, highlight,
  outline width/colour/strength, glow and interior colour; the inspector applies one in one
  step and every value stays editable. Default: Aurora with lighting 0.75, highlight 0.35,
  outline 0.7 at 1.6 px, 9-sample AA.
- **Palette anti-aliasing.** The smooth count changes by ~`1/(d ln 2)` per pixel at DE
  distance `d` pixels (`μ = −log₂ G`, `|∇G| ≈ G/d`, inference), so the palette footprint of
  a pixel is `density/(d ln 2)` cycles. The palette is uploaded as a cyclic mip chain
  (box-filtered halvings) and looked up at the matching level: dense regions get their band
  average instead of noise, and moving images shimmer less before any AA sample arrives.
  A pixel made of N samples (AA, the supersampling grid) filters each sample over `1/√N` of
  the pixel, not the whole of it: filtering every sample over the full pixel and averaging
  them filters twice and greyed fine rings ([Anti-aliasing and supersampling](anti-aliasing.md), 2026-10-16).
- **Order: outline, then light.** The DE outline (boundary emphasis and filament coverage)
  darkens towards the edge colour first, also where the palette filter averaged more than
  1/8 cycle per pixel; lighting (Lambert on Chéritat's normal lifted to 3D, plus a
  Blinn-Phong highlight, normalised so it redistributes rather than dims) comes after, so
  dark filaments keep their relief. In very dense regions almost every pixel is within a
  pixel of the set; their AA average of dark lace and bright background reads as a
  metallic grey, which we kept.
- **Cycling and depth tint.** Colour cycling animates the palette phase only while the view
  rests (cycles per second, off by default); depth tint shifts the phase per decade of zoom.
- **Headless tools** (render, bench, verify, regress) keep the plain M2.5 colouring unless
  `--look` is given, so benchmark timings stay comparable (lighting needs DE: +30–50 % ALU).

## Interior colouring as built

Built 2026-10-14 (`crates/almondbread-render/src/shaders/mandelbrot.metal`, `kInteriorColor`;
CLI `--inside flat|bubbles|period|angle|distance`, control center → Interior). Every mode but
flat needs the attracting cycle of each interior pixel, so the iterate kernel analyses it.

**Atom domain during iteration.** The pipeline with the analysis tracks the iteration `q` of
the smallest `|z_n|` (the atom domain: the `N` minimising `|Z_N|`, Peitgen and Richter;
src: [Mu-Ency: Atom Domain](../sources/muency-atom-domain.md) §Definition) with one compare per exact step, in floatexp
where `|Z|` is tiny (deep kernel). BLA blocks are not looked into; they are never valid where
`|Z|` is small, which is where the minimum is (inference).

**Cycle analysis after the interior test.** A pixel proven interior (`|w| n² < 1e-3`, see
[Interior detection](interior-detection.md)), or at the limit with the test off, keeps iterating from
its current point `z_s`, now close to the cycle, with exact perturbation steps in floatexp
(any depth, no BLA). The period `p` is the first `k` that **divides q** at which the orbit
closes, `|z_{s+k} − z_s| ≤ 1e-3 |z_s|`, **and** whose multiplier is attracting,
`|λ_k| = |∏ 2 z| < 1`. Our design; mathr takes new minima of `|z_n|` as period candidates and
verifies each by Newton (src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §atom domain partials). Two
pitfalls found while building it:

- *Closure alone is wrong at depth.* At sea-8007 (period 8007) 1501 of 1502 interior pixels
  reported period 39: near a repelling or parabolic cycle a deep orbit returns within any
  fixed relative tolerance, and near the parabolic seahorse points even `|λ_39| < 1` holds.
  Restricting candidates to divisors of `q` fixed it (8007, with bulbs at 16014, 24021, …);
  the needle-1e305 minibrot gives 255 and its bulbs 510, 765, … (measured 2026-10-14, M1 Pro).
- *Off by one.* Checking closure only below `k = q` misses period `q` itself (12 336 of
  70 540 home-view interior pixels had no period); the check at `k = q` comes before the
  restart.

No closure by `q`: restart from where it stopped (the start may not have settled: slow
convergence near a component's boundary), up to `max(2q, q + 4096)` steps in all, then give
up (period 0: drawn flat). Near a component's boundary `q` is a multiple `b·p` (the orbit
comes closest when `λ^b ≈ 1`), which still contains `p`.

Along the period it accumulates (old values on the right, `dz = λ` from 1, the others from 0;
src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md) §Practical interior distance rendering):

```text
dcdz <- 2 (z dcdz + dc dz)
dzdz <- 2 (dz^2 + z dzdz)
dc   <- 2 z dc + texel          // per texel, so the DE comes out in texels
dz   <- 2 z dz                  // the multiplier lambda after p steps
```

and the interior distance `(1 − |λ|²) / |dcdz + dzdz dc / (1 − λ)|`
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Interior distance estimation), plus
`log2 min |z|` over the cycle. Mathematically any cycle point serves as `z0`; ours is the
settled orbit point, accurate to the closure tolerance (inference).

**Storage, cost, unresolved.** The analysis state lives in a separate 48-byte-per-slot buffer
parallel to the live list, bound only in `kInteriorColor` pipelines, so the others are
unchanged (bit-identical). Interior texels write `.y = p` in the iteration texture,
`(|λ|, arg λ / 2π)` in the DE texture and `(distance in texels, log2 min |z|)` in an extra
rg32f texture allocated on first use. The class never changes (shown-frame checks and verify
compare classes). Unresolved pixels keep the outline colour, distinct from every mode.
Switching between non-flat modes only recolours; flat ↔ analysed renders again.

**Modes** (colorize): *bubbles* palette by `−log2 |z*| · density` (rings around each nucleus,
where `z*` → 0); *period* golden-ratio steps through the palette per period, brightened by
`√(1 − |λ|²)`; *angle* palette once around `arg λ` with stripes `cos(2π s arg λ)` along the
internal rays (faded where they converge) and rings of equal `log(1 − |λ|)`; *distance* palette
by `log2(1 + d_px)`, dark within ~6 px of the boundary.

**Cost** (measured 2026-10-15, M1 Pro, release, 960×600, `--extend 16`): see
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "Interior colourings, overlays, control center". Under 1 % more
iterations at home, about 3 % more loop steps at sea-8007 and needle-1e305 (one period of floatexp steps per interior pixel).

## Dense regions (as built)

Reported by the user: the default look filled the near-boundary band with grey noise where
a reference renderer shows calm dark blue with thin bright filaments. Cause: where the palette
phase changes by more than ~1/16 cycle per pixel (`footprint = density / (d ln 2)` cycles per
pixel at DE distance `d`, see palette anti-aliasing above), point lookups are per-pixel noise
and the filtered palette is its mean, a grey. Fix (2026-10-14, `shade`, setting
`dense_calm`, on by default, "Calm dense regions" in the control center): from 1/16 to 1/2
cycle per pixel the colour fades to a per-palette *deep* colour (the palette's darkest third,
averaged and brought to luminance 0.012, so it keeps the palette's hue), with a lift towards a
*filament* colour (its brightest tenth at luminance 0.35) for the farthest of those pixels
(`smoothstep(0.2, 0.7, d_px) · 0.5`); the outline and 80 % of the lighting are faded out
there, since both are noise at that density. Unresolved pixels (outline colour, near black)
and interior stay distinguishable from the deep colour. Works for any palette given as stops
(`palette::dense_colors`). Off restores the old look. Before/after stills:
`scripts/render-dense-looks.sh`.

## Density pivot (as built)

Reported by the user (2026-10-21): moving the Density slider a tiny bit made the picture
flicker and scramble, worst at depth (Pastel palette). Cause: the colour pass maps
`t = fract(n · density) + s · density + offset` (log scale: `ln(mu + 1) · density + offset`),
so density stretches the palette around mu = 0, which at depth is nowhere in view. With escape
counts of 1e4–1e6, one point of slider travel (≈3 %, the log slider spans three decades)
moves each pixel by `n · Δd`, hundreds of cycles, differently for neighbouring counts.

Fix: a user edit of the density pivots on the view's own counts. When the density goes from
`d1` to `d2`, the offset becomes `offset + n_ref (d1 − d2)` (log scale: `ln(n_ref + 1)` in
place of `n_ref`), wrapped into [0, 1), so the colour at `n_ref` stays; a pixel 100 counts
away moves by `100 · Δd` cycles. Bands widen or narrow around the main colour, like zooming the
gradient, and the Offset slider moves along.

- `n_ref`: the median smooth count `n + s` of the escaped texels of the shown view. The
  renderer samples every finished level of a resting view (no gesture frames, AA or extension
  passes) with the `escapeSample` kernel, one texel per cell of a 64 × 40 grid, in a command
  buffer of its own (like the autopilot survey), and takes the median when it has run
  (`almondbread_core::density_pivot::escape_pivot`; fewer than 16 escaped samples: none). One
  tiny pass per finished view, nothing per frame, no GPU wait on the main thread.
- Rule: `density_pivot::pivoted_offset` (core), exported as `pivoted_color_offset`;
  `Explorer::density_pivot()` hands out the cached `n_ref`. Without one (all interior, nothing
  finished yet) the offset is unchanged.
- Captured when the drag starts (`Slider`'s `onEditingChanged`), so one drag keeps one
  anchor even if a level finishes mid-drag; a change without a drag (keyboard steps on the
  focused slider) takes the current pivot. Escape counts do not depend on colouring, so a
  density change starts no new job and the pivot stays valid while dragging.
- Only the slider pivots (`ExplorerModel.editColorDensity`); looks, palettes, places, links,
  tours and reset set density and offset as stored, and saved values stay ordinary values.
  The Julia panel uses the same settings: its colours turn uniformly (its counts are small).
- Measured (2026-10-21, `almondbread render out.png --density-edit d`, 1200 × 800, +3 %):
  spiral-galaxy (n_ref 1461): mean |ΔRGB| 7.4 pivoted vs 65.1 plain, pixels moved by more than
  32: 11 % vs 74 %; dense-5e-14 Pastel (n_ref 2773): 36.7 vs 103.8, 65 % vs 97 % (the moved
  pixels are its noise bands; the large flowers keep their colours); the same with log scale
  (density 0.5): 0.4 vs 30.4, 0 % vs 87 %.

## For almondbread

v1: smooth `(n, frac)` → log transfer → periodic cosine palette, plus DE
darkening and slope lighting from stored `u` direction. All in a separate
colouring pass over stored per-pixel data (n, frac, de_px, u, arg z, flags), so
palette edits are instant. Histogram colouring as an option via GPU
reduction. Per-iteration averages later, restricted to the post-skip tail.
Related: [Anti-aliasing and supersampling](anti-aliasing.md), [Exponential map (log-polar) rendering](exponential-map.md).

# Citations

- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
- [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md)
- [Wikibooks: Triangle inequality average (TIA) colouring](../sources/wikibooks-triangle-inequality.md)
- [Wikibooks: Mandelbrot set interior](../sources/wikibooks-mandelbrot-interior.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [zoomasm (zoom video assembler)](../sources/zoomasm.md)
- [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)
- [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md)
- [Mu-Ency: Atom Domain](../sources/muency-atom-domain.md)
