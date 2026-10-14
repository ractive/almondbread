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
