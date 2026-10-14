# landscape: L0 prototype of landscape mode

CPU still renderer for the design in `kb/wiki/decisions/landscape-mode.md`: the
Mandelbrot set as one terrain `h(c) = f(DE(c))`. The set boundary is the valley floor
(height 0), the interior is flat water at 0, and the exterior rises with the distance
estimate. The camera altitude `S` sets the detail scale. The ground is sampled at the
level of detail its distance needs. Nothing here depends on the app crates: it has its
own f64 iteration and its own Cargo workspace.

```sh
cargo build --release
target/release/landscape render --loc seahorse --scale 1e-4 -o out/a.png
target/release/landscape render --loc seahorse --scale 1e-6 --snap minibrot --water 0.01 -o out/b.jpg
target/release/landscape dive --loc seahorse --scale 1e-3 --scale-end 1e-10 --frames 8 --out-dir out/dive
target/release/landscape montage --cols 2 --labels "a|b" -o out/ab.jpg out/a.png out/b.jpg
target/release/landscape info --loc elephant --scale 1e-12      # where the camera lands
./gallery.sh [--quick] [--video]                                # the curated gallery
```

`out/` is ignored by git. `gallery/` holds the curated JPEGs.

## How it works

- **Iteration** (`src/mandel.rs`): plain f64, `z`, `dz/dc` (for DE, with a power-of-two
  exponent so long orbits do not overflow) and `dz/dz1` for the interior test
  `|w|·n² < 1e-3` (kb: techniques/interior-detection, Pitfalls). Cardioid/bulb test.
  The bailout is 1e5, so the DE is smooth across escape-count changes. Points that
  reach `--max-iter` without proof are "unresolved" and shown as lake (their share is
  printed). A tile evaluates 8 orbits interleaved, which gives 2.8 ns per iteration
  instead of 4.3 ns. c is absolute f64; this is fine to S ≈ 1e-13 (at 1e-12 no noise
  is visible).
- **Placement**: `--snap boundary` (default) walks from the start point towards the
  set in steps of DE/2 until DE < S/1000. Then it backs off `--back` S (default 2)
  outwards and looks towards the set, plus `--turn-heading` (25°). `--snap minibrot`
  finds a minibrot of size 0.2–40 S nearby (ball period, Newton nucleus, mathr's
  size estimate) and lands on its shore.
- **Terrain** (`src/terrain.rs`): world units are S, measured from the camera's ground
  point. Level `L` has sample spacing `2^L`. Tiles of 33×33 samples (height, DE/S,
  smooth count) are computed on first touch and memoised, one `OnceLock` per tile, and
  every thread keeps a small handle cache. Lookups are bilinear and blended between
  two levels. That gives continuous LOD with no seams.
- **Rendering** (`src/render.rs`): per-pixel ray marching. At distance `t` the
  footprint is `t · pixel angle · lod_bias`, which selects the level for both the
  height lookup and the minimum step. The step is `max(0.45·gap, 0.6·footprint)`, and
  the hit is refined by 8 bisections. Normals come from central differences at the
  hit level. Then sun with Quilez-style soft shadows, a sky term with horizon-style AO
  (18 height probes), a bounce term, water (Fresnel, marched reflection of the
  terrain, sun glitter), exponential fog and a sky gradient with the sun. ACES-like
  tonemap. `--ss N` gives N×N supersampling.
- **Colour**: sandstone strata keyed by `log2(DE/S)`, so the bands are self-similar,
  plus thin pale layers. Fine bands from the smooth iteration count are faded in only
  where the sampling resolves them (`DE > 4·footprint`). Green on flat ground near
  the valley floor, a pale crust on flat high ground, darker red rock on steep faces.
- **Planet curvature** (`--curvature R`, default 150 S): the ground drops by
  `d²/2R` with distance from the camera. It is a rendering-only trick that hides far
  walls below the horizon.

## Height functions (`--height`, `--k`, `--sigma`)

| Variant | Formula (world units S) | Fixed world? | Look |
| --- | --- | --- | --- |
| `linear` | `k·DE` | yes, exactly self-similar | k ≥ 0.5: the horizon is a wall (far DE ~ distance). k ≈ 0.2: a flat green plain with low mesas, since most ground near the boundary has DE ≪ S. |
| `log` (default) | `σ·ln(1 + k·DE/σ)`, σ in S | linear (fixed) below DE ≈ σ/k; the far field scales with the camera | **Recommended**, σ = 0.1, k = 4: canyon country. Near relief is strong, far mesas are capped at ~0.8 S. |
| `sqrt` | `k·sqrt(σ·DE)` | no | Close to log nearby, but tall far mesas dominate the horizon. |
| `potential` | `k·G/|∇G|(ground point)` | yes | Unusable: `|∇G|` varies by orders of magnitude, so it gives cliffs and spikes. Kept for comparison only. |

`--water W` floods everything below `W·S`; lakes (the interior) are always at 0.

Other knobs: `--altitude` (in S, default 1), `--pitch` (−12°), `--fov` (70°),
`--lod-bias` (1.5), `--fog` (120 S), `--sun-az` (relative to the heading, 150°),
`--sun-el` (22°), `--no-shadows`, `--no-reflections`, `--max-dist` (3000 S),
`--max-iter` (50 000), `--palette-shift`, `--label`.

## Gallery (`gallery/`, made by `./gallery.sh --video`, 1920×1080 at 2×2 SS, cells downscaled)

1. `01-variants-seahorse-1e-4.jpg`: the four height functions, same shot.
2. `02-variants-minibrot-1e-6.jpg`: the same on the shore of a period-39 minibrot (lake).
3. `03-far-field.jpg`: linear vs log, flat world vs curvature: walls at the horizon.
4. `04-selfsim-seahorse.jpg`: the same shot at S = 1e-3, 1e-6, 1e-9, 1e-12.
5. `05-selfsim-locations.jpg`: main-cardioid shore 1e-1, −3/4 neck 1e-4, minibrot
   (period 936) 1e-8, elephant valley 1e-12.
6. `06-dive.jpg`: 8 frames on a glide path into one boundary point, 1e-3 → 1e-10.
7. `07..09-hero-*.jpg`: seahorse 1e-4, minibrot 1e-6 and elephant 1e-6, flooded valleys.
8. `10-dive.mp4`: 240 frames (640×360, 8 s) of the same glide path, 1e-3 → 1e-11.

Timings and an assessment are in `kb/wiki/decisions/landscape-mode.md`
("Prototype results (L0)").
