# R4 motion sample reuse: experiment notes (2026-10-14/15)

Raw notes of the R4 runs. Machine: M1 Pro (16-core GPU, 10 CPU cores, 32 GB), macOS 26.5,
release builds, worktree branch `worktree-agent-abad90e6e8bdb3fc5`. The GPU and CPU were
shared with other agents and the user's app throughout; iteration counts and image metrics are
deterministic and unaffected; GPU timings are marked with their spread. The runs were frozen
(SIGSTOP) for ~5.5 h by the driver mid-way and resumed; counts are unaffected.

## Simulator (prototypes/motion-reuse)

- Camera path: the app's `Navigator` stepped at virtual 60 Hz refreshes from the flight scripts
  (copy of `fly`'s event feeding). Gesture frames = refreshes where `is_interacting`.
- Samples: the real GPU kernel through `Renderer::render_offscreen` with a per-frame jitter
  (`offscreen_jitter`, Halton(2,3) index k % 64 + 1 as the scheduler), iteration limit
  `depth_iterations(view)` (auto learning off), default look. Readback of iteration (n, smooth
  offset) and DE (texels, angle) fields. Interior texels carry the iterations spent in `.y`
  (kernel change), so every sample's cost is known exactly; unresolved = the limit.
- Colour: CPU port of `shade` (default look Aurora) from GPU sample data, the same for every
  strategy and the reference.
- Reference: the frame rendered on a 4 x 4 grid per pixel (16 samples, box mean), every 10th
  gesture frame and the frame before it (for the temporal metric).
- Metrics: OKLab ΔE x 100 mean and p99 per evaluated frame, averaged over evaluated frames;
  "temporal error" = mean over pixels of |(cur − ref_cur) − warp(prev − ref_prev)| in OKLab x 100
  (bilinear warp along the exact pan/zoom). Plain frame-difference flicker was tried first and
  rejected: the warp's own resampling error dominated (the reference itself scored 0.26–1.0).
- Each segment of gesture frames starts from the resting view's exact centre-sample frame (the
  app's refined frame), for every strategy.

Strategies:

- `fl`: port of `compose` mode 0 (Catmull-Rom history, decay step², cap 6, Gaussian σ 0.5 splat
  of the jittered preview, bilinear preview fallback with weight 0.02). Preview ratio from a
  deterministic model of the dynamic resolution: largest 1/8 step with
  ratio² · (mean iterations per texel of the previous preview + 30) · 1920·1080 ≤ 1.6e8.
  Calibrated: the real `fly` seahorse dive (1920x1080, 60 Hz, loaded GPU) averaged ratio 0.453;
  the model gives 0.452 over the same flight; both sit at 0.125 below ~1e6 zoom.
- `fl-full`, `fl-r0.25`, `fl-r0.5`, `fl-r0.375`: same compose, fixed ratios.
- `sc-fill1`, `sc-fill2`: sample cache (K = 4 per pixel, newest kept), a new sample (this
  frame's jittered full-resolution field) in every pixel with fewer than 1 / 2 samples.
- `sc-b{0.5,1,2}`: budget policy: an iteration budget = multiple of `fl`'s preview cost this
  frame, spent on pixels in order of fewest samples, pseudo-random inside a class.
- `hy-*`: the same with hybrid reconstruction (cache splat blended with the cache's own
  reprojected output, weight 0.3 x zoom decay, where sample weight is low).
- `xn-0.5`: XaoS-like: centre samples, a new one when no cached sample lies within 0.5 px of the
  pixel centre per axis; display the nearest sample (no filter). (`xn-0.25`: first run only.)
- `mfx-t{r}`, `mfx-s{r}`: MetalFX temporal / spatial scaler (prototypes/metalfx-probe) fed the
  same jittered preview colour at ratio r (0.5, 0.375 = 2x, 2.67x), exact affine motion
  vectors, constant depth.

Unresolved samples are evicted when the limit exceeds 1.25x theirs. Positions in f64 output
pixels re-expressed every frame (prototype; the GPU design uses birth coordinates).

## Runs (runs/)

| flight | size | gesture frames | evaluated | FL mean ratio |
|---|---|---:|---:|---:|
| seahorse-dive (Space, E at 6 s, 1 → 1e9) | 1280x720 | 726 | 72 | 0.452 |
| pan-1e5 (WASD at 1e5) | 960x540 | 477 | 47 | 0.635 |
| filament-cruise (WASD + Space at 1e6) | 960x540 | 849 | 84 | 0.358 |
| fast-dive (7.6 e-folds/s to 1e12 and back) | 960x540 | 528 | 52 | 0.866 |
| deep-zoom (2.5 e-folds/s, 1e-24 → 1e-33/px) | 960x540 | 523 | 52 | 0.125 |

`analysis-strategies.md`: all tables (whole flight and per zoom phase of 10^3) from
`analyze.py`. `analysis-metalfx.md`: MetalFX runs (seahorse 1280x720, pan 960x540).
`analysis-first-run-low-budget.md`: the first seahorse run with budget 6e7 (mean ratio 0.27)
and the first strategy set (box reconstruction, xn-0.25). Crops at 100 %: `crops/`.

Observations while looking at the crops:

- seahorse k0636 (zoom ~5e8, dense spirals): FL (ratio 0.125) is a soft blur; hy-b1 at the same
  iterations is sharp but grainy; sc-b1 shows blocky nearest-sample fill where samples are
  sparse; sc-fill1 (3.4x the iterations) is indistinguishable from the reference at 100 %.
- pan-1e5 k0330: FL visibly softer than the reference; sc-fill1 (2 % of FL's iterations) sharp;
  xn-0.5 aliased (stair-stepped, noisy texture).
- deep-zoom k0336 (~1e26): FL a blur with coloured speckle; hy-b1 noisy; sc-fill1 close to the
  reference.
- seahorse (mfx) k0336: MetalFX temporal at 2x and our compose at the same ratio look alike
  (both soft); MetalFX spatial is grainy and aliased; hy-b1 sharpest.

Anomaly: deep-zoom's last phase (1e30–1e33 per pixel zoom, 52 frames, 5 evaluated: the zoom ends
on the minibrot, mostly interior) has the cache worse than FL (ΔE 0.80 vs 0.29). Probably stale
unresolved/limit effects near the minibrot (limit grows ~1 % per frame; unresolved samples are
kept until +25 %); not investigated further.

## GPU bookkeeping of a cache (prototypes/motion-reuse --scgpu 1|2)

Synthetic content, K = 4, fill policy 1, 120 frames after 20 warm-up, median; 3 rounds each,
repeated 4 times (2 split, 2 with all passes in one command buffer and two frames in flight:
identical within 1–2 %). Spread over rounds/repeats ±5 % (pan at 1080p 3.6–4.2).

v1 (32-byte samples, separate shade pass, untiled resolve), first run under load:
1280x720 4.5–7.1 ms, 1920x1080 8.8–19 ms, 3456x2160 44–73 ms per frame.

v2 (12-byte SoA samples, tiled resolve that shades in threadgroup memory):

| size | zoom 1.6 e-folds/s | zoom 7.6 | pan 13 px | yardstick copy 16 B/px |
|---|---:|---:|---:|---:|
| 1280x720 | 2.8 (move 1.4, resolve 1.2) | 2.3 | 2.4 | 0.10–0.15 |
| 1920x1080 | 6.3 (move 3.2, resolve 2.7) | 5.1–5.2 | 3.6–4.2 | 0.29–0.35 |
| 3456x2160 | 22.6–23.6 (move 11.9, resolve 9.7) | 18.7–19.5 | 21.2–22.2 | 1.05–1.20 |

u8 vs u32 counts: no difference. The move (gather by preimage) pass runs at ~10x the
bandwidth yardstick; not understood yet (latency-bound gather with divergent loops is the
suspect). For comparison FL's per-frame compose + preview colour measured 1.67 ms at 1920x1080
(`fly` seahorse, 2026-10-14).

## MetalFX (subagent, see metalfx-notes.md)

Temporal scaler on M1 Pro: 1.0–3.0x per axis only; 1.0–1.5 ms at 1600x1000, 4.7–6.6 ms at
3456x2160 (min over runs, 2x); synthetic high-frequency pan with fractional motion: no gain over
bilinear.

Quality on our flights (same jittered preview as the FL compose at that ratio):

| flight | ratio | FL compose ΔE / p99 / temporal | MetalFX temporal | MetalFX spatial |
|---|---|---|---|---|
| seahorse 1280x720 | 0.5 | 1.66 / 13.8 / 0.79 | 1.61 / 14.5 / 0.81 | 3.28 / 25.6 / 4.33 |
| seahorse | 0.375 | 1.83 / 15.1 / 0.81 | 1.92 / 17.1 / 0.86 | 3.66 / 28.2 / 4.73 |
| pan-1e5 960x540 | 0.5 | 1.06 / 10.5 / 0.59 | 1.14 / 11.0 / 0.62 | 2.74 / 23.0 / 3.66 |
| pan-1e5 | 0.375 | 1.19 / 11.8 / 0.60 | 1.51 / 14.5 / 0.66 | 3.12 / 26.1 / 3.98 |
