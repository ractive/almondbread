---
title: "Landscape mode: fly over the Mandelbrot set as 2.5D terrain"
type: decision
date: 2026-10-14
status: proposed
tags: [3d, landscape, flight, navigation, product]
---

# Landscape mode: fly over the Mandelbrot set as 2.5D terrain

## Context

The user wants to fly through a 3D fractal landscape and asked how that
combines with zooming. True 3D fractals (Mandelbulb, Mandelbox) have no
practical deep-zoom scheme ([[formulas/others-feasibility]]); the 2D
engine does ([[techniques/perturbation]], [[techniques/bla]]).

## Proposal

- **Terrain:** height = f(exterior distance estimate)
  ([[techniques/distance-estimation]]); the set boundary is the valley
  floor at 0, interior regions (minibrots) are flat lakes, the exterior
  rises. DE scales with zoom, so the terrain is self-similar at every
  depth. Candidate f: linear, or a soft log curve so plateaus do not
  dominate; decided by prototype images.
- **One fixed world; zoom is just distance:** h(c) is a fixed function of
  the location, independent of any zoom level; the terrain merely looks
  alike at every scale. There is no separate zoom parameter: the camera
  moves in 3D, perspective makes nearby ground larger, and each part of
  the ground is computed at the level of detail its distance needs (LOD).
  Sinking a little is a small zoom; a dive is a lot of sinking. New detail
  fades in (LOD blending, no popping).
- **Flight:** speed proportional to height above ground, so descending
  from 1e3 to 1e300 feels uniform. Terrain-following clearance is measured
  against the ground smoothed to the scale of the camera's altitude, so
  small side canyons do not make the camera twitch. Level flight = constant
  detail scale through valleys and over ridges; nose-dive = zoom in; pull
  up = zoom out. Camera position in arbitrary precision; height stored
  relative to the current scale.
- **Flight recorder:** every flight is recorded (position, clearance,
  heading per sample; exact coordinates as decimal strings, kilobytes per
  flight). Uses: rewind ("go back in time") and resume flying from any
  point, replay, video export of the flown path, sharing a whole flight,
  automatic waypoints at dives and sharp turns.
- **Rendering:** nested rings of 2D renders around the point below the
  camera, each at its own scale (clipmap / exponential-map idea,
  [[techniques/exponential-map]]), reused while flying; height-field ray
  marching with the existing normals, fog, sky, water, shadows.
- **Controls:** the crosshair and keyboard model of [[decisions/controls]]
  carries over (Space = dive towards the crosshair, WASD = fly/steer).

## Plan

1. Prototype (after R2b): still images from the Rust CLI at a few
   locations and depths, height-formula variants side by side; the user
   judges the look.
2. If it convinces: a landscape-mode milestone after BLA/LA (M4/M6), since
   the near rings need fast iteration while moving.
3. True 3D fractals stay a separate later track ("3D worlds", limited to
   about 1e4–1e5 magnification in float32).

## Open questions

Height function; clearance policy (fixed multiple of scale vs adjustable
dive speed); interior surface; cost of the near rings at depth while
moving.

## Prototype results (L0)

Code: `prototypes/landscape/` (own Cargo workspace, CPU, f64; README there).
Gallery: `prototypes/landscape/gallery/` (9 JPEGs). Measured 2026-10-14, M1 Pro
(10 cores), release.

**What was built.** `h(c) = f(DE)` sampled lazily on a quadtree of 33×33-sample tiles
(level `L` = spacing `2^L·S`, S = camera altitude), bilinear plus a blend between two
levels. That is a continuous LOD with no seams. Per-pixel height-field ray marching
picks the level from the pixel footprint at distance `t`. Shading: normals by central
differences at the hit level, sun with soft shadows, horizon-style AO, water with
Fresnel and marched reflections, fog, sky. Strata colour keyed by `log2(DE/S)` keeps
the colour self-similar. Interior uses the `|w|·n² < 1e-3` test
([[techniques/interior-detection]]); DE has bailout 1e5
([[techniques/distance-estimation]]).

**Height function: use `log`, `h = σ·ln(1 + k·DE/σ)` with σ = 0.1 S and k = 4.**
- `linear` (`k·DE`) is the only fixed-world function that is exactly self-similar.
  Any `f` with `f(λx) = λf(x)` is linear. But most ground near the boundary has DE ≪ S
  while the far ground has DE comparable to its distance. So k ≥ 0.5 turns the
  horizon into a wall (gallery 03), and k ≈ 0.2 gives a flat green plain with low
  mesas (plateau dominance, gallery 01/02).
- `log` is linear, so fixed-world, below DE ≈ σ/k = 0.025 S, which is the ground
  under and near the camera. Large features are compressed to ≤ ~0.8 S. The result is
  canyon country with strong relief at the camera's scale, and the horizon stays
  visible. Cost: the compressed far field depends on the camera scale. While sinking,
  far mesas get lower in absolute terms (their elevation angle halves per halving of
  S, where linear keeps it constant). This is a morph in the far field only; whether
  it is visible in motion is for L1 to judge.
- `sqrt` (`k·sqrt(σ·DE)`) is close to log nearby but keeps tall far mesas.
- `potential` (`k·G/|∇G|` at the ground point) is unusable: `|∇G|` varies by orders
  of magnitude, so it gives cliffs and spikes.
- A planet curvature (ground drops by `d²/2R`, R = 150 S) is a cheap, rendering-only
  way to hide the remaining far walls. Without it, log at pitch −8° still shows a
  plateau band (gallery 03).

**Look (honest).** With log it reads as a sandstone canyon landscape. Filaments
are the canyon floors, decorations are eroded cliff spurs, minibrots and the cardioid
are lakes, and a small water level (`--water 0.01–0.02 S`) floods the dense
near-boundary floor into archipelagos (gallery 07–09). The same shot at S = 1e-3, 1e-6,
1e-9 and 1e-12 and at four different places looks alike (gallery 04/05). At 1e-12, f64
shows no noise. Problems:
- Plateau dominance where the camera is near a large exterior region (−3/4 neck
  foreground, spiral arms): big smooth featureless slopes. DE is smooth there; only
  strata colour gives them texture.
- Coarse LOD makes blocky coastlines: exterior samples isolated in a lake become
  square-ish islands (min/coverage prefiltering would be needed). The smooth-count
  colour bands alias, so they are faded out where `DE < 4·footprint`.
- Horizon: isotropic footprint sampling under-samples grazing ground. Supersampling
  (2×2) hides it in stills; no visible LOD seams (two-level blend).
- Interior that is slow to prove (near-parabolic minibrot cardioid) runs to maxIter:
  9 % of samples at the period-936 minibrot at 1e-8 with maxIter 50 000. It is shown
  as lake (correct look), but it dominates the cost.
- Lighting reads well with the sun behind or to the side of the camera (az 120–160°);
  backlit views go flat.

**Timings** (1920×1080, 2×2 supersampling, cold cache, wall clock; 10 threads):

| Shot | Wall | DE evals | Iterations (avg/eval) | Tile CPU share |
| --- | --- | --- | --- | --- |
| seahorse 1e-4 (any variant) | 19–21 s | 37 M | 3.9 G (106) | 8 % |
| main cardioid shore 1e-1 | 46 s | 39 M | 2.0 G (51) | 3 % |
| −3/4 neck 1e-4 | 39 s | 38 M | 15 G (397) | 17 % |
| seahorse 1e-6 / 1e-9 / 1e-12 | 50 / 52 / 53 s | 38 M | 38 / 49 / 60 G (978 / 1289 / 1584) | 38–55 % |
| minibrot p936, 1e-8 | 157 s | 39 M | 328 G (8494, 8.8 % unresolved) | 78 % |
| elephant 1e-12 | 109 s | 38 M | 238 G (6315) | 80 % |
| 960×540, no SS, seahorse 1e-4 | 1.6 s | 10 M | 1.1 G | 30 % |

What dominates: at shallow depth, marching and shading (~55 steps per primary ray,
plus shadow, AO and reflection rays): ~2 µs of CPU per ray. At depth and near
minibrots, iteration (f64 with two derivatives, 2.8 ns/iteration/core with 8
interleaved orbits; 4.3 ns scalar). The cold cache needs ~18 DE evaluations per
output pixel at LOD bias 1.5. That is a lot more than one per pixel, because grazing
rays pass over ground sampled at the isotropic horizontal footprint, and each level
is fetched for the two-level blend and in 32-sample tiles.

**Estimate for a real-time GPU version (L1).**
- Clipmap: rings of N×N samples per level. A spacing of `d·pixel angle·bias` at
  distance `d` needs `N ≈ 2/(pixel angle·bias)`. At 1920 px, a 70° FOV and bias 1.5
  that is N ≈ 2048; with bias 3, N ≈ 1024. Levels span from the nearest ground (~S)
  to the far clip (~3000 S at 1e-3 rad): ~12 levels. Memory at 1024² × 12 levels × 8 B
  (f16 height, f16 DE, f32 smooth count) is ~100 MB, or 400 MB at 2048²: so use
  1024² rings, or keep only the visible ~⅓ of each ring.
- Per-frame updates at 60 Hz, flying one altitude per second (speed ∝ height), N =
  2048: each level shifts `16.7/2^L` samples per frame, so the new strips sum to
  ~2·N·16.7 ≈ 70 k samples per frame. Sinking at 2× per second needs one new finest
  ring (N² = 4.2 M) per second, another ~70 k per frame (coarser rings are reused one
  level up). So ~140 k DE evaluations per frame while flying and sinking, half that
  with N = 1024. A cold start (teleport) is the full ~10–40 M.
- Iterations: 140 k × 100–1500 per evaluation (typical exterior) = 0.014–0.2 G
  iterations per frame. That is fine at the app's ~30–50 Giter/s of float32
  perturbation on the GPU ([[synthesis/benchmarks]]). Minibrot shores (5–9 k average,
  with slow interior) reach 0.7–1.2 G per frame, 25–40 ms: these need BLA/LA
  ([[techniques/bla]]) and the auto-iteration statistics.
- Marching: ~60 steps × 2 bilinear fetches × 2 M pixels ≈ 250 M fetches per frame,
  plus shadows. That is cheap on the GPU compared with the iteration.

**Open problems for L1.** Far-field morph of `log` while sinking (judge in motion);
prefiltered LOD (coverage/min for lakes and filaments, mean for height) against
blocky shores and horizon aliasing; anisotropic or ray-differential footprints;
unresolved slow interior near minibrots (cost and correctness); smoothed-ground
clearance against tall plateaus (camera-height policy when a 0.8 S mesa approaches);
the derivative under perturbation/BLA in float32 with an exponent; and keeping the
whole stack of rings consistent while the reference orbit changes.

## L1: real-time prototype (as built)

Code: `prototypes/landscape-rt/` (own Cargo workspace; README there). Measured 2026-10-14,
M1 Pro, release, 1280×800 pt window (2560×1600 px) on the 120 Hz panel, background test window.

**Stack: Metal on objc2-metal + winit, not wgpu.** This is the app's own stack
([[platform/render-architecture]]): the same MSL dialect, so the DE kernel is a cut-down copy
of the app's `iterate` kernel, and the same crates were already in the cargo cache. It gives
direct `CAMetalLayer` control (drawable count, `presentedTime` for pacing measurements),
memoryless MSAA and depth targets on the tile GPU, and no shader translation layer. wgpu would
only have bought portability, which a prototype does not need.

**Why L0 jittered (verified in its code).** Each frame of `landscape dive` built a new
`Terrain` anchored at that frame's ground point and scale. The sample grid `c0 + i·2^L·S`
moved with the camera and S, so every frame resampled the DE field at new points (aliasing
shimmer). The camera height also came from five samples that moved with it. L0's dive video
measures mean |f(t) − f(t−1)| 17.5 and |f(t) − 2f(t−1) + f(t−2)| 29.3 (0..255 luma, 480×270):
the second difference is larger than the first, which is noise, not motion.

**Design.**
- *Camera-independent samples*: level ℓ samples sit at `origin + (i, j)·2^ℓ` (absolute
  levels, f64 offsets from a fixed origin). A sample, once computed, never changes. Hover test:
  frame difference 0.000.
- *Clipmap data*: 11 levels × 1024² toroidal R16F `log2(DE/spacing)` (−128 = lake), 2 MB per
  level. A 16×16-tile meta texel holds (birth time, tag); the tag encodes level and wrap
  cycle, so a stale texel is never read. New tiles fade in over 0.3 s. Storing log2 matters:
  an f16 clamp of DE/spacing at 65000 made square terraces, one per level, over plateaus.
- *Geometry clipmap* (Losasso–Hoppe, (unverified) reference; our own implementation): rings of
  448² vertices at 2 samples spacing, origins snapped to 32·K samples so nested holes align
  with 16×16-quad blocks. Blocks are instanced and frustum-culled on the CPU (2–3.6 k blocks).
  Vertex heights are geomorphed to the coarser ring's triangle surface by an L∞ distance α,
  and towards it wherever data is missing or fading. Crack-free by construction.
- *DE on the GPU*: float32 perturbation against an f64 Newton nucleus reference (periodic, so
  rebasing at the end wraps), Zhuoran rebasing ([[techniques/rebasing]]), derivative interior
  test ([[techniques/interior-detection]]), maxIter 20 000. **dz/dc needs a power-of-two
  exponent**: near the set it overflows float32, inf − inf gives NaN, and NaN became DE = 2^60
  (needles at the horizon). After the fix, `landscape-rt check` (GPU vs CPU f64, 1000–4000
  random samples per level) finds no DE error above a factor 2 among samples with
  DE > spacing/64 at 1e-3, 1e-6 (elephant) and 1e-12 down to a spacing of 2.8e-14. Below that
  the f64 oracle itself runs out of bits (|c| ≈ 0.74). Lake mismatches are 0–0.4 %.
  **Depth limit: S ≥ 1e-12.**
- *Resumable tiles*: one batch of 16 tiles with slow pixels took 10 ms, which caused most of
  the missed refreshes. Now each sample gets at most 1500 iterations per frame, and its state
  (48 B) waits in a pool for the next frame. The tag is written only when all 256 samples are
  done. Resumed tiles get at most half of the budget; otherwise slow minibrot tiles starve new
  ones. Budget 2.5 GPU-ms per frame, with a fast-rise / slow-decay cost estimate.
- *Priority*: fallback error `spacing / distance`, tiles ahead before behind.

**Flight model** (all in units of S): W/S throttle (cruise speed 0.08–8 S/s, persists),
A/D turn with banking, Space or ↑ dive, Shift or ↓ climb (stick convention: push dives, pull climbs), Page Up/Down look, F autopilot. Every input is eased
(first order, τ 0.2–0.6 s). The altitude follows the cubic-B-spline-smoothed ground (CPU f64
DE at S/3, looking up to 1.8 s ahead) through a critically damped spring (ω = 2.2 /s), with a
hard floor at 0.3 S above the less smoothed ground. **A dive zooms towards an aim point**: the
view centre's ground point, moved up to 3 S down the DE gradient (snapped to the nearest
coast). The altitude stays the same in units of S, so the descent has no spring lag. A
straight-down dive (L0's plan) loses the set: DE/S grows by e per e-fold, and in the first L1
tour the camera ended over featureless plateau (DE/S ≈ 2000).

**"Melting" (user feedback) and the layered σ.** With σ = 0.1·S (L0), a mesa (k·DE/σ ≈ 40)
changes its absolute height by about 73 % per e-fold of S, and the strata keyed by DE/S slide
with it. Measured by a pop check (each frame's camera rendered with the previous and the
current world state, mean |diff| 0..255, 1080p):

| flight | σ = 0.1 S (continuous) | layered σ (2 octaves, riser 0.5) |
| --- | --- | --- |
| level cruise, constant S | 0.0001 p50 / 0.09 max | 0.0002 / 0.08 |
| dive (5 octaves in 8 s) | 2.9 p50 / 5.0 max, in every frame | 0.6–1.0 p50 (detail fading in), 6.2–6.6 max in risers |

So level flight is static in both modes; LOD and fade changes are negligible
(< 0.1 % of pixels change by > 8/255). The melting is σ(S) alone. Layered σ keeps σ fixed
within each layer of 2 octaves of S. Mesas then stay rigid and grow as you approach (a real
zoom). σ changes over the second half of each layer as a smooth function of S: driven by S,
not by time, so hovering mid-transition is static and climbing reverses it. A mist closes in
(fog distance × (1 − 0.8·sin πx)), and the strata are keyed by DE/σ.
Trade-off: the morph is concentrated, 2–3× faster per frame than the continuous one while it
lasts, and a big mesa visibly sinks within about 0.8 s of full-speed dive. Riser 0.3 / 1 or
3 octaves measured max 7.2–8.7, so 2 octaves with riser 0.5 is the gentlest. Alternatives
not taken: a fixed σ (linear world, horizon walls, L0); σ per tile or per level (a
different surface per LOD, which is seams or distance-driven morphing); σ by distance from
the camera (melts as you approach, in every direction). `--sigma-mode continuous` keeps the
old behaviour for comparison.

**Collapsing rings on hilltops (second user feedback) and world-fixed colour.** With layered σ
the user still saw "gradients on hilltops changing while diving, like rings that collapse".
Measured with the pop check on a hilltop mask (flat, `DE/σ > 2.8`, alpha channel), dive
script, mean |diff| per frame 0..255:

| render | level flight | dive, rigid phase | dive, σ transition |
| --- | --- | --- | --- |
| full image (before) | 0.04 | 0.02 | 4.3 (12 % of pixels > 8) |
| colour only (before) | 0.03 | 0.007 | 8.7 (37 %) |
| lighting only | 0.07 | 0.004 | 2.5 |
| colour only, geometry σ held: before / after | 0.015 / 0.000 | 0.006 / 0.000 | **5.7 (21 %) / 0.28 (0 %)** |

Cause: the strata were keyed to log2(DE/σ), so every σ transition (a factor 4) shifted
them by 0.8 colour cycles, and the thin pale layers by 2.4. The iso-DE bands around a
mesa top contracted towards its centre. Terms keyed to DE/S (vegetation, crust, valley
darkening) also slid continuously while diving, mostly in valleys, since they saturate
on mesa tops. Fix: colour is now a function of the world position. Strata are keyed to
log2 of the absolute DE (colour cycles 0.4 per octave as before, so the hue drifts with
depth instead of repeating). The thin layers fade out where a pixel spans more than a
quarter of their period. Vegetation, crust and darkening use DE/σ of a hard colour layer
and cross-fade between the two layers over the transition. The rest of the change on
hilltops in transitions is lighting (2.5): the mesa really changes shape, and its
steep/flat shoulder moves with it. That is the remaining cost of layered σ.
`--colour legacy` keeps the old colouring. Clips: `review/landscape/l1-dive-colour-before.mp4`
/ `-after.mp4`, `rings-before-after.png`. 120 Hz unchanged (dive 119.8 fps, tour 119.3).

**Results.** Frame pacing at 120 Hz in [[synthesis/benchmarks]] (L1): 117.9–119.6 fps
presented, 0.2–1.8 % refreshes missed, render 3.5–4.4 ms p50 / ≤ 9 ms p99, DE ≤ 3 ms p99.
Review videos 1080p60 in `review/landscape/` (gitignored): `l1-tour.mp4` (35 s, 1e-3 → 9e-8),
`l1-dive.mp4`, `l1-dive-continuous-sigma.mp4` (before/after of the melting fix),
`l1-turns.mp4`, `melt-continuous-vs-layers.png`.

**Still weak (honest).**
- Detail arrives late in dives. Each octave of dive needs a new level of 4 k tiles, about
  1.3 octaves/s at full dive. With 2048² data the visible backlog (tiles within ±60° and
  2 fog distances) was 15 k tiles p50 in the dive script; with 1024² it is 0.4 k p50 and
  5.9 k p95. That is why 1024² is the default, at half the data resolution (normal detail
  ~3–6 px).
- The DE tile budget competes with the render: misses come from GPU frames over 8.3 ms
  (render p99 7–9 ms at 2560×1600 with 4× MSAA, plus DE). Other GPU users (a running app
  test, video in a browser) push it to 110–115 fps.
- Plateaus far from the boundary are featureless (smooth DE) and fill the view when the camera
  skims a mesa top during a dive. The look has no colour detail from the iteration count.
- Faceted geometry at distance (vertex spacing 2 samples) and normal detail fading by the
  pixel footprint. No shadows, AO or water reflections (out of scope for L1).
- The autopilot zig-zags along the coast (side flips); it is a demo aid, not tuned.
