---
title: "Rust core to Swift: UniFFI and the XCFramework pipeline"
type: platform
status: draft
sources: ["sources/uniffi-user-guide"]
tags: [rust, swift, ffi, uniffi, xcframework, build, ios, macos]
---

# Rust core to Swift: UniFFI and the XCFramework pipeline

How the Rust workspace of [Rust core with a Rust Metal backend; Swift for UI only](../decisions/rust-core.md) reaches the Swift UI on macOS and
iOS, what a call across the boundary costs, and the rules that follow. All numbers
are our own measurements (measured 2026-10-14, M1 Pro, release builds, Xcode 26.6,
rustc 1.99, UniFFI 0.32).

## Pipeline

```text
crates/almondbread-math   pure Rust maths (no platform code, forbid(unsafe))
crates/almondbread-core   platform-neutral core (re-exports math; grows in R2)
crates/almondbread-render backend trait, scheduler, Host, Metal backend + CAMetalLayer surface
crates/almondbread-ffi    #[uniffi::export] surface; crate-type lib + staticlib + cdylib
crates/uniffi-bindgen     the bindings generator as a workspace binary
        |
scripts/build-rust-xcframework.sh   (make rust)
  1. cargo build --release -p almondbread-ffi --target
       aarch64-apple-darwin, aarch64-apple-ios, aarch64-apple-ios-sim
     with MACOSX_DEPLOYMENT_TARGET=14.0, IPHONEOS_DEPLOYMENT_TARGET=17.0
  2. uniffi-bindgen generate --library <host libalmondbread_ffi.dylib> --language swift
       -> almondbread_ffi.swift, almondbread_ffiFFI.h, almondbread_ffiFFI.modulemap
  3. headers dir = the .h + the modulemap renamed module.modulemap
  4. xcodebuild -create-xcframework (-library <.a> -headers <dir>) x 3
       -> build/rust/AlmondbreadCoreFFI.xcframework  (swapped in atomically,
          rebuilt only when a library or the header hash changes)
  5. almondbread_ffi.swift -> Sources/AlmondbreadCore/Generated/ (only if changed)
        |
Package.swift
  .binaryTarget AlmondbreadCoreFFI  (path build/rust/AlmondbreadCoreFFI.xcframework)
  .target AlmondbreadCore           (generated bindings + thin helpers; links Metal,
                                     QuartzCore, CoreGraphics for the Rust renderer)
  .target AlmondbreadKit            (UI only; depends on AlmondbreadCore)
  .executableTarget almondbread-core-check
project.yml: both app targets depend on the AlmondbreadCore product.
```

- UniFFI runs in proc-macro mode: `uniffi::setup_scaffolding!()` plus
  `#[uniffi::export]` functions and `uniffi::Record` / `Object` / `Enum` / `Error`
  types; no UDL file. Library mode reads the interface from metadata embedded in the
  compiled library (src: [The UniFFI user guide (Mozilla)](../sources/uniffi-user-guide.md)), so the host dylib is enough
  to generate bindings for all slices.
- The binary target's name must equal the XCFramework's file name, so the
  framework is `AlmondbreadCoreFFI` and the Swift wrapper module `AlmondbreadCore`.
  The C module Swift imports is `almondbread_ffiFFI` (from the generated modulemap).
- `make build`, `make run`, `make app` and `make ios-sim` depend on `make rust`
  (a no-op rebuild costs ~3 s); `make rust-check` runs `cargo test`, clippy with
  `-D warnings` and `cargo fmt --check`; `make core-check` runs the bridge check.
- Outputs are gitignored: `/target/`, `/build/`,
  `Sources/AlmondbreadCore/Generated/`.
- The release static library is ~30 MB (all of std plus line tables); the linker
  dead-strips it. The macOS app binary was 3.8 MB with the core linked.
- Release profile: `lto = "fat"`, `codegen-units = 1`. `panic` stays `unwind`:
  UniFFI catches panics at the boundary; with `abort` any failed assertion would
  kill the app. Throwing exports surface a panic as a Swift error, non-throwing
  ones still crash, so exports validate their arguments and return `CoreError`.
- `cargo check` of math, core, render and ffi passes for aarch64-apple-darwin,
  aarch64-apple-ios, aarch64-apple-ios-sim, x86_64-unknown-linux-gnu,
  x86_64-unknown-linux-musl (cdylib dropped there, static only) and
  x86_64-pc-windows-msvc.

## The R1 interface

| Export | Purpose |
| --- | --- |
| `core_version()` | "almondbread-core 0.1.0" |
| `ping(x)` | x + 1; measures the bare call cost |
| `normalize_coordinate(re, im, bits, digits?)` | correctly rounded parse + format |
| `recommended_precision_bits(spacing_log2, size)` | precision rule ([Reference orbit: precision, cost, storage, compression](../techniques/reference-orbit-precision.md)) |
| `select_reference(re, im, radius, max_iter, bits, method, cancel?)` | period + Newton nucleus or centre ([Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md)) |
| `compute_reference_orbit(re, im, bits, max_iter, bailout, cancel?)` | returns a `ReferenceOrbit` object owned by Rust |
| `ReferenceOrbit.copy_extended_to(address, capacity)` | one copy of the floatexp points into caller memory |
| `ReferenceOrbit.copy_points_to`, `points_interleaved()`, `final_z(digits?)` | float32 points, the full-precision last point |
| `CancelToken` (`cancel`, `is_cancelled`, `progress`) | an `AtomicBool` and a progress counter shared with Swift |
| `float_exp_complex_layout()` | size and alignment as Rust lays the GPU struct out |

`FloatExpComplex` is `#[repr(C, align(8))] { mantissa: [f32; 2], exponent: i32 }`:
16-byte stride, 8-byte alignment, the MSL `struct { float2 m; int e; }`
([Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)). A compile-time assertion pins it in Rust and
`almondbread-core-check` checked it against Swift's `MemoryLayout` while the Swift maths
existed (R1); since R2b it checks the exported size and alignment, [16, 8].

## Measured call costs

`almondbread-core-check` (best of 5 rounds):

| Call | Cost |
| --- | ---: |
| `ping(u64) -> u64` | 3.6 ns |
| `core_version() -> String` (string lifted into Swift) | 66 ns |
| `compute_reference_orbit`, 1 iteration at 64 bits (two decimal parses, object handle) | 1.07 µs (Swift-only: 2.23 µs) |
| `copy_extended_to`, 1 M points (16 MB) | 0.34 ms (47 GB/s) |
| `points_interleaved() -> Vec<f32>`, 1 M points (8 MB) | 572 ms |

A scalar call costs a few nanoseconds: a frame's handful of control calls is free.
Bulk data is the trap: UniFFI serialises a `Vec<f32>` through a `RustBuffer`
(src: [The UniFFI user guide (Mozilla)](../sources/uniffi-user-guide.md)) and the generated Swift reads it back element by
element, about 1700 times slower than a memcpy.

## Rules

- Nothing per pixel or per iteration crosses the boundary ([Rust core with a Rust Metal backend; Swift for UI only](../decisions/rust-core.md)).
  A frame makes a handful of calls.
- Bulk data never travels as a UniFFI `Vec`: Rust keeps it in an object and copies it
  once into caller memory (`copy_extended_to(address, capacity)`; in R2 the Metal
  buffers are owned by Rust and nothing is copied at all). The one `unsafe` block in
  the FFI crate is this copy; the address and capacity contract is on the export. Since
  2026-10-20 (review U1) the exports return `Result`: a misaligned address or a range that
  wraps the address space is `CoreError.InvalidArgument` instead of a panic (a Swift trap),
  and the Swift helpers `ReferenceOrbit.copyExtended(into:)` / `copyPoints(into:)` throw
  and are the only callers (they derive address and capacity from one buffer).
- Long computations (orbits, reference selection) block: call them off the main
  thread, cancel with a `CancelToken` from any thread (polled every 4096
  iterations), read `progress()` for the UI.
- Coordinates cross as decimal strings, exact at their precision: no binary
  BigFloat format leaks into Swift.
- Bit-for-bit parity with the Swift maths was checked through the real bindings in R1
  (`make core-check` against AlmondbreadMath). Since R2b the Swift maths is gone, the
  maths is tested in Rust (oracle fixtures, property tests) and `make core-check` is a
  slim bridge check: coordinate round trips, an orbit copied into Swift memory,
  cancellation, the palette list, the `Explorer` on a windowless `CAMetalLayer`
  (settings and viewport round trips, input, frames until settled, an offscreen PNG),
  and the call costs below.

## The R2b interface: `Explorer`

The app's renderer is one UniFFI object per window (`crates/almondbread-ffi/src/explorer.rs`,
Apple targets; it wraps the platform-neutral `Host` of [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](render-architecture.md)).

| Export | Purpose |
| --- | --- |
| `Explorer(layer: u64, scale)` | the `CAMetalLayer`'s address, passed once (`Unmanaged.passUnretained(layer).toOpaque()`); Rust retains and configures it, creates device, queue, pipelines |
| `resize(width, height, scale)` | view size in points, backing scale; sets `contentsScale` and the drawable size; the first size fits the home view |
| `frame(timestamp) -> UInt8` | one per display refresh (the display link's timestamp); bit 1 clear: pause the link; bit 2: keyboard mode (M2.5, [Controls: keyboard, pointer and touch](../decisions/controls.md)). Bits, not a record: a record is serialised through a heap buffer per call (11–18 µs per frame in the window vs 2–3 µs, [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M2.5) |
| `crosshairX()`, `crosshairY()` | the keyboard crosshair after the last `frame` (points from the centre, y up); called only in keyboard mode |
| `input(InputEvent) -> InputResult {handled, redraw, mode}` | decoded events (`KeyDown/KeyUp(NavKey)`, `Shift`, `FocusLost`, `Insets`, `Pointer`, `PointerExited`, `Pan`, `WheelZoom`, `Magnify`, `Pinch`, `DoubleTap`, `ZoomBy`); `handled`: swallow it, `redraw`: resume the link, `mode`: pointer or keyboard after the event. Non-finite coordinates are rejected |
| `setEdgePan(EdgePan)`, `edgePan()` | edge-panning setting (off / while zooming / always) |
| `settings()`, `setSettings(ExplorerSettings)` | the inspector's settings as one record (validated; the learned iteration floor is kept) |
| `stats() -> ExplorerStats` | HUD values: shown view, limit, GPU time, chunks, size, preview/refining, reference, zoom speed, held keys, settled, failed frames |
| `viewport()`, `setViewport(re, im, scale)`, `resetViewport()`, `coordinatesText()` | centre as decimal strings (M3b-ready) |
| `renderPng(path, w, h)` | offscreen render of the view (separate renderer, extension passes as in the view) |
| `beginMeasurement()`, `endMeasurement() -> InteractionReport`, `finalFrame()`, `displayedFrameMismatch()`, `finishFrames()` | interaction test and self-test |
| `palettes()` | id, name and stops for the inspector |

The Swift side (`AlmondbreadKit`) keeps decoding only: key codes to `NavKey`, gesture
recognisers, positions to points from the view centre (y up). `ExplorerModel` mirrors the
settings and forwards each change with `setSettings`, and reads `stats()` at ~10 Hz while
the display link runs (assigning only on change, so idle views do not re-render).
`Settings` was renamed `ExplorerSettings` because SwiftUI has a `Settings` scene type.

Threading: Swift calls from the main thread; the state sits behind a mutex. The objc2
objects are not `Send`, so the FFI crate asserts it for the mutex-guarded state (one
`unsafe impl`, plus the layer-pointer `from_raw`).

### Measured costs (R2b)

`make core-check` and the interaction test (measured 2026-10-14, M1 Pro, release):

| Call | Cost |
| --- | ---: |
| `input(.pointer)` (enum and record through a `RustBuffer`, mutex, clock) | 0.9 µs |
| `stats()` (record with two formatted coordinate strings) | 6.2 µs |
| `frame()` per call: Swift-side time minus the time Rust measures inside it, in the window (interaction test, 8 scenarios x 9 runs) | 2–7 µs |
| the same on a windowless layer (core-check; noisy, one outlier of 19 µs under load) | 0.2–1 µs |

At 60–120 Hz the frame call costs < 0.1 % of a refresh; events are a few per refresh;
`stats()` runs at 10 Hz. The window figure includes Rust's autorelease pool and the
presentation-log drain; nothing per pixel crosses.

## Open

- Callbacks from Rust into Swift (frame finished, progress) via UniFFI callback
  interfaces or foreign traits, when something needs them (R2b got by with polling).
- An async export (UniFFI supports `async fn`) instead of blocking calls on a Swift
  background queue, once reference orbits move off the render thread (M3b).

# Citations

- [The UniFFI user guide (Mozilla)](../sources/uniffi-user-guide.md)
