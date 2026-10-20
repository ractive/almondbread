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
practical deep-zoom scheme ([Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)); the 2D
engine does ([Perturbation](../techniques/perturbation.md), [Bivariate linear approximation (BLA)](../techniques/bla.md)).

## Proposal

- **Terrain:** height = f(exterior distance estimate)
  ([Distance estimation](../techniques/distance-estimation.md)); the set boundary is the valley
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
  [Exponential map (log-polar) rendering](../techniques/exponential-map.md)), reused while flying; height-field ray
  marching with the existing normals, fog, sky, water, shadows.
- **Controls:** the crosshair and keyboard model of [Controls: keyboard, pointer and touch](controls.md)
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
([Interior detection](../techniques/interior-detection.md)); DE has bailout 1e5
([Distance estimation](../techniques/distance-estimation.md)).

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
  perturbation on the GPU ([almondbread benchmarks and verification record](../synthesis/benchmarks.md)). Minibrot shores (5–9 k average,
  with slow interior) reach 0.7–1.2 G per frame, 25–40 ms: these need BLA/LA
  ([Bivariate linear approximation (BLA)](../techniques/bla.md)) and the auto-iteration statistics.
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
([Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md)): the same MSL dialect, so the DE kernel is a cut-down copy
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
  rebasing at the end wraps), Zhuoran rebasing ([Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)), derivative interior
  test ([Interior detection](../techniques/interior-detection.md)), maxIter 20 000. **dz/dc needs a power-of-two
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

**Flight model, first version (superseded below by the 3D nose-flying model)** (all in units of S): W/S throttle (cruise speed 0.08–8 S/s, persists),
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

**Lake flicker (user feedback 3).** Ground was painted over minibrot lakes in 16×16-tile
staircases and ring-shaped L-blocks, and it flickered. Cause: the fragment shader blended level ℓ
with level ℓ+1 by vertex-interpolated weights only. Where ℓ+1 was not valid (tile not
computed yet, or a stale texel of another place in the toroidal texture), its data was mixed
in, and lake coverage fell to 0 where both weights vanished. The bug was there from the
first build (it reproduces at 4b1c040, 9.3 % of pixels > 24/255 in one frame); flying over
lakes exposed it. Fix: the fragment checks validity (tag + fade) of ℓ, ℓ+1 and ℓ+2 itself,
blends normalized weights, and falls back to the geometry (lake when the surface sits at lake
level). Same-camera check over the minibrot3 tour: worst frame 17.4 % → 0.009 % of pixels
> 24/255; lake pixels (mask) 0.000–0.002 in level flight and the rigid phase. GPU cost was
+1 ms at 1080p; a 3-tap gradient (centre + forward differences) and skipping zero-weight
levels brought the render to 2.4 ms p50 (before the fix 3.6).

**R32F data.** Flying low exposed f16 precision. log2(DE/spacing) is about 14 on plateaus at
the finest levels, where an f16 step is 2^-7: height terraces of about 0.002 S, which showed
as regular zig-zag stripes 0.04 S below the eye. The data is now R32Float (4 MB per level,
filterable on M1).

**Controls redesign (user feedback 4–6): fly the nose, zoom separately.** Users found the
first model unsteerable. It held altitude (terrain following), clamped the pitch, and dived
by zooming towards a coast-snapped aim point. "You can't fly into a valley, it keeps
zooming in." New model (`flight.rs`):
- The nose is the flight path. W/↑ push and S/↓ pull (rate control, down to −90°), A/D/←/→
  bank-turn, Q/E throttle (cruise, persists).
- With the zoom held, the world is rigid: σ is fixed, so the terrain is a fixed 3D shape and
  nothing melts by construction. The terrain is only a collision floor (0.04 S); there is no
  altitude hold.
- Zoom is explicit. Shift+W/↑ zooms in about the crosshair point (the nose ray's ground hit,
  fixed on screen); Shift+S/↓ zooms out about the camera's ground point. Eased, 1 e-fold/s.
- Auto-zoom mode (Z, `--auto-zoom`): a nose steeper than 45° zooms in (smoothstep to the
  maximum at 85°), and a height above ground over 1.5 S zooms out (full at 4 S). Pitch-up
  does not zoom out, so the pilot never has to stare at the sky.
- Speed is S·throttle·clamp(1.5·height/S, 0.05, 1). With √ scaling (≥ 0.25) the ground at
  0.04 S rushed past at about 7 rad/s, mean frame difference 19–23/255; linear keeps low flight
  readable.
- Detail follows the eye: the finest clipmap level follows min(S, 2·height above ground),
  down to 4 octaves below the zoom's finest (11 → 15 levels when low).
- A crosshair at the screen centre (the flight path) turns amber while zooming in and blue
  while zooming out.

Target dive (`record --script target[-auto] --loc …`): a closed-loop pilot uses the same stick
and zoom inputs. It points the nose at the location's centre, flies down to it at fixed zoom
(with a 1.2 S altitude hold in auto mode until the target is more than 55° below, and slowing
near it so it does not orbit), then zooms in: Shift+↑ in manual mode, the steep nose in auto.
Final build, 45 s budget, fill-limited zoom:

| target | manual zoom | auto zoom |
| --- | --- | --- |
| seahorse spiral centre (≈ 5e-3 away) | 1e-7 in 40 s, 0.000 S off | 1.7e-7 at 45 s, 0.4 S off |
| minibrot3 lake centre (period-3 nucleus) | 9.6e-7 at 45 s, 0.9 S off, over the lake | 6.4e-8 in 31 s, 0.000 S off |

Before the fill limit the manual seahorse dive took 13 s, but the end was tile soup.

Which feels better: manual for exploring (you choose when the world rescales; flying at a
fixed zoom is the calm part), auto for a quick plunge onto one spot. Auto zoom only triggers
in steep dives, which a pilot does deliberately. Its zoom-out-by-height prevents flying "up
and away" at a fixed zoom, but it also means you cannot climb high to look around: that is
the trade-off. (This verdict is from the scripted flights and the videos, not from flying it
by hand.)
Weak: the target pilot banks hard at the start and skims the lake at the clearance floor
(visually dull). Collision is a floor, not an avoidance, so flying into a wall slides you up
the face of the wall.

**Reference orbit must follow the zoom (correction).** The first version kept one nucleus
reference for the whole session ("a periodic reference never escapes, so rebasing makes it
universal"). That is wrong for float32 deltas. In a seahorse dive with the start's reference
(period 25, 6e-3 away), 6–55 % of the computed samples at the fine levels were wrong at
S ≈ 8e-8 (spikes): `record --check-end` compares the clipmap's computed tiles with CPU f64.
The CPU check must skip tiles that are not computed yet; the first in-session numbers
included them and overstated the errors. Two effects:
- dc = c − c_ref is a float32 per tile, rounded at |dc|·2^-24, which must stay well below the
  finest spacing (S/32768).
- A nucleus picked at a coarser scale does not serve finer ones: period 39 picked at 6e-6
  failed at 1e-7, where a nearby period-624 nucleus is exact.

Rule now: re-reference when the camera is more than 48 S from the reference or S has shrunk
4× since it was chosen. The new reference is the highest-period nucleus within 24 S found by
ball-period searches at radii 0.5–32 S; this takes 0–0.4 ms. In-flight tiles finish; tiles
waiting to resume are dropped and requested again. Result: 0 errors on all computed tiles of
11 levels at 8e-8. Window pacing in the dive script with re-references: 119.5 fps.

**Fill-limited zoom.** Zooming in at 1 e-fold/s onto a minibrot shore (slow tiles) outruns the
DE budget. Several levels in a row go missing, and the fallback showed tile-shaped water
holes (height 0) or, with a longer fallback chain, tile-shaped slabs. Fixes:
- The vertex fallback chain now reaches ℓ+5.
- While zooming, detail follows S only (the eye-height levels would be obsolete within a second).
- The zoom-in rate yields to the backlog: × (1 − missing ahead beyond 800 / 3500), at least
  0.15, and × (1 − missing within 3 S / 60), at least 0.1.

The seahorse target dive is then clean, at the price of speed: 1e-3 → 2.8e-7 in 30 s instead of
→ 8e-8 in 13 s.

**Results.** Frame pacing at 120 Hz in [almondbread benchmarks and verification record](../synthesis/benchmarks.md) (L1): first version 117.9–119.6 fps; after the controls redesign hover 119.3, cruise 119.2, dive 117.2, tour 118.8 fps (render 3.8–4.6 ms p50, ≤ 7.7 ms p99; measured with no other GPU load; with another agent's dense-sampling job and a second landscape-rt instance running it drops to 65–106 fps)
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

## L2: close-up detail and robustness (as built)

Code: `prototypes/landscape-rt/` (same crate). Measured 2026-10-15, M1 Pro, release. Other
agents' GPU jobs (two `motion-reuse` runs, builds) and the user's own landscape-rt window shared
the GPU during every measurement, so GPU times are noisy: every comparison below is L1 and L2
interleaved under the same load, repeated twice (both values given where they differ).

**User feedback on L1.** "It feels ok... Not wow yet. You should be able to probably zoom in
more. The detail level at the highest zoom level is currently very low." The screenshot
(S = 1.76e-6, eye 0.04 S above the ground, backlog 5658 tiles) showed flat untextured slabs
(white, light blue, steel blue) in tile-shaped steps in the foreground, a blurry near ground and
a flat green plain with small lakes.

**What the slabs were.** Missing data rendered by the wrong path. The vertex fallback below
ℓ+2 point-sampled ℓ+3..ℓ+5 at the ring vertices: 2×2 and 4×4 vertex groups took one coarse
sample, flat tile-shaped steps, and height 0 (water) past ℓ+5. The fragment stopped at ℓ+2
and shaded the rest from the faceted geometry, with "lake" wherever that geometry sat at 0: a
different material. Reproduced with the new low-flight pilot and a starved DE budget
(`record --script low --scale 1.76e-6 --budget-ms 0.4`, `LRT_MASK=fallback`): L1 shades
10 % of the pixels p50 (45 % max) without any data and 16 % p50 (69 % max) from ℓ+2 or coarser.
The L1 manual target dive also shows it at full budget: up to 99.9 % of a frame without data
(p99 60 %) while the zoom outruns the tiles.

**Fixes.**
- *A complete pyramid.* A tile is scheduled only after its parent tile (level ℓ+1) is done
  (CPU `DONE` bit per tile, set when the GPU job finishes). The valid levels at any point are
  then always a contiguous range up from the coarsest, so a fallback always exists.
- *Fallback through all levels.* Each ring gets every coarser active level (and every finer
  one, below). Vertex: below ℓ+2, the bilinear surface of the finest fully valid coarser level
  with fading finer ones blended on top (smooth, no steps). Fragment: normalized weights over
  all levels by validity; geometry-only shading only where no level at all is valid.
- *Geomorph with curvature.* The coarse target of the geomorph now includes the coarse ring's
  curvature drop (interpolated like the height); L1 dropped each vertex by its own d²/2R, which
  left hairline cracks along ring edges (sky showing through, faint white streaks on lakes).
- Result: no-data pixels 0.0000 % in every run (low, starved, plain, zoom, dives), against
  10 % p50 / 45 % max (starved) and 60 % p99 (target dive) in L1.

**Scheduling and budget.**
- *Priority = screen-space error removed*: the parent's spacing (what is shown now) over the 3D
  distance from the eye (the eye height counts, so flying low the ground under the camera is
  nearest), times visibility from the view frustum (inside 1, just outside 0.35, else 0.15,
  behind 0.1). L1 used horizontal distance and heading only.
- *Fill-limited zoom was self-throttling.* L1 counted missing tiles before scheduling, so each
  frame's fresh demand (≥ 60 tiles within 3 S) held the zoom at 0.1–0.15 e-folds/s while the GPU
  idled (DE 0.5 ms of a 5 ms budget). That is the user's "zoom in more": 5e-4 → 7e-6 in 40 s.
  L2 counts the backlog left after the frame's batch (in view within 2 fog, or within 1.5 S of
  the eye). Zoom-and-descend script: L1 5e-4 → 5.7–7.5e-6 in 40 s, L2 → 1.26e-6 (the 300×
  target) and then the low flight.
- *Cost estimate.* L1 took every rise of the per-tile cost at once and decayed 5 % a frame;
  small (overhead-dominated) batches kept the budget at a fraction for seconds. L2 pools
  batches to ≥ 32 tiles and takes an EMA (only a 1.6× overshoot rises at once).
- *Window budget follows the render*: DE gets 90 % of the refresh period minus the render
  p90, at least 0.8 ms (the floor keeps the terrain filling while another process's GPU work
  shows up in our command buffers' times), at most 1.6 × `budget_ms`. Offscreen: fixed.
- *Detail follows the eye harder*: one finest sample ≈ 2 px under the eye (L1: 4 px), at most 5
  octaves below the zoom's finest level (L1: 4).

**Cheaper DE tiles: parent samples.** With the parent done first, the 64 even samples of a
tile are the parent's samples (log2(DE/spacing) + 1, lake stays lake): copied, not iterated.
Threads 0–63 copy, threads 64–255 iterate the other 192 samples (three per 2×2 block), so two
whole SIMD groups finish at once: 25 % fewer SIMD cycles. Cold prefill of 6 locations
(`bench`, ~360 k tiles with the 4 extra levels): L1 82–86 k tiles/s, L2 98–109 k tiles/s
(1.15–1.27×; per location 0.93–1.7×, noisy). Not 1.33×: the slowest samples dominate a tile.
BLA and local SA were not tried: M4 found BLA does not pay at f64-reference depths, and
float32 local SA is an open question ([Does local series approximation hold in the float32 perturbation kernel?](../questions/local-sa-float32.md)); the scheduler, not the
per-tile cost, was the bottleneck (the demand while flying low is 1–6 k tiles/s against a
capacity of 10–50 k tiles/s inside a 2.5 ms budget).

**Detail below the computed resolution.** World-anchored value noise: octave `oct` lives on
the integer lattice of absolute level oct (cell 2^oct complex units), so it is a fixed function
of c whichever ring or level evaluates it (the lattice base per ring and octave is computed
exactly on the CPU, mod 128 cells; the noise and its gradient come from a 512² RGBA16F
periodic texture: hashing the four corners per octave cost 2.8 ms per frame at 2560×1600).
Amplitude 0.12 × wavelength, only for wavelengths under DE/4 (the true DE field is smooth below
that scale, so this is surface texture, never fractal structure the set does not have), from
0.05 S down to 1.5 px. The normals get all octaves; the geometry the octaves of ≥ 4 samples of
the evaluated level (consistent between a ring and the ring it morphs to). It also tints
vegetation and rock and darkens cavities. Hover: frame difference 0.000.

**Plains.** Three parts. The detail texture above (on plains DE is large, so all octaves
apply). Equipotential ridges from the smooth iteration count ν: the DE kernel stores
r = sin²(πν/2) per sample (R16Unorm; R8 made visible contour terraces), and the height adds
0.3·σ·r where DE/σ > 1.5 (full at 10), faded where two dwell bands span < 12 samples of the
level. First baked into DE (`DE·(1 + a·sin²(πν))`, σ-free): that put ridges on every mesa
flank, where the bands run downhill, and the flanks looked like zebras. Vegetation patches
from the noise value.

**Rings and shading resolution.** Even in L1 the vertex stage was ~4.5 ms of the 6.5 ms render
in the window (2560×1600). Rings are now 256² vertices (L1 448², `--ring`), a third of the
vertex work; fragments shade from the finest level that resolves the pixel (≤ 2.5 samples
per pixel), also from levels finer than the ring's own, by footprint and validity only. That
choice must not depend on the ring: with the ring's α in it, pixels changing ring when a ring
snapped changed level (pop check max 1.3–2.5). A finer level's window is faded by L∞ distance
from the camera (400–460 samples); a fade keyed to the window edge jumped with its 16-sample
steps.

**Results** (recorder 1920×1080 at 60 fps, DE budget = live 2.5 ms × 2; two interleaved
repeats; numbers in [almondbread benchmarks and verification record](../synthesis/benchmarks.md) L2):

| run | L1 | L2 |
| --- | --- | --- |
| low valley flight, S = 1.76e-6, eye 0.065–0.08 S: tiles completed/s | 2 400–2 800 | 4 300–5 000 |
| same: backlog in view (unscheduled, frustum, < 2 fog) p95 / max | 1 500–4 500 / 2 200–4 900 | 4–6 / 56–83 |
| same: pixels without data / two finest levels missing, max | 0.007 % / 5.8–6.4 % | 0 / 0.03 % |
| same, DE starved (0.4 ms): tiles/s | 384–396 | 627–1 020 |
| same, starved: pixels without data p50 / max | 10.2 % / 45.4 % | 0 / 0 |
| same, starved: two finest levels missing p50 / max | 15.6 % / 69 % | 0 / 4.3 % |
| zoom 300× while descending, then low flight: S after 40 s | 5.7–7.5e-6 | 1.26e-6 (target reached) |
| manual target dive onto the seahorse centre, 1e-3 → 1e-7 | 39.6–40.5 s, no-data p99 60 %, max 99.9 % | 28.5 s, no-data 0 |
| `deep` target dive (L2 only) | — | 1.7e-11 in 70 s |
| cold prefill (`bench`, 6 locations) | 82–86 k tiles/s | 98–109 k tiles/s |
| pop check hover, mean diff max | 0.005 | 0.010 |
| pop check cruise (S-turns), mean diff p99 / max | 0.022 / 0.033 | 0.050 / 0.112 |
| pop check low flight, mean diff p99 / max | 0.45 / 0.72 | 0.53 / 0.65 |

The cruise pop check is 3× L1's: more fine data arrives while flying (finer levels are used
for shading and fade in over 0.3 s); 0.2 % of the pixels change by more than 8/255 in the worst
frame. In a dive the pop check measures the layered-σ morph, which is now 2–3× faster per frame
because the zoom no longer stalls (p99 17.6 at ~0.9 e-folds/s).

Window pacing (`fly --background`, 2560×1600 at 120 Hz, two runs each, interleaved, other GPU
jobs running): low flight L1 106.5 / 111.9 fps, L2 111.3 / 106.1 fps; dive L1 114.4 / 106.3, L2
88.3 / 112.5 fps (L2 zooms to 7.6e-7–1.1e-6 in the 13 s, L1 to 9.3e-5–1e-4). Render p50 4.0–5.9
ms for both; p99 16–42 ms under this load for both, so the 120 Hz criterion could not be
verified on a quiet GPU. A trivial shader (`LRT_PROFILE=6`) measured 3.8 ms in this state, and
4.2 ms in a 320×200 window: most of the render time was other processes' work. With hashed
noise L2 rendered at 12.9 ms (L1 6.5) in the same window; the texture noise and the smaller
rings brought it to L1's level.

Videos (1080p60, `review/landscape/`, gitignored): `l2-l1-low.mp4` / `l2-l2-low.mp4` (low
valley flight at the screenshot's scale), `l2-l1look-plain.mp4` / `l2-l2-plain.mp4` (low over a
plain; "before" is L2 with detail, ridges and variation off, since the L1 build has no plain
pilot), `l2-l1-target-seahorse.mp4` / `l2-l2-target-seahorse.mp4` (1e-3 → 1e-7 dive),
`l2-l2-deep.mp4` (to 1.7e-11), stills `l2-low-before-after-f600.png`, `-f900.png`.

**Depth limit (not lifted).** S ≥ 1e-12 stays. What limits it, and what lifting it takes:
- *CPU side, f64 absolute coordinates.* The camera ground (`Ground`, collision floor and nose
  ray), the start placement (`snap_to_boundary`) and the nucleus search (`find_minibrot`,
  Newton) iterate absolute f64 c. At |c| ≈ 0.75 an f64 step is 1.1e-16, and the finest samples
  at S = 1e-12 are S/32768 = 3e-17 apart. Needed: the reference from `almondbread-core`
  (`ReferenceRequest` / `Reference::compute_big`: BigFloat ball-period nucleus and orbit, already
  used by M3b), and the ground by f64 perturbation against that orbit (f64 deltas reach 1e-300).
- *World origin.* Offsets are f64 from a fixed origin c0. Deep, c0 must be a `Center`
  (BigComplex) and must move with the camera, or offsets of 1e-3 lose the 1e-20 sample
  spacing. A move by a multiple of N·256 coarsest-level samples (262 144 × the coarsest
  spacing) leaves every texel index and tile tag unchanged (texel = index mod N, tag = index div
  N mod 256), so no data is lost: shift the level windows, ring origins, resume keys, camera
  and reference offsets, and clear the ground cache.
- *GPU kernel, float32 deltas.* dc and dz are float32 offsets of about S; the rebasing test
  |z|² < |dz|² underflows below |dz| ≈ 1e-19, so the plain kernel serves S down to ~1e-16 (the
  app's plain kernel stops at 1e-17 per pixel for the same reason, [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M3b).
  Deeper needs the app's rescaled deep kernel (float32 mantissa with a shared exponent,
  floatexp full steps where |Z| is tiny), with dz/dc already carrying an exponent here.
So the first step (big reference, perturbed ground, moving origin) buys about four decades
(to ~1e-16); unlimited depth also needs the deep kernel. Neither is a small change to a
prototype, so L2 spent the time on detail and robustness instead. The user's "zoom in more"
at S = 1.76e-6 was, as measured, the fill-limited zoom stuck at 0.1–0.15 e-folds/s (below).

**Still weak (honest).**
- At full budget the low valley flight looks almost the same as L1 (compare the stills). L2's
  gains show under load (DE starved, zooming, another process on the GPU), which is where the
  user saw the slabs. Over a plain the detail texture is a clear gain. Elsewhere it is "ok", not
  "wow".
- The detail noise is plain value noise: sandy, mottled. It reads as terrain, not as rock, and
  there is no erosion structure. The domes and mesas are still smooth clay in backlight; there
  are no cast shadows and no AO, and those would do more for depth than more noise.
- The green valley floor near minibrots is still a uniform carpet from a distance.
- Straight-down fast zooms look blobby (coarse data stretched, strata rings) until the levels
  arrive.
- The depth limit stays at 1e-12 (above). After the `deep` dive (S = 1.7e-11) the CPU f64
  oracle disagrees with 0–16 of 600 samples per level (DE off by more than 2×): the f64 limit
  of the oracle and the f64 nucleus, as in L1.
- 120 Hz under a quiet GPU is not re-measured (see above).

# Citations

- [No cited source: our own design decision and our own prototype measurements, as stated above](#context)
