---
title: "Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)"
type: platform
status: draft
sources: ["[[sources/mlx-watchdog-issue]]", "[[sources/apple-metal-compile-options]]"]
tags: [rust, metal, gpu, scheduler, chunking, architecture, objc2, interaction]
---

# Render architecture: GPU backend trait, chunked scheduler, Metal backend

How the renderer is built in Rust since R2a of [Rust core with a Rust Metal backend; Swift for UI only](../decisions/rust-core.md): one scheduler
written against a small GPU backend trait, a Metal backend on objc2-metal, and the
platform-neutral pieces (viewport, settings, navigation, oracle) in the core crate; since
R2b both apps render through it and Swift is UI only. The Swift renderer
(`Sources/AlmondbreadKit/Renderer.swift`, deleted in R2b) was the reference; the port makes
the same decisions with the same constants and kernel inputs, so iteration fields are
bit-identical ([almondbread benchmarks and verification record](../synthesis/benchmarks.md), R2a section). The scheduler's design is the one
on [GPU watchdog and chunked, resumable kernels](gpu-watchdog-and-chunking.md) and [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md);
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
state. Everything on [GPU watchdog and chunked, resumable kernels](gpu-watchdog-and-chunking.md) (As built) carries over
unchanged: jobs per view with generations (cancellation by ignoring old generations),
GPU-side admission in `prepareChunk`, ping-ponged live lists with order-preserving
compaction, K from the damped throughput model (16 ms idle, 8 ms gesture chunks), the
64-slot stats ring, gesture frames with dynamic preview resolution (1/8 steps from the
measured cost per texel, 6 ms budget), refinement over the bilinear preview, unresolved
extension passes (x4 / x2, 2 GPU-second estimated budget, never cut short), the adaptive
interior test ([Interior detection](../techniques/interior-detection.md)) and the learned limit
([Automatic maximum-iteration control](../techniques/auto-iterations.md), pure functions in `core::autoiter`). Failed chunks divide
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
results bit-identical; [Metal compute pipeline for a deep-zoom renderer](metal-compute.md)).

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
([almondbread benchmarks and verification record](../synthesis/benchmarks.md), R2b). Two corrections to the design were needed:

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
M2.5's 120 Hz runs had the panel alone. 2026-10-15 the same window got only 20–33
callbacks per second, and with frames offered by a `CVDisplayLink` at 120 Hz half the
refreshes found no free drawable ([almondbread benchmarks and verification record](../synthesis/benchmarks.md) "120 Hz on ProMotion"): 120 Hz
presentation needs a frontmost window (`make interaction-test-120`, run by a person).
So FL's numbers are 60 Hz; the headless flight
recorder (`almondbread fly --hz 120`) can drive 120 Hz refreshes with real budgets.

## FL: compose pass, history and progressive AA

Interactive frames no longer colorize the shown level directly: a `compose` pass blends a
reprojected history image with new samples (gesture: temporal upsampling of jittered
previews; refine: per-pixel fade; idle: adaptive AA samples) and writes both the drawable
and the next history ([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md) FL,
[Anti-aliasing and supersampling](../techniques/anti-aliasing.md) FL). Offscreen renders keep `colorize` with grid supersampling.
Levels admit 16×16 tiles in raster or priority order. `Renderer::frames_committed` /
`frames_completed` and `GpuBackend::read_shared_texture` let headless capture read frames
without draining the queue.

## Motion sample cache (2026-10-15; code removed 2026-10-17)

**Removed 2026-10-17.** The cache was opt-in and never the default (it made zoom-out blurry in the
real window), so `sample_cache.rs`, its kernels and switches were deleted. This section describes
what was built, as history; see [Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md).

Gesture frames no longer render a fresh jittered preview each refresh. They reuse the exact
samples of earlier frames, kept on a lattice fixed in the plane, and compute only the tiles
the view lacks ([Sample reuse while zooming and panning (sample cache in fractal coordinates)](../techniques/sample-reuse.md) "As built", [Motion sample reuse: fewer iterations per gesture frame (R4)](../synthesis/motion-sample-reuse.md)).
The module is `sample_cache.rs` (`SampleCache<B>`, owned by the renderer); its kernels are
`shaders/sample_cache.metal`, appended to `mandelbrot.metal` at compile time. The scheduler
hooks are narrow:

- `attach_cache_level` replaces a fresh gesture job's preview level with a cache level:
  the cache's toroidal window textures, a tile list as admission order, and lattice
  parameters (`halfSize`, `rot`, `wrap`).
- An unfinished cache batch moves into the next frame's job (`take_cache_level`), and its
  in-flight chunk records are renamed to the new generation.
- `encode_chunk` takes the epoch's pinned BLA/interior regime; cache levels never seed
  (M5) or use LA (M6).
- `harvest` reports every level-0 chunk to the cache (`on_chunk`), superseded jobs
  included.
- `encode_compose` shades the cache (its own command buffer, committed first) and sets the
  compose pass's cache mapping.
- `seed_sample_cache` seeds an epoch from each settled full level. `make_key` resets the
  cache on a jump.

Switches were `ALMONDBREAD_SAMPLE_CACHE`, `_CACHE_TRACE`, `_CACHE_RATIO`, `_CACHE_TEXELS`. Two texture sets plus colour cost
~32 B per window texel: 4.2 M visible texels at most, ~1.4× that allocated (rotation slack),
so up to ~190 MB at Retina size.

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
   [Controls: keyboard, pointer and touch](../decisions/controls.md) As built). An idle refresh whose job is finally presented with
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
   10 Hz, [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M2 findings).
5. **Self-test and interaction test** run through the same path: the self-test waits for
   `stats().settled`, `finishFrames()` (GPU completion), `displayedFrameMismatch()` and
   writes an offscreen PNG (`renderPng`). The interaction test brackets each scenario with
   `beginMeasurement`/`endMeasurement`. Presentation times come from the drawables'
   presented handlers, tagged per drawable, and Swift times the `frame` call to get the
   FFI overhead.

**Per-frame FFI cost:** Swift-side time of `frame` minus the time Rust measures inside it:
2–7 µs in the window (incl. Rust's autorelease pool and the presentation-log drain),
0.2–1 µs on a windowless layer; `input` ~0.9 µs, `stats()` ~6 µs
([Rust core to Swift: UniFFI and the XCFramework pipeline](rust-swift-ffi.md)). A frame is one call; nothing per pixel crosses.

**Platform-neutral split.** `Host<B>` holds everything a Windows or Linux UI needs besides
its own surface: view size and first fit, input forwarding with the redraw decision, the
frame call with a `target_ready` gate and an `acquire(final_frame)` closure, and
measurements. A new platform supplies a backend, a surface with a non-blocking readiness
check and presentation timestamps, and the event decoding.

## M3b: any depth

- **Viewport** (`core::viewport`): `Center` is a `BigComplex` whose precision follows the zoom
  (point spacing + 96 guard bits); `Scale` is an f64 mantissa with an i64 exponent and rounds like
  f64 inside f64's range. Pans and zooms add `dx · scale` exactly; offsets between centres are
  taken in units of a scale (`delta_in`), which history reprojection, tile priority and kernel
  parameters use. Coordinates cross the FFI as decimal strings with the digits the depth needs;
  `set_viewport_text`, `paste_coordinates` and the "Go to Copied Coordinates" menu take them back.
- **References**: f64 orbit (synchronous) above 1e-12 per point; big references from a
  `ReferenceWorker` thread below ([Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md) M3b). The renderer adopts a
  finished one when it serves the view at least as well as the current one; a deep view without
  a usable reference has no job, and each refresh composes the history reprojected into the
  current view (`encode_history_only`, counted as `waiting_frames`) until it arrives. Offscreen
  renders wait for it.
- **Kernels**: the iterate pipeline gains `kDeep` (function constant 3; buffers 16: floatexp
  reference points, 17: extended-range BLA table); `IterateParams` 112 B (`scaleExp`),
  `PixelState` 48 B (delta and DE exponents; the plain kernel ignores them).
  [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md) "As built (M3b)".
- **Gesture previews** may drop to 1/8 resolution (was 1/4): dense deep views need 10^4+
  iterations per texel.

## F2: pluggable formulas (2026-10-19)

The formula is a parameter of the engine ([F2: Pluggable-formula refactor](../plans/f2-pluggable-formulas.md)); Mandelbrot stays
formula 0 and its code is the pre-F2 code.

- **Maths** (`almondbread-math/src/formula.rs`): `Formula { Mandelbrot, Multibrot(p) }`, an enum
  matched outside the hot loops. `Multibrot(p)` is the generic `z^p + c` path (test-only until F3).
  Hooks: `step`, `prime` (`f'`), `second` (`f''`), `delta_factor` (`S` of `z' = z S + c`, `W = Z + z`,
  Horner) in f64 and extended range. `ReferenceOrbitComputer::with_formula` runs one generic loop
  (`p - 1` complex products on the limb registers) for non-Mandelbrot formulas, never the
  three-thread split. Selection has `find_period_for`, `atom_domain_period_for`,
  `find_nucleus_for`, `select_reference_for` (the old names are Mandelbrot): ball bound
  `R' = (|z| + R)^p - |z|^p + r`; Newton stays in the extended-range derivative (no exact finish)
  and no minibrot size estimate for other formulas.
- **Core**: `RenderSettings::formula`; `formula::FormulaCapabilities` (platform-free; the FFI
  mirrors it): perturbation, rebasing, BLA form, LA, SA seeds, derivative interior test, probe,
  closed-form components, DE, seek, atom domain, π meter, external rays, orbit overlay, content
  (places, tours, journeys), Julia panel. Per-formula hooks in `reference.rs`
  (`compute_f64_with`, `ReferenceRequest::formula`, `Reference::formula`), `oracle.rs`
  (`cpu_render_for`, `cpu_render_deep_for`, `OracleReference::formula`), `orbit.rs`
  (`ProbePoint::formula`), `bla.rs` (`build_for`: `A = f'(Z)`, `R = eps 2|Z|/(p-1)`), `cycles.rs`
  (`view_periods_for`). Places, tours (header `formula:`) and journey anchors carry a formula.
- **Kernels**: function constants `kFormula` (16), `kPower` (17), `kConformal` (18, unused until
  F5) read through `is_function_constant_defined`, so a pipeline that never sets them is formula
  0. Every step site of `iterate` and `seedLevel` calls an inline hook (`fStep`, `fMulPrime`,
  `fPrime`, `fPrimeX`, `fStepX`, `fSeedB`, `fSeedBX`, `fSeedWDX`, `fProbeF2`, `fProbeF2X`, `fIcCZ`,
  `fIcZZ`) whose `kFormula == 0` branch is the expression the site had before. The closed-form
  component colouring stays Mandelbrot (gated on the CPU). Mandelbrot pipelines are created
  exactly as before (no integer constants), so their keys, warm-up and shader cache are unchanged;
  other formulas compile lazily (`formula_pipelines`, `GpuBackend::pipeline_with`).
- **Scheduler**: `ComputeKey::formula`; a formula change is a jump (references, spare, worker,
  BLA/LA tables dropped; `set_reference` refuses another formula's orbit). LA (`prefer_la`), the
  closed-form test and SA seeds are gated by capabilities.
- **FFI / Swift**: `ExplorerSettings::formula` (selectable formulas only), `capabilities()`,
  `formulas()`; places and tours switch to their formula; journeys, seek, the π meter and the
  Julia panel are off for a formula without them; Swift hides those controls (`model.has(\.x)`).

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
