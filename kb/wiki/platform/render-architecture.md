---
title: "Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)"
type: platform
status: draft
sources: ["[[sources/mlx-watchdog-issue]]", "[[sources/apple-metal-compile-options]]"]
tags: [rust, metal, gpu, scheduler, chunking, architecture, objc2, interaction]
---

# Render architecture: GPU backend trait, chunked scheduler, Metal backend

How the renderer is built in Rust since R2a of [[decisions/rust-core]]: one scheduler
written against a small GPU backend trait, a Metal backend on objc2-metal, and the
platform-neutral pieces (viewport, settings, navigation, oracle) in the core crate; since
R2b both apps render through it and Swift is UI only. The Swift renderer
(`Sources/AlmondbreadKit/Renderer.swift`, deleted in R2b) was the reference; the port makes
the same decisions with the same constants and kernel inputs, so iteration fields are
bit-identical ([[synthesis/benchmarks]], R2a section). The scheduler's design is the one
on [[platform/gpu-watchdog-and-chunking]] and [[techniques/frame-reuse-and-progressive]];
this page covers how it is cut into crates and traits, and what changed in the port.
Numbers: measured 2026-10-14, M1 Pro, release, unless noted.

## Crates

```text
almondbread-core     viewport (Center type, M3b-ready), RenderSettings, palettes, f64 reference
                     orbit, autoiter (learned limit, interior-test switch), oracle (CPU f64
                     iteration with rayon, IterationField .iter dumps, verify metrics and
                     heatmap), benchmark locations, PNG output, input (Navigator), cfmt (%g)
almondbread-render   backend.rs    GpuBackend / ComputeEncoder / Submission traits
                     gpu_types.rs  #[repr(C)] kernel structs + compile-time layout asserts
                     scheduler.rs  Renderer<B: GpuBackend>: jobs, chunks, colorize, offscreen,
                                   scripted sessions, shown frame vs fresh render
                     host.rs       Host<B>: the renderer in a view (size, first fit, input,
                                   one frame per refresh, interaction measurements)
                     shaders/mandelbrot.metal  the MSL kernels (include_str!)
                     metal/        MetalBackend (objc2-metal), MetalLayerSurface (CAMetalLayer)
                                   cfg(target_vendor = "apple"); other targets build without
almondbread-cli      binary `almondbread`: render, verify (--fields, --history), bench, regress
almondbread-ffi      Explorer (UniFFI): Host<MetalBackend> + MetalLayerSurface for the Swift UI
```

Core stays `forbid(unsafe_code)` and platform-free; all `unsafe` and objc2 code sits in
`almondbread-render/src/metal/`. `cargo check` of core, render and cli passes for
Linux (gnu) and Windows (msvc); there the CLI reports "no GPU backend".

## The backend trait

A compute-only subset of a modern GPU API, enough for the scheduler and nothing more:

| Item | Purpose |
| --- | --- |
| `type Buffer, Texture, Pipeline: Clone` | reference-counted handles; a clone keeps the resource alive (a job holds its reference-orbit buffer and level textures while its chunks run) |
| `create_buffer(len, Private/Shared)`, `create_buffer_with_data`, `read_buffer` | live lists, counters, stats ring (shared), palette and reference orbit |
| `create_texture(w, h, format, storage)`, `texture_size`, `read_texture(region)` | rg32f iteration, rg16f DE, rgba16f preview colour, bgra8 targets; readback of the used region |
| `pipeline(function, bool constants)`, `pipeline_limits` | function-constant specialisation (`kDE`, `kInterior`), SIMD width for the colorize grid |
| `begin(label) -> Encoder` | one command buffer with one compute pass |
| `Encoder`: `set_pipeline`, `set_bytes`, `set_buffer`, `set_texture`, `dispatch_threads`, `dispatch_indirect`, `present(drawable)`, `commit() -> Submission` | the chunk's three dispatches (prepare, indirect iterate, finish) and colorize |
| `Submission`: `status()` (pending / completed / failed), `gpu_time()`, `wait()` | harvest in commit order, GPU timestamps for the rate model, watchdog errors |

No completion callbacks: the scheduler polls submission status at each refresh (the Swift
renderer used completion handlers that hopped to the main queue). Polling is simpler
across FFI and threads, and observes completion no later than the handlers did.

## Scheduler

`Renderer<B>` owns the backend, `viewport`, `settings`, the `Navigator`, and the GPU
state. Everything on [[platform/gpu-watchdog-and-chunking]] (As built) carries over
unchanged: jobs per view with generations (cancellation by ignoring old generations),
GPU-side admission in `prepareChunk`, ping-ponged live lists with order-preserving
compaction, K from the damped throughput model (16 ms idle, 8 ms gesture chunks), the
64-slot stats ring, gesture frames with dynamic preview resolution (1/8 steps from the
measured cost per texel, 6 ms budget), refinement over the bilinear preview, unresolved
extension passes (x4 / x2, 2 GPU-second estimated budget, never cut short), the adaptive
interior test ([[techniques/interior-detection]]) and the learned limit
([[techniques/auto-iterations]], pure functions in `core::autoiter`). Failed chunks divide
the rate by 8 and restart the job.

Changes from the Swift renderer:

- **Time is an argument.** `render_frame(now, width, height, points_per_pixel, acquire)`
  takes the host's timestamp; the headless tools use `Renderer::clock()`. Navigation
  integrates held keys from the same timestamps.
- **Preview textures are allocated once** at the largest preview size (the output size;
  preview ratios are at most 1) and levels use the top-left corner. The Swift renderer
  cached up to four texture sets by size; the dynamic resolution changes size every few
  frames. Colorize takes the used sizes in `ColorParams` (`srcSize`, `outSize`,
  `underSize`; 96 → 128 bytes) instead of `get_width()`, and clamps bilinear preview
  samples half a texel inside the used region, which is what `clamp_to_edge` did for an
  exactly sized texture. `iterate` is unchanged.
- **The target is acquired through a closure** that may return None (no drawable without
  blocking); the refresh then skips presentation instead of stalling (next section).
- **Kernel struct layouts are checked twice:** `offset_of!` asserts at compile time, and a
  test compiles a probe kernel with the shipped structs and compares `sizeof`/`alignof`
  as the Metal compiler lays them out.

Kernels compile at runtime from `include_str!` source with the Swift options: MSL 3.0,
`fastMathEnabled = true` (deprecated in favour of `mathMode`, but the same setting keeps
results bit-identical; [[platform/metal-compute]]).

## The 9.4 ms draw CPU in "neck zoom" (Swift), and the design answer

`--interaction-test` reported draw CPU p50 9.4 ms in the neck-zoom scenario against
~0.15 ms elsewhere (60 Hz display, 2560×1640 px). Splitting the draw into phases
(temporary instrumentation, measured 2026-10-14, M1 Pro):

| phase (p50) | home pan | neck zoom |
| --- | ---: | ---: |
| harvest | 0.012 ms | 0.008 ms |
| key / start job (incl. texture lookup, reference) | 0.008 ms | 0.010 ms |
| encode chunks | 0.073 ms | 0.048 ms |
| `view.currentDrawable` | 0.034 ms | **8.9 ms** |
| colorize + commit | 0.026 ms | 0.064 ms |

Not texture allocation (`startJob` p50 0.03–0.2 ms, max 1.7 ms) and not GPU load: with
the preview floor lowered to 1/16 the GPU time per frame fell from 9.8 to 3.2 ms and the
wait stayed at 9 ms (draw→glass 74 ms either way). With one frame in flight instead of two
the wait vanished (0.03 ms, latency 54 vs 65–81 ms) but frame rate dropped on heavy views
(space zoom 47.7 vs 56.3 fps, 34 skipped refreshes). So the presentation queue had grown
one frame deeper than the drawable pool and stayed there: every refresh blocked in
`nextDrawable` until the compositor released one, which keeps the queue full
(self-sustaining once a slow frame starts it).

The Rust design: `MetalLayerSurface` predicts the pool. A drawable returns to the pool when
its successor is on screen, so `in use = acquired − max(presented − 1, 0)` with `presented`
counted by `addPresentedHandler` (an atomic, any thread). When `in use` reaches
`maximumDrawableCount`, `next_drawable` returns None and the refresh skips presentation;
the queue drains by one instead of blocking. If presentation counts stall for 0.25 s (a
hidden window may never report), it falls back to the plain acquire. Two frames in flight
stay.

**Outcome (R2b, measured 2026-10-14, M1 Pro, 60 Hz display, interleaved runs):** the stall
is gone. Neck zoom draw CPU p50 Swift 8.6–8.9 ms (it shows in neck zoom every round, and in
WASD, Space and limit zoom in some), Rust 0.1–1.0 ms, at the same frame rate
([[synthesis/benchmarks]], R2b). Two corrections to the design were needed:

- **The strict count is too conservative.** With `in use = acquired − (presented − 1)`
  every skip was "no drawable" and the test ran at 36–38 fps: the presented callback
  arrives about a refresh after the drawable reached the glass, which hides the drawable
  that is already free. `in use = acquired − presented` (the lag cancels the −1) gives
  54–59 fps; the acquire then waits at most ~1 ms (draw CPU p50 0.8–0.9 ms in neck and
  limit zoom), never a refresh. (The blocking reference is the Swift renderer's
  `currentDrawable`, same fps with the 9 ms wait; a blocking-mode run of the Rust surface
  was occluded and gave no presentation times.)
- **Gate before encoding, but only gestures.** A gesture frame whose drawable is then
  missing wastes its GPU work and delays the next one (the first Rust run, strict count
  and the check only at acquire: neck zoom 38.7 fps, 17.8 GPU ms per encoded frame). So `render_frame_gated(…, target_ready, …)` skips a
  gesture refresh before encoding when the surface predicts no drawable. Refinement keeps
  encoding its chunks and only skips presenting; gating it too delayed the final frame by
  ~50 ms. The view's refined final frame takes the plain acquire (`acquire(final_frame)`),
  waiting at most about a refresh.

## FL: drawable pre-acquisition instead of pool prediction

The presented-count prediction stayed too conservative: at 120 Hz heavy zooms skipped
12–21 refreshes as "no drawable" where the blocking Swift renderer skipped none (M2.5), and
at 60 Hz neck zoom still skipped 16–18 (baseline 2026-10-14). The presented callback lags
the real pool, and Core Animation can return a drawable before reporting it presented.

`MetalLayerSurface` now runs a small thread that calls the plain, blocking `nextDrawable`
and parks the drawable in a one-slot mailbox (mutex + condvar); the main thread never
blocks. `drawable_ready` is true exactly when a drawable of the current size is parked
(a stale size is dropped back to the pool), so a refresh is skipped only when a blocking
acquire would really have waited, which is the Swift renderer's behaviour minus the stall.
The refined final frame may wait up to 17 ms for the slot. Measured at 60 Hz (the
background test window gets 60 display-link callbacks per second even on the 120 Hz panel,
see below): neck zoom 53.7 → 58.7 fps, "no drawable" skips 16 → 2; space zoom 50 → 55 fps.

**120 Hz in background windows.** With an external 60 Hz display attached, the interaction
test's window on the ProMotion panel reports `duration` 8.33 ms but receives 60 callbacks
per second whatever `preferredFrameRateRange` asks for (both links; 2026-10-14, macOS 26).
M2.5's 120 Hz runs had the panel alone. So FL's numbers are 60 Hz; the headless flight
recorder (`almondbread fly --hz 120`) can drive 120 Hz refreshes with real budgets.

## FL: compose pass, history and progressive AA

Interactive frames no longer colorize the shown level directly: a `compose` pass blends a
reprojected history image with new samples (gesture: temporal upsampling of jittered
previews; refine: per-pixel fade; idle: adaptive AA samples) and writes both the drawable
and the next history ([[techniques/frame-reuse-and-progressive]] FL,
[[techniques/anti-aliasing]] FL). Offscreen renders keep `colorize` with grid supersampling.
Levels admit 16×16 tiles in raster or priority order. `Renderer::frames_committed` /
`frames_completed` and `GpuBackend::read_shared_texture` let headless capture read frames
without draining the queue.

## Hosting in the app (as built, R2b)

```text
Swift (AlmondbreadKit)                          Rust (almondbread-ffi Explorer -> render Host)
FractalNSView / FractalUIView
  CAMetalLayer, created in Swift  -- address, once -->  MetalLayerSurface::from_raw (retain,
                                                        device, BGRA8, not framebuffer-only)
  layout / backing change  -- resize(w pt, h pt, scale) --> contentsScale, drawableSize,
                                                        Navigator Resize, first fit of home
  display link (NSView.displayLink /
  CADisplayLink), proxy target  -- frame(timestamp) -> bits --> pool -> Host::frame:
                                                        Navigator::advance, render_frame_gated,
                                                        present; !animating: Swift pauses the
                                                        link; keyboard bit: crosshairX/Y()
  key monitor, gestures, pointer  -- input(InputEvent) -> {handled, redraw, mode} --> Navigator
ExplorerModel (@Observable)  -- setSettings(record) on change; stats() at ~10 Hz -->
```

1. **Layer handover, once.** The platform view creates a plain `CAMetalLayer` (macOS:
   the view's backing layer via `makeBackingLayer`; iOS: a sublayer) and passes
   `Unmanaged.passUnretained(layer).toOpaque()` as `u64` to `Explorer(layer:scale:)`. Rust
   owns device, queue, pipelines, buffers and drawables.
2. **One call per refresh.** The display link's callback calls `frame(timestamp)`; Rust
   uses the later of the timestamp and `CACurrentMediaTime()` as "now". macOS reports the
   last vsync, up to a refresh in the past, and scheduling on it started refinement a
   refresh late (+15 ms to the final frame). When `frame` returns false the link pauses;
   any `input` whose result says `redraw`, a settings change or a resize resumes it.
   Shift and pointer moves do not, unless the move switches the input mode or starts edge
   panning. The result's bits also carry the input mode; in keyboard mode Swift reads the
   crosshair's position with two scalar calls and applies it to a Core Animation overlay layer right after the call (M2.5,
   [[decisions/controls]] As built). An idle refresh whose job is finally presented with
   unchanged settings presents nothing (before M2.5 every `frame` call re-coloured and
   re-presented the same image; the crosshair glide would have done so ~60 times).
3. **Events.** Swift decodes only: key codes and characters to `NavKey` (WASD/arrows,
   Space, Q/E, +/−, R) through the window-local monitor (inspector focus keeps working,
   handled keys are swallowed), `FocusLost` on resign-key or first-responder loss,
   pointer moves through a tracking area (the HUD counts as off-canvas), safe-area insets,
   scroll/magnify/wheel, double-click, iOS pinch/pan/taps, iPad hover and hardware keys. Positions are points from the view
   centre, y up. Speeds, zoom target, idle timing and the precision clamp are Rust's.
4. **Settings and HUD.** `ExplorerModel` mirrors the settings record for SwiftUI bindings
   and forwards every change (`setSettings`). It reads `stats()` at ~10 Hz while the link
   runs and once when it stops, assigning only on change (SwiftUI invalidation stays at
   10 Hz, [[synthesis/benchmarks]] M2 findings).
5. **Self-test and interaction test** run through the same path: the self-test waits for
   `stats().settled`, `finishFrames()` (GPU completion), `displayedFrameMismatch()` and
   writes an offscreen PNG (`renderPng`). The interaction test brackets each scenario with
   `beginMeasurement`/`endMeasurement`. Presentation times come from the drawables'
   presented handlers, tagged per drawable, and Swift times the `frame` call to get the
   FFI overhead.

**Per-frame FFI cost:** Swift-side time of `frame` minus the time Rust measures inside it:
2–7 µs in the window (incl. Rust's autorelease pool and the presentation-log drain),
0.2–1 µs on a windowless layer; `input` ~0.9 µs, `stats()` ~6 µs
([[platform/rust-swift-ffi]]). A frame is one call; nothing per pixel crosses.

**Platform-neutral split.** `Host<B>` holds everything a Windows or Linux UI needs besides
its own surface: view size and first fit, input forwarding with the redraw decision, the
frame call with a `target_ready` gate and an `acquire(final_frame)` closure, and
measurements. A new platform supplies a backend, a surface with a non-blocking readiness
check and presentation timestamps, and the event decoding.

## Pitfalls

- `nextDrawable` blocking is invisible in GPU timings; measure draw CPU per phase.
- objc2 returns autoreleased Metal objects as `Retained`; long headless loops run inside
  an autorelease pool (`MetalBackend::pool`).
- A shared texture read with `getBytes` needs the queue drained first (the backend
  commits and waits on an empty command buffer).
- `setBytes` is limited to 4 KB; the palette and orbit go in buffers.
- An occluded window's drawables report no presentation time (`presentedTime` 0), so
  frame rate and latency cannot be measured behind other windows. The interaction test
  puts its window, still rearmost and never key, in the bottom-right corner of its screen,
  where it usually shows some uncovered desktop. It prints a warning when nothing was
  presented.
- Ordering a test window back when its content view moves in is too early: SwiftUI shows
  the window afterwards, in front of the user's windows. `TestLaunch.placeInBackground`
  orders it back again on the next run-loop turns (measured: z-index 1 before, rearmost
  after).
- A separate path argument to the app (`--selftest-png out.png`) makes AppKit treat it as
  a file to open, and SwiftUI then opens no window. Use `--flag=value`.
