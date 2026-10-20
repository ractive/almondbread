---
title: "Motion sample reuse: fewer iterations per gesture frame (R4)"
type: synthesis
date: 2026-10-15
sources: ["sources/xaos-algorithms", "sources/wikipedia-xaos", "sources/apple-metalfx-docs", "sources/wwdc22-metalfx-upscaling", "sources/wwdc25-metal4-games", "sources/apple-metal-feature-set-tables", "sources/apple-metalfx-taa-sample"]
tags: [frame-reuse, sample-reuse, temporal, metalfx, interactive, measured, decision]
---

# Motion sample reuse: fewer iterations per gesture frame (R4)

> **Code removed 2026-10-17.** The production sample cache this page recommends and describes "as built" was opt-in, never the default, and is deleted ([Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md) has the note and what stays). The research and measurements below are history.

**Question.** While zooming and panning continuously (Space at 1–8 e-folds/s, WASD,
autopilot), how many pixel-iterations per displayed frame can we avoid while the shown image
stays as sharp and stable as now or better? FL computes every gesture frame's samples fresh
(a jittered preview at a dynamic ratio) and reuses only the previous *image*
([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md) FL). Candidates: XaoS-style sample reuse
([Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md)) and Apple MetalFX upscaling ([MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md)).

**Answer.** Reuse the samples, not the image. A **sample cache in fractal coordinates** keeps
computed samples with their exact position and raw data, moves them with the view, re-shades
them every frame, reconstructs with a Gaussian, and computes new samples only where pixels
lack them ([Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md)). It gives today's shown gesture image **at 2 % (pans)
to 50 % (zooms) of FL's iterations per frame**. At FL's iterations it gives a clearly sharper
image (OKLab ΔE 1.4–1.8× lower) with equal or lower temporal error, in every flight except the
deep zoom beyond 1e-24. There, FL's budget (preview ratio 0.125) is far below what any reuse
needs, and the cache gains nothing at equal iterations; cheaper samples (M5 local SA) have to
come first. XaoS-style nearest-sample reuse, which accepts a positional error instead of
filtering, aliases: 2–3× FL's temporal error. **MetalFX does not help**: at the same input
its temporal scaler at best matches our compose (worse on pans); it stops at 3× per axis while
our deep previews run at 8×; and it costs 1–1.5 ms at 1600×1000 and 5–7 ms at Retina
resolution. The catch for the cache is its own GPU bookkeeping. Unoptimised, it costs
2.3–2.8 ms per frame at 1280×720, 3.6–6.3 ms at 1920×1080 and 19–23 ms at 3456×2160, against
1.7 ms for FL's compose at 1080p. Before it saves GPU time rather than only iterations, it
needs a ~3× faster move pass and one cell per point on Retina displays.

## Method

Prototype `prototypes/motion-reuse/`; raw notes and all tables are in
`kb/raw/research/motion-sample-reuse/experiments.md` and `runs/`. (It does not build against
main, checked 2026-10-19: the renderer hooks it needs, `Renderer::offscreen_jitter` and
`OffscreenResult::de`, stayed on branch `worktree-agent-abad90e6e8bdb3fc5`, and `Viewport`
has gained `angle` since.) It is a deterministic flight simulator:

- The app's `Navigator` is stepped at 60 Hz through the scripted flights.
- For every gesture frame the real GPU kernel renders the fields each strategy needs: fixed
  limit `depth_iterations`, Halton jitter as in the scheduler.
- The strategies run on the CPU with a port of `shade` (default look).
- Every 10th frame is compared with a converged reference (4 × 4 grid per pixel).

Metrics:

- Exact iteration counts of the samples each strategy computes.
- OKLab ΔE × 100, mean and p99, against the reference.
- **Temporal error**: the change of a frame's error against its reference from the previous
  frame's error, warped along the exact motion. It is 0 for a strategy that equals the
  reference, and it cancels the warp's own resampling error, which made plain frame
  differences useless.

FL's preview ratio follows a deterministic model of the dynamic resolution, calibrated to a
real `fly` run: on the seahorse dive the model averages ratio 0.452 and the real run 0.453,
and both sit at 0.125 below zoom ~1e6. Measured 2026-10-14/15, M1 Pro, release.

Strategies:

| name | what it does |
|---|---|
| `fl` | FL compose port at the model ratio |
| `fl-full` | FL with a full-resolution preview every frame |
| `fl-r*` | FL at fixed ratios |
| `sc-fill1` / `sc-fill2` | cache; a new sample wherever a pixel has fewer than 1 / 2 |
| `sc-b*` | cache spending a multiple of FL's iterations for this frame, emptiest pixels first |
| `hy-b*` | the same with hybrid reconstruction: gaps filled from the cache's own reprojected output |
| `xn-0.5` | XaoS-like nearest reuse within 0.5 px |
| `mfx-t*` / `mfx-s*` | MetalFX temporal / spatial on the same jittered preview |

## Results per flight

Each cell gives iterations per gesture frame relative to FL, then ΔE mean / p99 / temporal
error. The seahorse dive ran at 1280×720, the others at 960×540. Where `sc-fill1` and
`hy-fill1` differ (by ≤ 0.02), the cell shows `hy-fill1`.

| flight (FL mean ratio) | FL | full res every frame | sc-fill1 | sc-fill2 | hy-b0.5 | hy-b1 | xn-0.5 |
|---|---|---|---|---|---|---|---|
| seahorse dive 1 → 1e9 (0.45) | 1, 2.07 / 15.7 / 0.84 | 13.9×, 1.08 / 9.3 / 0.73 | 1.31×, 1.18 / 9.9 / 0.72 | 2.39×, 1.03 / 8.7 / 0.67 | 0.50×, 1.81 / 14.1 / 0.81 | 0.95×, 1.48 / 11.8 / 0.78 | 1.46×, 2.19 / 19.6 / 1.93 |
| pan at 1e5 (0.64) | 1, 0.99 / 9.7 / 0.57 | 2.6×, 0.77 / 7.7 / 0.53 | **0.024×**, 0.93 / 8.8 / 0.50 | 0.054×, 0.76 / 7.3 / 0.45 | 0.48×, 0.74 / 7.0 / 0.47 | 0.60×, 0.72 / 6.9 / 0.47 | 0.024×, 1.75 / 16.2 / 1.45 |
| filament cruise at 1e6 (0.36) | 1, 1.65 / 13.8 / 0.79 | 7.8×, 0.99 / 8.8 / 0.69 | **0.15×**, 1.11 / 9.7 / 0.66 | 0.30×, 0.94 / 8.3 / 0.60 | 0.50×, 1.00 / 8.5 / 0.63 | 0.99×, 0.94 / 8.1 / 0.62 | 0.18×, 2.11 / 17.8 / 2.04 |
| fast dive 7.6 e-folds/s (0.87) | 1, 0.41 / 7.8 / 0.22 | 1.5×, 0.37 / 7.1 / 0.21 | **0.32×**, 0.33 / 6.8 / 0.21 | 0.56×, 0.31 / 6.2 / 0.21 | 0.48×, 0.32 / 6.5 / 0.21 | 0.83×, 0.29 / 6.0 / 0.20 | 0.33×, 0.58 / 12.2 / 0.59 |
| deep zoom 1e-24 → 1e-33 (0.125) | 1, 5.79 / 26.1 / 2.30 | 64×, 2.34 / 11.4 / 1.73 | 7.0×, 2.91 / 14.2 / 1.80 | 12.5×, 2.48 / 12.1 / 1.68 | 0.50×, 7.21 / 35.0 / 2.14 | 1.00×, 5.92 / 29.8 / 2.20 | 7.2×, 5.42 / 27.8 / 4.63 |

The seahorse dive by zoom phase:

| phase | FL | sc-fill1 | hy-b0.5 | hy-b1 | full res |
|---|---|---|---|---|---|
| 1 → 1e3 (FL ratio ~0.6) | 1, 0.78 / 10.6 / 0.40 | **0.36×**, 0.61 / 8.7 / 0.38 | 0.50×, 0.59 / 8.3 / 0.37 | 0.91×, 0.52 / 7.4 / 0.36 | 4.0×, 0.53 / 7.7 / 0.38 |
| 1e3 → 1e6 (~0.6) | 1, 1.99 / 14.4 / 0.97 | **0.43×**, 1.42 / 10.3 / 0.90 | 0.50×, 1.35 / 9.7 / 0.86 | 0.95×, 1.22 / 8.9 / 0.83 | 4.3×, 1.33 / 10.2 / 0.90 |
| 1e6 → 1e9 (0.125–0.25) | 1, 3.53 / 22.5 / 1.20 | 3.4×, 1.59 / 10.9 / 0.93 | 0.50×, 3.56 / 24.4 / 1.24 | **1.00×**, 2.75 / 19.3 / 1.19 | 36×, 1.43 / 10.3 / 0.96 |

**How many iterations can go.** This is the cheapest cache variant that is no worse than FL in
ΔE mean, ΔE p99 and temporal error:

| flight | share saved | cheapest variant |
|---|---|---|
| pan at 1e5 | **98 %** | sc-fill1, 0.024× |
| filament cruise | **85 %** | 0.15× |
| fast dive | **68 %** | 0.32× |
| seahorse dive, whole flight | **50 %** | hy-b0.5 |
| seahorse dive, shallow phases | 57–64 % | |
| seahorse dive, deep phase | **0 %** | hy-b1 at 1.0×, much sharper (ΔE 2.75 vs 3.53) |
| deep zoom beyond 1e-24 | **none** | |

In the deep zoom no variant at ≤ 2× is as good on every metric. hy-b1 at 1.0× is equal within
2–15 %: sharper, but noisier.

**Full-resolution quality at a fraction of the cost.** sc-fill2 matches the ΔE of a
full-resolution preview every frame within ±6 % at 1/3 to 1/50 of its iterations. The ratio is
1/3 in the fast dive, where FL is already near full resolution, and 1/5 in the deep zoom.

**Why.** A zoom-in spreads the samples by `a²` per frame. To keep one sample per pixel in
steady state, a zoom at `r` e-folds/s and refresh `f` needs `1 − e^(−2r/f)` new samples per pixel
per frame:

- 3.3 % at 1 e-fold/s and 60 Hz;
- 8 % at 2.5 e-folds/s;
- 22 % at 7.6 e-folds/s (the fast dive measured 26 %, edges included).

A pan needs only the exposed strip, ~1 % per frame here. FL recomputes `ratio²` of the pixels
every frame whatever the motion: 1.6–75 % in these flights. Where FL's budget is below the
steady-state need (deep dense views: 1.6 % against 8 %), reuse cannot keep up and the image is
sample-starved either way.

### Positional exactness and XaoS

Reused samples are exact for their own positions. The cache draws them where they are, so its
only error is reconstruction. Showing the nearest sample at the pixel centre instead is
XaoS's approach, here with per-sample rather than per-row positions and a 0.5 px tolerance. It
is 2–3× worse in temporal error than FL in every flight and roughly doubles ΔE p99 (pan: 16.2
vs 9.7). The positional error moves edges by up to half a pixel each time a sample is
replaced, which reads as shimmer.

The first, low-budget seahorse run (FL ratio 0.27) adds two more variants:

- A 0.25 px tolerance cost 19.5× FL's iterations and was sharper (ΔE 1.81 vs 2.51), but twice
  as unstable (temporal 1.78 vs 0.89).
- A box filter over the samples inside each pixel aliases the same way (temporal 1.68 vs 0.73
  for the Gaussian).

### Crops at 100 % (looked at)

| view | what the crops show |
|---|---|
| seahorse at zoom ~5e8 | FL (ratio 0.125) is a soft blur of the spirals; hy-b1 at the same iterations is sharp but grainy; sc-b1 (no history fill) shows blocks of nearest samples where samples are sparse; sc-fill1 is indistinguishable from the reference |
| pan at 1e5 | FL visibly soft; sc-fill1 (2 % of FL's iterations) sharp; xn-0.5 stair-stepped |
| deep zoom at ~1e26 | FL a blur with speckle; hy-b1 noisy; sc-fill1 close to the reference |
| MetalFX, seahorse | temporal at 2× looks like our compose at 2×; spatial is grainy |

The crops are in `kb/raw/research/motion-sample-reuse/runs/crops/`.

## MetalFX

At the same jittered input, MetalFX's temporal scaler is at best equal to our compose:

| flight | ratio | our compose: ΔE / p99 / temporal | MetalFX temporal | MetalFX spatial |
|---|---|---|---|---|
| seahorse dive | 0.5 | 1.66 / 13.8 / 0.79 | 1.61 / 14.5 / 0.81 | 3.28 / 25.6 / 4.33 |
| seahorse dive | 0.375 | 1.83 / 15.1 / 0.81 | 1.92 / 17.1 / 0.86 | 3.66 / 28.2 / 4.73 |
| pan at 1e5 | 0.5 | 1.06 / 10.5 / 0.59 | 1.14 / 11.0 / 0.62 | 2.74 / 23.0 / 3.66 |
| pan at 1e5 | 0.375 | 1.19 / 11.8 / 0.60 | 1.51 / 14.5 / 0.66 | 3.12 / 26.1 / 3.98 |

Why it does not fit ([MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md) unless noted):

- Its history logic cannot use what we know exactly: sample positions, re-shading, sub-pixel
  motion of fractal detail. Its gain vanishes with fractional per-frame motion on
  high-frequency content.
- Its range ends at 3× per axis; our deep previews need 4–8×.
- It costs 1.0–1.5 ms at 1600×1000 and 4.7–6.6 ms at 3456×2160 (minimum over loaded runs),
  plus 41–278 MB.
- On iOS it needs A14+ and, in practice, iOS 17 (src: [Apple Metal Feature Set Tables (May 2026)](../sources/apple-metal-feature-set-tables.md)).
- The spatial scaler is far worse than our bilinear preview path.
- The newer denoiser and frame interpolator do not fit either: we have no normals or albedo,
  and interpolation adds a frame of latency (src: [WWDC25 session 211: Go further with Metal 4 games](../sources/wwdc25-metal4-games.md)).

## GPU cost of the cache's bookkeeping

Measured with synthetic content (`motion-reuse --scgpu 2`). The passes are: 12-byte samples,
K = 4; move by preimage gather; deficit list; insert; a tiled resolve that shades in
threadgroup memory. Medians over 120 frames, 3 rounds, repeated 4 times; spread ±5 %; the GPU
was shared.

| size | zoom 1.6 e-folds/s | zoom 7.6 | pan 13 px |
|---|---:|---:|---:|
| 1280×720 | 2.8 ms | 2.3 | 2.4 |
| 1920×1080 | 6.3 | 5.1 | 3.6–4.2 |
| 3456×2160 | 23 | 19 | 21–22 |

About half is the move pass (1080p: 3.2 ms) and half the resolve (2.7 ms).

- **The move is slow for unknown reasons.** A plain copy of 16 B per pixel takes 0.3 ms at
  1080p on the same GPU, so the move runs at ~10× the bandwidth bound. The gather's divergent
  loops are the suspect.
- **The resolve shades much more than FL.** It shades every cached sample (~2 M at 1080p),
  where FL shades only the preview. FL's compose plus preview colour takes 1.7 ms at 1080p
  (`fly`, [almondbread benchmarks and verification record](benchmarks.md) FL).
- **Today the cache trades iterations for bookkeeping about one to one at 1080p, and loses at
  Retina resolution.** FL iterates for at most half a refresh per gesture frame (≤ 8 ms at
  60 Hz), so saving 50–85 % of that is worth ~2–7 ms at 1080p. At equal GPU time, the cache's
  win is image quality.
- It becomes a net saving with the move at ≤ 1 ms and one cell per point (2 × 2 pixels on
  Retina, which brings 3456 × 2160 down to the 1080p cost).

## Recommendation

1. **Build the sample cache for gesture frames** (design below), in this order:
   1. GPU move, deficit list, insert, hybrid resolve.
   2. The budget policy inside the existing gesture budget.
   3. AA reuse at rest.

   Expected with the GPU budget unchanged: shallow and mid-depth flights get 1.4–1.8× lower
   ΔE (sharper) at equal temporal stability, and pans come near full-resolution quality. Deep
   dense flights stay equal until samples get cheaper. Alternatively, holding quality, 2–40×
   fewer iterations per frame saves battery on iPad and iPhone.
2. **Optimise the bookkeeping before shipping.** Target ≤ 1.5 ms at 1080p.
   - Move ≤ 0.5 ms: scatter with per-cell atomics, or a sorted list, instead of the preimage
     gather.
   - Resolve: shade each sample once into a compact colour buffer, then gather colours.
   - One cell per point on Retina displays.

   Measure frame rate and latency with `--interaction-test`.
3. **Do M5 local SA first, or together.** In deep dense views the cache is sample-starved: FL's
   budget covers 1.6 % of pixels per frame against the 8 % needed at 2.5 e-folds/s. Local SA's
   3–5× cheaper samples ([Dense boundary regions: fewer pixel-iterations for the same image (R3)](dense-region-sampling.md)) close most of that gap, and each
   sample then lives for many frames.
4. **Do not build**:
   - MetalFX upscaling: no quality gain over our compose, the 3× limit, the Retina cost and the
     iOS hardware floor.
   - XaoS-style nearest reuse: shimmer.

   Revisit MetalFX only for a non-fractal overlay, or if Apple's Neural Engine upscaler
   (WWDC26, M5 Pro/Max) changes the cost picture.

## Integration design (scheduler)

State (`SampleCache<B>` in the renderer, GPU-resident):

```text
slots[2]   K = 4 per cell (cell = output pixel; = point on Retina), ping-pong:
           birth (u8 ring index), pos (half2 offset in the birth texel), raw (float mu with
           class in sign, half DE in birth-texel units, half angle), 12 B
counts[2]  u8 per cell
births     ring of 256 recent views (CPU: Viewport; GPU: per ring slot the affine map birth px
           -> current px, recomputed every frame from BigComplex deltas in f64, uploaded as
           float a, b): positions never drift, any depth
```

A gesture frame replaces "fresh jittered preview + compose" with:

1. `scMove`: every cell gathers the samples whose current position (birth map ∘ birth
   position) falls in it, and scales their DE by the zoom. It drops samples that are older than
   the ring, out of view, or unresolved below the current limit. Drop unresolved samples on any
   limit growth, not at +25 % as the prototype did (see the deep-flight anomaly in the raw
   notes).
2. `scList`: lists cells with count < m (fill) in priority order: focus first, then exposed
   area, as `priority_order` does. The dynamic-resolution model becomes a new-sample budget,
   `cost_per_texel × listed ≤ gesture budget − bookkeeping`. Cells beyond the budget wait;
   hybrid fill shows the reprojected image there.
3. `iterate` the list: `listMode 2` admission of texel indices into the full level's textures,
   at this frame's Halton jitter. This is the mechanism the extension passes and the AA list
   already use.
4. `scInsert`: appends the finished texels of the listed cells to their slots.
5. `scResolve` (+ hybrid): a Gaussian with σ 0.5 over the samples of the 3 × 3 cells, shaded
   with the current colour parameters. Where the weight is low, it blends with the reprojected
   history (the history texture FL already keeps). It writes the drawable and the history.

Other cases:

| situation | handling |
|---|---|
| refinement at rest | unchanged (full centre level, fade); its samples also enter the cache |
| AA at rest | cached samples inside a pixel count toward the 9-sample target; new AA samples only for the remainder |
| colour change | re-shade, no recompute |
| reference change | samples stay valid (a sample does not depend on the reference) |
| change that alters samples (limit down, interior test, formula) | clear the cache |

**Check.** The refined level stays fresh at pixel centres, so `displayed_frame_mismatch` is
unchanged. Add a new `cache_mismatch`: re-iterate 4096 random cached samples at their birth
positions with a fresh renderer and require identical class and `n`. A transform, eviction or
stale-limit bug shows immediately in chaotic regions. Run it in `--interaction-test` after
each scenario and in `verify --history`.

**Memory.** 2 × (12 B × 4 + 1) ≈ 100 B per cell:

- 200 MB with one cell per pixel at 1920 × 1080;
- 180 MB on a Retina 16" with one cell per point;
- iPhone: K = 2 and one cell per point.

## Risks

- **Bookkeeping cost** (above): the whole gain depends on bringing it down ~3×. Measure it in
  the app, not in isolation.
- **Scattered admission**: new samples are scattered pixels, not a grid, so the iterate
  kernel's SIMD coherence drops (neighbouring lanes are no longer neighbouring pixels). Not
  measured; the list is in tile order, so lanes stay spatially close.
- **Limit growth**: samples run to the limit of their birth frame. Unresolved ones must be
  evicted on any growth (the deep anomaly); escaped and interior samples stay exact.
- **Grain at low density**: on starved budgets the cache is sharper but grainier than FL's blur
  (hy-b1 deep: p99 +14 %). The hybrid weight trades one against the other.
- **The simulator is not the app**: CPU port of the compose, deterministic budget model,
  960 × 540 / 1280 × 720, fixed limits, no extension passes; GPU numbers from synthetic content.

## Production (as built, 2026-10-15)

The cache was built ([Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md) "As built"), but not as designed above. The
design's per-pixel cells with K free samples and a move pass are O(pixels), and the
bookkeeping cost was the risk. The build keeps samples on a **lattice fixed in the plane**,
held as a **toroidal window** of 16 × 16 tiles:

- A pan clears and computes only the entering tiles.
- A zoom switches the lattice level and copies the coinciding samples in one GPU pass.
- Planning is per tile on the CPU, and the iterate kernel runs a tile list (`wrap` is its
  only kernel change).
- Reconstruction is the FL compose pass with the lattice as its new samples.

Measured ([almondbread benchmarks and verification record](benchmarks.md) "Sample cache"): bookkeeping is 0.1–0.6 ms per frame,
within the 1.5 ms budget. Iterations per gesture frame fall by 55–56 % at 1600 × 1000 (at up
to √2 × the preview resolution) and by 92 % at Retina size. Iterate GPU falls by 4–8 ms per
frame.

Departures from the plan:

- **No AA in motion.** One sample per lattice point; the history accumulates as in FL.
- **Deep dense views remain the open case.** In the 1e-19 spiral the cache keeps 58–110 fps
  where fresh previews manage 11–13, but it fills only ~40 texels a frame.
- **LA views use fresh previews.** In views whose refined frame uses LA (M6) the cache tiles
  lose to SA-seeded LA previews (~100× the loop steps per sample), so gestures there do not
  use the cache.
- **The compose pass is ~1 ms dearer at Retina size**, so the cache's total colour-side cost
  there is 1.2–1.5 ms per frame.

Open:

- Local SA seeds for cache tiles.
- Coverage of newly exposed areas in heavy views (a coarse level under the fine one).
- A cheaper compose at Retina size.

## Related

[Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md), [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md),
[Anti-aliasing and supersampling](../techniques/anti-aliasing.md), [MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md), [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md),
[Dense boundary regions: fewer pixel-iterations for the same image (R3)](dense-region-sampling.md), [Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md),
[almondbread benchmarks and verification record](benchmarks.md) (R4), [Roadmap](../roadmap.md).

# Citations

- [XaoS: Algorithms document](../sources/xaos-algorithms.md)
- [Wikipedia: XaoS](../sources/wikipedia-xaos.md)
- [Apple docs and SDK headers: MetalFX framework](../sources/apple-metalfx-docs.md)
- [WWDC22 session 10103: Boost performance with MetalFX Upscaling](../sources/wwdc22-metalfx-upscaling.md)
- [WWDC25 session 211: Go further with Metal 4 games](../sources/wwdc25-metal4-games.md)
- [Apple Metal Feature Set Tables (May 2026)](../sources/apple-metal-feature-set-tables.md)
- [Apple sample: Applying temporal antialiasing and upscaling using MetalFX](../sources/apple-metalfx-taa-sample.md)
