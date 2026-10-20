---
title: "Sample reuse while zooming and panning (sample cache in fractal coordinates)"
type: technique
status: reviewed
sources: ["[[sources/xaos-algorithms]]", "[[sources/wikipedia-xaos]]"]
tags: [frame-reuse, sample-reuse, temporal, interactive, xaos, measured]
---

# Sample reuse while zooming and panning

> **Code removed 2026-10-17.** The sample cache was opt-in (`ALMONDBREAD_SAMPLE_CACHE=1`) and never
> the default; `sample_cache.rs`, its kernels, tests and switches are deleted. This page keeps
> the research and the measurements. "As built" below describes the deleted code.

Keep the point samples a gesture computed, move them with the view, and compute new samples
only where the image lacks them. A sample of the Mandelbrot set is a pure function of its
position `c` (and the iteration limit), so a sample computed in an earlier frame is **exact**
in every later frame; only where it sits in the new pixel grid changes. R4 measured this
against the FL pipeline, which recomputes a fresh jittered preview every gesture frame and
reuses only the previous *image* ([Frame reuse and progressive refinement](frame-reuse-and-progressive.md) FL). Results
and recommendation: [Motion sample reuse: fewer iterations per gesture frame (R4)](../synthesis/motion-sample-reuse.md).

## XaoS: approximate reuse of rows and columns

XaoS (Hubička, 1990s) keeps only the previous frame and remembers one real coordinate per
column and one imaginary coordinate per row, not per pixel (src: [XaoS: Algorithms document](../sources/xaos-algorithms.md)
§Saving Previous Pixels; [Wikipedia: XaoS](../sources/wikipedia-xaos.md) §XaoS). For each new frame it matches old
rows/columns to new ones by dynamic programming in linear time: a reuse costs the squared
distance from the ideal position, a fresh row/column costs `(4·step)²`, matches must stay in
order (no doubling, no swaps), at most ~8 candidates per old line (src:
[XaoS: Algorithms document](../sources/xaos-algorithms.md) §Approximation Algorithm). Unmatched lines are computed fresh at
the mean of their neighbours' positions, so displayed rows and columns are **not** at their
ideal positions: XaoS trades a positional error of up to a few pixel steps for computing about
`xsize + ysize` samples per frame instead of `xsize × ysize`
([Frame reuse and progressive refinement](frame-reuse-and-progressive.md) XaoS). Errors show as distortion; doubling makes the
image jagged (src: [XaoS: Algorithms document](../sources/xaos-algorithms.md)). Dynamic resolution computes the middle of the
largest uncomputed block first and raises rows near the zoom centre (src:
[XaoS: Algorithms document](../sources/xaos-algorithms.md) §Dynamic Resolution).

## Exact positions instead of approximate ones

A GPU does not need the row/column restriction. Store each sample with its position and its
raw kernel outputs, and draw it where it really is:

- **Position**: exact. A pan/zoom maps frame A's pixels to frame B's by
  `x_B = (x_A − C)·a + C + b` (C the view centre, `a = s_A / s_B`, `b` the centre offset in B's
  pixels, taken from the `BigComplex` centres with `delta_in`, so at any depth). Store each
  sample's birth position (its texel + jitter in the birth frame, exact in float32) and the
  birth frame's map; recompute its current position from the birth frame each frame instead
  of moving it frame by frame, so float32 error does not accumulate (~1e-4 px at 3456 px,
  never growing). Prototype: positions in f64 re-expressed each frame.
- **Data**: escape count `n`, smooth offset, distance estimate, normal angle, class
  (exterior / interior / unresolved), the limit it ran to. Colour is **not** stored: it is
  re-shaded every frame, because the look depends on the DE in *current* output pixels
  (outline width, palette filter level; [Colouring methods](coloring-methods.md) FL). FL's history
  image bakes the old pixel scale into colours; a sample cache does not.
- **Validity**: escaped samples are valid forever; derivative-proven interior samples too.
  Unresolved samples are stale once the limit has grown (prototype: evicted when the limit
  exceeds 1.25× theirs). Colour-setting changes cost nothing (re-shade).
- **Reconstruction** (each frame): Gaussian weights (σ = 0.5 px, as FL's compose) over the
  samples in the 3 × 3 neighbouring cells; fallback the nearest sample within 5 × 5. A box
  (mean of the samples inside the pixel) or the nearest sample (XaoS-like display) are worse:
  both alias (measured, synthesis page).
- **Density control**: a cell (output pixel) gets a new sample when it has fewer than `m`
  (fill policy), or the frame's iteration budget goes to the cells with the fewest samples
  first (budget policy); at most `K` samples per cell (newest kept). A zoom-in spreads the
  samples by `a²` per frame, so at a zoom rate of `r` e-folds/s and refresh `f` the steady
  state needs about `1 − e^(−2r/f)` new samples per pixel per frame for one sample per pixel:
  3.3 % at 1 e-fold/s and 60 Hz, 8 % at 2.5, 22 % at 7.6. A pan needs the exposed strip only
  (speed / width per frame). FL recomputes `ratio²` of the pixels every frame whatever the
  motion.
- **Hybrid reconstruction**: where samples are sparse (fast zoom on a small budget), blend
  the cache's reconstruction with its own reprojected previous output (Catmull-Rom, weight
  0.3 × FL's zoom-in decay), so gaps fill smoothly instead of showing the nearest sample.

## Positional error, quantified

With exact positions there is no positional error: every displayed sample sits where it was
computed; what remains is reconstruction from irregular samples, which the Gaussian handles.
The XaoS-like variant (show the sample nearest the pixel centre; compute a new centre sample
when no cached sample lies within ε of the centre, per axis) has positional error up to ε:
measured in [Motion sample reuse: fewer iterations per gesture frame (R4)](../synthesis/motion-sample-reuse.md) (`xn-*` rows). Its aliasing, not ε, dominates.

## Interaction with temporal accumulation and AA

The cache *is* the temporal accumulation, of exact samples instead of resampled colours:
no Catmull-Rom blur per frame, no confidence heuristics. Under a pan or slow zoom it keeps
gaining samples per pixel (up to K) while moving, i.e. it anti-aliases in motion. When the
view rests, the cached samples inside each pixel are legitimate jittered AA samples for the
settled image: the idle AA (target 9 per pixel, [Anti-aliasing and supersampling](anti-aliasing.md) FL) can count them
and compute only the remainder.

## The "shown frame = fresh render" check

The check compares the refined (centre-sample) level with a fresh render
([Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md), `displayed_frame_mismatch`); gesture frames are already
exempt. Proposal: keep the refined level computed fresh at pixel centres (the check is
unchanged), and add a **cache check**: re-iterate a random subset (e.g. 4096) of the cached
samples from scratch at their recomputed birth positions with a fresh renderer and require
identical class and `n` (bit-identical `smooth`, DE, angle). A wrong transform, a stale
sample or a mis-evicted one changes `n` in chaotic regions immediately, so the check catches
the bugs the frame check would have. Samples reused as AA samples go through the same check.

## Measured (R4)

Measured 2026-10-15 on an M1 Pro, release builds, in a deterministic simulator of five flights
against 16-sample references. Full tables are in [Motion sample reuse: fewer iterations per gesture frame (R4)](../synthesis/motion-sample-reuse.md).

| strategy | iterations per gesture frame vs FL | quality vs FL |
|---|---|---|
| cache, one sample per pixel (fill) | pan at 1e5: 2.4 %; filament cruise: 15 %; fast dive at 7.6 e-folds/s: 32 %; seahorse dive: 36–43 % in its shallow phases | as good or better |
| cache at FL's own iterations (budget policy, hybrid fill) | 1× | ΔE 1.4–1.8× lower, equal or lower temporal error; no gain in the deep, sample-starved zoom past 1e-24 |
| XaoS-like nearest reuse (ε 0.5 px) | about the same as fill | 2–3× FL's temporal error, about 2× ΔE p99 (shimmer) |
| box reconstruction | same as fill | aliases like nearest reuse |

GPU bookkeeping, unoptimised prototype: 3.6–6.3 ms per frame at 1080p. It needs ~3× less
before the iteration savings turn into GPU time savings.

## As built in almondbread (production cache, 2026-10-15)

`crates/almondbread-render/src/sample_cache.rs`, kernels in `shaders/sample_cache.metal`,
hooks in `scheduler.rs`. Our design, built after R4; it departs from R4's design (per-pixel
cells holding K free-positioned samples, a move pass) because that bookkeeping was O(pixels)
and cost 19–23 ms per frame at Retina size (measured R4, [Motion sample reuse: fewer iterations per gesture frame (R4)](../synthesis/motion-sample-reuse.md)).

**A lattice fixed in the plane instead of free samples.** Lattice point `(i, j)` of a level
is `anchor + h · R(angle) · (i, −j)`. Samples sit only on lattice points, so a sample never
moves in its storage. One level is a **toroidal window** of the lattice: a W × H texture set
(iteration rg32f, DE rg16f, interior analysis when an interior colouring needs it) where
window texel `t` (lattice index `16·win0 + t`) is texel `(lattice index) mod (W, H)`.

- **Pan**: the window slides by whole 16 × 16 tiles. Entering tiles are cleared on the GPU
  (`scClear`, one thread per texel of the listed tiles) and planned for computing. Nothing
  else moves.
- **Zoom**: when the spacing leaves its band of texels per output pixel (0.45–1 × the target
  ratio), the level switches. The coarser level's points are every other point of the finer
  one. `scResample` builds the new window from the old one: a finer level takes the old
  samples at even indices (a quarter of its points), a coarser one takes every other old
  sample (all of its points). Distance estimates are in texels, so they scale by 2 or 1/2.
  The anchor moves to a tile-aligned lattice point near the view centre at each switch, so
  lattice indices stay small.
- **Rest**: the refined frame's full level seeds a new epoch (its texel centres form a lattice
  at the pixel spacing; above the ratio cap it is decimated). A gesture from rest starts with
  every sample. Tiles started from local SA seeds (M5) or levels iterated through the LA
  table (M6) are left out: they are another regime.
- **Iterate**: the cache level is an ordinary level of the chunked scheduler with a tile list
  as its admission order (`tileOrder`). The only kernel change is the position: texel `gid`
  holds window texel `(gid + wrap) mod size` (`IterateParams.wrap`, 0 for every other level).
  `halfSize = −(lattice index of window texel 0)`, jitter 0, `rot` = the lattice angle.
- **Bookkeeping is per tile on the CPU**: holder (lattice tile), state (empty, partial,
  full), the limit its samples ran to, the batch computing it, and whether the colour texture
  holds it shaded. The visible scan, the slide and the plan are O(visible tiles), a few
  thousand, never O(pixels).
- **Shade and compose**: `scShade` colours the visible tiles of the window (premultiplied:
  alpha 0 marks no sample) with the current colour parameters. During a pan the colours do not
  change, so it shades only tiles whose samples changed (`scShadeTiles`). Threadgroups are tiles
  and also write per-tile statistics (escape-count sum, escaped, interior, unresolved, largest
  escape count). The compose pass treats the cache like FL's jittered preview: Gaussian point
  samples from the 2 × 2 lattice points around a pixel, the reprojected history, and a
  bilinear base. The mapping turns by the view angle minus the lattice angle. Where fewer than
  four samples surround a pixel it takes the history, or else a small Gaussian over the 4 × 4
  points around.
- **Budget and density**: a target ratio steps down from the cap (√2 texels per pixel, lowered
  so the visible window holds at most 4.2 M texels; cap/32 at most) with hysteresis. The
  demand model counts exposed area and level switches (zoom in: 1.08 × the visible texels
  per e-fold). Batches are the priority prefix (empty before partial before stale, then
  distance to the focus) whose estimated GPU time fits the gesture budget. The estimate is
  per-tile iteration units from the shade statistics × a cost per unit calibrated from
  finished batches, plus a latency term from the batch's slowest pixel. At most two batches
  are on the GPU. A batch runs **one chunk per frame**, with K sized so that its slowest
  chains fit the budget (GPU seconds per loop step measured on chunks that left pixels live).
  Unfinished pixels **carry over** into the next frames' jobs (the level moves on with its
  live state) unless a slide reassigned one of the batch's tiles or the level switched.
- **Exactness**: an epoch pins its regime (deep kernel, BLA, LA, interior test on, DE,
  interior analysis). A regime change starts a new epoch, and a jump resets the cache (M5 jump
  rule).
- **Where fresh previews win**: gestures that start from a view whose refined frame stepped
  through the LA table (M6, deep nucleus references) render fresh previews. Such previews
  start from SA seeds and take LA blocks; cache tiles, exact samples from scratch, took ~100×
  the loop steps per sample there (1e-305 needle). The cache left the panned-in area empty
  where fresh previews kept 55 fps.
  References may change within an epoch: a sample does not depend on the reference beyond
  float32 rounding of `dc` (below). Tiles whose limit fell 25 % behind the view's are
  recomputed.
- **Interior-test statistics**: a cache frame's new tiles are a biased sample of the view.
  The interior test switch takes the view's statistics from the per-tile shade statistics,
  and may only be turned on by them (escapes past the limit from an extension pass count as
  unresolved). Before this fix the settled frame differed from a fresh render by 0.1 %.

**The cache check** (`Renderer::sample_cache_check`, run by `verify --history` after the
gesture and once settled): every cached sample against a fresh offscreen render of the same
lattice points with the live reference and the epoch's regime. With one reference throughout
a gesture, no sample differs in class and escape counts match up to float32 rounding of the
sample position: the test `sample_cache_exact` allows < 0.5 % of escape counts, and the zoom
step measured 0.08 %. The lattice point's `dc = i·h + (anchor − reference)` rounds
differently from the check render's (other centre), as two fresh renders centred differently
do. Across references the escape counts differ as much as the session's own refined frame
differs from a fresh render (0.2–55 % in dense views, the reference's float32 noise), so
`verify` gates escape counts relative to that noise and class strictly (0.05 %). A one-texel
position error shows as 5–6 % in a smooth view, a 0.1-texel one as 2 % (tested 2026-10-15).

Measured (M1 Pro, release, 2026-10-15; [almondbread benchmarks and verification record](../synthesis/benchmarks.md) "Sample cache"):

- **Iterations per gesture frame**, against fresh previews: shallow −56 % at 1600 × 1000 and
  −92 % at 3456 × 2160; 1e-14 −55 % and −92 %. In the 1e-19 dense spiral −99.9 %, but the
  cache then fills only ~40 texels a frame.
- **GPU iterate time per frame**: 1–2 ms instead of 4–11 ms.
- **Frame rate** (120 Hz refreshes at Retina size): shallow 106 vs 74 fps, spiral 110 vs
  13 fps. At 1600 × 1000 (60 Hz) equal, except the spiral: 58 vs 11 fps.
- **Bookkeeping** (CPU planning and encoding, maintenance passes): 0.1–0.4 ms per frame,
  with single-frame CPU spikes to 3–9 ms on the loaded machine.
- **Colour passes**: shade 0.03–0.9 ms. The compose pass costs ~1 ms more than FL's preview
  colour plus compose at Retina size (3.0–3.1 vs 2.1 ms), and the same at 1600 × 1000.

## User test: off by default (2026-10-15)

In the real foreground window on the M1 Pro (Retina, ProMotion) the user found the app
"very sluggish" with the cache on. Zoom-out frames were very blurry and fast zoom-ins less smooth
than the build without the cache. Switching features off one at a time (env switches, same
build) pinned it on the cache alone: M5 local SA and M6 LA were as smooth as or smoother than the
day before. Headless `fly` replays at 3456×2160 had shown no difference (96–99 fps either way), and
background window tests are capped near 30 Hz, so neither check could see it. The cache is now
opt-in (`ALMONDBREAD_SAMPLE_CACHE=1`, since removed). It must not return as the default until a foreground
measurement and the user's own check show that zoom-out stays sharp and fast zooms feel smoother.

## Pitfalls

- **Use a reconstruction filter, never the nearest sample.** A sample replaced under a pixel
  moves edges by its positional error, and that reads as shimmer.
- **Iteration limit.** Samples run to their birth frame's limit. Evict unresolved samples on
  any limit growth.
- **Sparse samples.** On a starved budget the cache is grainy where FL is blurry. Fill gaps
  from the reprojected output (hybrid).
- **Superseded jobs waste batches.** Every gesture frame is a new job, so a batch split into
  budgeted chunks was dropped after its first chunk, and the same tiles were planned again
  forever (1e-305 needle: 9 ms GPU per frame, little progress). Batches carry over.
- **Throughput is not latency.** A batch of cheap tiles plus a few pixels running to a
  200 000 limit took 0.3–1.1 s (needle, deep kernel without SA seeds), although its total
  work fitted the budget. The planner bounds the slowest pixel too.
- **Biased statistics.** Interior-test and cost statistics from the computed tiles alone
  turned the interior test off after a pan; use the whole cached view.
- **Integer division in per-pixel code.** Wrapping texel indices with `%` cost about 0.5 ms
  of the compose pass at Retina size; a conditional subtraction does it.
- **Apple GPUs overlap command buffers.** A compose next to a 20 ms chunk reports 20 ms GPU
  time. Pass costs needed a serial-compose measurement mode (`ALMONDBREAD_SERIAL_COMPOSE`, removed 2026-10-17).

# Citations

- [XaoS: Algorithms document](../sources/xaos-algorithms.md)
- [Wikipedia: XaoS](../sources/wikipedia-xaos.md)
