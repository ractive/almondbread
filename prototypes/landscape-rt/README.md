# landscape-rt: L1 real-time landscape prototype

Fly over the Mandelbrot set as 2.5D terrain at the display refresh rate (kb:
`decisions/landscape-mode.md`, section L1). This crate has its own Cargo workspace, Metal on
objc2-metal and a winit window. It reuses no app crate: the DE kernel is a cut-down copy of
the app's `iterate` kernel, and the f64 helpers come from the L0 prototype.

```sh
cargo run --release                       # fly (seahorse valley, S = 1e-3)
cargo run --release -- fly --loc elephant --scale 1e-5
cargo run --release -- fly --script tour --background      # test window: 120 Hz pacing stats
cargo run --release -- record --script tour -o out/tour.mp4 --popcheck   # 1080p60 offscreen video
cargo run --release -- check --scale 1e-9  # GPU DE samples vs CPU f64
cargo run --release -- framestats some.mp4 # temporal metrics of any video
```

## Controls

| Key | Action |
| --- | --- |
| W / S | throttle up / down (cruise speed in S per second, 0.08–8, persists) |
| A / D or ← / → | turn (eased yaw rate, the camera banks) |
| Space | dive = zoom in towards the aim point (view centre on the ground, snapped to the nearest coast) |
| Shift | climb = zoom out from it |
| ↑ / ↓ | dive / climb (stick: push dives, pull climbs) |
| Page Up / Page Down | look up / down |
| F | toggle the coast-following autopilot (steers; you keep throttle and dive) |
| R | restart at the start location |
| Esc | quit |

Altitude is automatic: the eye follows the B-spline-smoothed ground ahead at 1 S, through a
critically damped spring. Speed is in units of S, so every depth feels the same. The window
title shows S, speed, altitude, GPU ms and the DE tile backlog.

## How it works

- **World**: complex offsets from a fixed origin (f64). Level ℓ samples sit at `origin +
  (i, j)·2^ℓ` and never move. L0 put its sample grid on the camera's ground point and on S
  every frame, so every frame resampled the terrain. That was the jitter in its dive video.
- **Data**: 11 clipmap levels, each a 1024² toroidal R16F slice of `log2(DE/spacing)` (lake
  = −128), plus a per-16×16-tile meta texel `(birth time, tag)`. The tag names level and
  wrap cycle, so a stale texel is never used. Tiles fade in over 0.3 s.
- **DE**: GPU float32 perturbation against a nucleus reference (f64, Newton), with Zhuoran
  rebasing, dz/dc with an exponent (it overflows near the set), and the derivative interior
  test, maxIter 20 000. Tiles are resumable: at most 1500 iterations per sample and frame, so a
  tile with slow pixels cannot stall a frame; resumed tiles get at most half the budget. Budget: 2.5 GPU-ms per frame with a fast-rise cost estimate.
  Priority: fallback error (spacing / distance), ahead before behind.
- **Geometry**: geometry clipmap (Losasso–Hoppe). Each ring is 448² vertices at 2 samples
  spacing (`--n 2048 --k 4`: twice the data resolution, ~4× the DE work), drawn as instanced 16×16-quad blocks and frustum-culled on the CPU. Heights are
  geomorphed to the coarser ring's triangle surface by distance (α), and also where data is
  missing or still fading in, so there are no cracks and no pops.
- **Height**: `h = σ·ln(1 + 4·DE/σ)` (L0). σ stays fixed within layers of 2 octaves of S
  and changes over the second half of each layer (S-driven, reversible, a mist closes in).
  `--sigma-mode continuous` (σ = 0.1 S) re-shapes the whole world all the time while diving:
  the "melting" users saw.
- **Shading**: normals from the filtered data of two levels, sun, sky light, fog, planet
  curvature (R = 300 S), 4× MSAA. Colour is a property of the world position: the L0
  sandstone strata are keyed to log2 of the absolute DE (they never slide), and vegetation,
  crust and valley darkening to DE/σ of a hard colour layer, cross-faded between layers.
  `--colour legacy` restores strata by DE/σ and terms by DE/S (rings collapsing in σ
  transitions).
- **Measurements**: `record --popcheck` renders each frame's camera with the previous and the
  current world and reports the difference, also for hilltops only (mask in alpha) split by
  level flight / rigid phase / transition. `LRT_PROFILE=4` (colour only) and `=5` (lighting
  only), and `LRT_POP_COLOUR=1` (geometry σ held, only the colour state varies), isolate the
  terms.

Depth limit: S ≥ 1e-12. The CPU f64 check stops matching below a sample spacing of ~1e-14,
where the absolute f64 position itself runs out of bits.
