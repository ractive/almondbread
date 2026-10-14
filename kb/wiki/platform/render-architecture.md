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
platform-neutral pieces (viewport, settings, navigation, oracle) in the core crate. The
Swift renderer (`Sources/AlmondbreadKit/Renderer.swift`) was the reference; the port makes
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
                                   scripted sessions
                     shaders/mandelbrot.metal  the MSL kernels (include_str!)
                     metal/        MetalBackend (objc2-metal), MetalLayerSurface (CAMetalLayer)
                                   cfg(target_vendor = "apple"); other targets build without
almondbread-cli      binary `almondbread`: render, verify (--fields, --history), bench, regress
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
stay. Headless, a layer without a window passes the hosting test; the real check is the
R2b interaction test (draw CPU p50 near 0.15 ms in neck zoom, frame rate unchanged).

## Hosting in the app (R2b)

The Rust side is in place; R2b wires it through the FFI crate:

1. **Layer handover, once:** the Swift view creates its `CAMetalLayer` (an `MTKView`'s
   layer or a plain layer-backed view) and passes the pointer as `u64` to an exported
   `Engine::new(layer)`; Rust retains it (`MetalLayerSurface::from_raw`), sets device,
   `BGRA8Unorm`, `framebufferOnly = false`, and owns device, queue, buffers and pipelines.
   `resize(width_px, height_px, points_per_pixel)` on layout changes.
2. **Per frame, one call** from the display link: `frame(timestamp) -> { animating }`
   runs `Navigator::advance`, `render_frame` and presentation; Swift pauses its display
   link when `animating` is false and resumes it on the next event.
3. **Events:** decoded platform events cross as the `InputEvent` enum (`KeyDown/KeyUp`
   of `NavKey`, `Shift`, `FocusLost`, `Resize`, `Pointer`, `Pan`, `WheelZoom`,
   `Magnify`, `Pinch`, `DoubleTap`). Swift keeps only decoding (key codes, gesture
   recognisers, coordinates to points from the centre, y up) and calls
   `input(event) -> handled`; pan speed, zoom target, zoom speed, diagonals and the
   idle/gesture timing live in `almondbread_core::input`.
4. **Settings and HUD:** settings as a UniFFI record set on change; `ShownStats` read
   at ~10 Hz for the HUD (never per input event, [[synthesis/benchmarks]] M2 findings).

Each of these is a handful of scalar calls per frame (3.6 ns each,
[[platform/rust-swift-ffi]]); nothing per pixel crosses.

## Pitfalls

- `nextDrawable` blocking is invisible in GPU timings; measure draw CPU per phase.
- objc2 returns autoreleased Metal objects as `Retained`; long headless loops run inside
  an autorelease pool (`MetalBackend::pool`).
- A shared texture read with `getBytes` needs the queue drained first (the backend
  commits and waits on an empty command buffer).
- `setBytes` is limited to 4 KB; the palette and orbit go in buffers.
