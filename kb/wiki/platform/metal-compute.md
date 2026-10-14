---
title: "Metal compute pipeline for a deep-zoom renderer"
type: platform
status: draft
sources: ["[[sources/apple-metal-compile-options]]", "[[sources/metal-benchmarks-readme]]", "[[sources/luma-gl-gpu-precision]]", "[[sources/mlx-watchdog-issue]]", "[[sources/fraktaler-3-site]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/swift-mandelbrot-readme]]"]
tags: [metal, pipeline, function-constants, unified-memory, compilation]
---

# Metal compute pipeline for a deep-zoom renderer

How almondbread should drive Metal. Several Apple API details could not be
checked because Apple's documentation pages did not render during research;
those are marked (unverified). The decision to use Metal is in
[[decisions/app-stack]].

## Runtime compilation and math mode

- Shaders are compiled at runtime from source with `MTLDevice.makeLibrary(source:options:)`
  ([[decisions/app-stack]]). `MTLCompileOptions.mathMode` (`.safe`, `.relaxed`,
  `.fast`) supersedes the deprecated `fastMathEnabled`, and
  `mathFloatingPointFunctions` (`.fast`/`.precise`) controls library function
  precision (src: [[sources/apple-metal-compile-options]]). Exact semantics, the
  default and the OS version that introduced them are (unverified); see
  [[questions/metal-fast-math-defaults]].
- Rule: compile the float-float library ([[techniques/double-float]]) with
  `.safe` and keep `.fast`/`.relaxed` elsewhere. Error-free transforms
  (TwoSum, TwoProd) rely on exact rounding events; "a compiler that reassociates
  the expression can therefore erase the information"
  (src: [[sources/luma-gl-gpu-precision]] §Why This Is Fragile In WGSL And Metal).
  fp32 FMA throughput is the same either way (src: [[sources/metal-benchmarks-readme]] §Operations per Second),
  so `.safe` costs little for perturbation (inference).
- Startup known-answer test: run a tiny kernel computing TwoSum(1, 1e-8) and
  check that the low limb is nonzero; luma.gl's validation advice is to inspect a
  required nonzero low component on real hardware
  (src: [[sources/luma-gl-gpu-precision]] §Validation Is Part Of The Technique).

## Specialisation with function constants

- Generate one pipeline per (formula, number type, feature set) with
  `MTLFunctionConstantValues` and `makeFunction(name:constantValues:)` so dead
  branches are removed at compile time (unverified API details; standard Metal feature).
- Why: on M1/M2 worst-case register use sets occupancy
  ([[platform/apple-gpu-architecture]]), and mathr found that generating code per
  formula, instead of branching per iteration, "runs much faster"
  (src: [[sources/mathr-deep-zoom-theory]] §Code Generation).
- Suggested axes (inference):

| Constant | Values |
| --- | --- |
| number type | float32, rescaled float32 + floatexp, float-float + exponent |
| skip method | none, BLA, LA/AT |
| derivatives | off, dz/dc (DE), dz/dz (interior) |
| formula | opcode list compiled to straight-line code |

## Memory: shared buffers

- Apple GPUs use unified memory. A reference orbit or skip table written by the
  CPU into a `storageModeShared` `MTLBuffer` needs no copy (unverified in a fetched
  source; standard Apple guidance "Choosing a resource storage mode for Apple GPUs").
- Double-buffer: the CPU writes the next reference orbit and table into buffer B
  while the GPU renders from A; hand over with a command-buffer completion handler
  or `MTLSharedEvent` (inference). Do not make a command buffer wait long on a
  CPU-signalled event: waiting time counts toward the watchdog
  (src: [[sources/mlx-watchdog-issue]]).

## Dispatch, timing and compaction

- Each command buffer should carry 10 to 30 ms of work; measure with
  `gpuStartTime`/`gpuEndTime` (or `kernelEndTime`) and adapt the per-dispatch
  iteration budget (inference; API (unverified)). Details and scheduler pseudocode:
  [[platform/gpu-watchdog-and-chunking]].
- Indirect dispatch (`dispatchThreadgroups(indirectBuffer:...)`) launches the next
  chunk over a compacted list of still-live pixels whose count was produced on the
  GPU by an atomic counter, with no CPU round trip (inference).
- Fraktaler 3 schedules OpenCL work in tiles (default 128x128); bigger tiles are
  faster but risk OS timeouts (src: [[sources/fraktaler-3-site]]).

## MetalFX

- Spatial upscaling could render interactive previews at 50% resolution.
  Temporal upscaling needs motion vectors and jitter, which are trivial for 2D
  pan/zoom, but it may smear filament detail (speculative, no source).

## Prior art on Metal

- swift-mandelbrot (SwiftUI + Metal, iPhone/iPad/Mac) steps from Float to
  double-float to perturbation with BLA and rebasing and claims zooms past 10^1000
  (src: [[sources/swift-mandelbrot-readme]]). See [[implementations/swift-mandelbrot]].

## Measured in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, release, macOS 26 (Darwin 25.5), Xcode 26.

- **Xcode precompiles `.metal` resources.** A SwiftPM `.copy("Shaders/x.metal")`
  resource is compiled by Xcode into `default.metallib` (with Xcode's own math options)
  and the source is dropped, so `Bundle.module.url(forResource:withExtension: "metal")`
  returns nil in the .app. The Swift renderer shipped the source as `Mandelbrot.metalsrc`
  (copied verbatim by both SwiftPM and Xcode, compiled at runtime with explicit options).
  Since R2b the kernels live only in the Rust crate (`include_str!`, compiled at runtime
  with the same options; [[platform/render-architecture]]), so no bundle resource is
  involved.
- **Function constants** (`kDE`, `kInterior`) select four iterate pipelines; a disabled
  feature costs nothing per iteration.
- **Float atomics are slow**: `atomic_float` add, one per SIMD-group per chunk, cost
  ~28 ms per million threads; `atomic_uint` is fine
  ([[platform/gpu-watchdog-and-chunking]]).
- **`MTLDrawable.addPresentedHandler` is not in the iOS simulator SDK** (compile
  error); guard it with `#if os(macOS)` if used for measurements.
- **Presentation latency.** In a window at 120 Hz, draw → `presentedTime` is ~28 ms
  (3.4 refreshes) whatever the frames-in-flight limit or drawable count; it is the
  compositor. `displaySyncEnabled = false` cut it to ~10 ms but most frames were never
  presented. Details in [[synthesis/benchmarks]].
- **`preferredFramesPerSecond` defaults to 60.** Set it to the screen's
  `maximumFramesPerSecond` for ProMotion (and `CADisableMinimumFrameDurationOnPhone`
  on iPhone).
- **SwiftUI over an MTKView can throttle it.** A HUD that re-rendered (with material
  blur) on every zoom event cut the view to ~67 draws/s at 120 Hz; publishing HUD
  values at 10 Hz restored ~114 fps.

## Gaps

- No fetched Apple WWDC session on compute tuning (threadgroup sizing, 64-bit ints).
- Default math mode of runtime compilation not verified.
