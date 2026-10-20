---
title: "Rust core with a Rust Metal backend; Swift for UI only"
type: decision
date: 2026-10-14
status: accepted
tags: [architecture, rust, metal, swift, portability]
---

# Rust core with a Rust Metal backend; Swift for UI only

Amends [App stack: Swift, SwiftUI/AppKit and Metal](app-stack.md) (Swift stays the UI language on Apple
platforms; everything else moves to Rust).

## Context

The user wants all non-UI code reusable, so that Windows and Linux UIs can
be added later. Milestones 1–3a were written in Swift (renderer, chunked
scheduler, verification harness, arbitrary-precision library
[Arbitrary precision: own Swift/C implementation, not GMP/MPFR](arbitrary-precision.md)).

## Decision

- **Rust workspace** in `crates/`: maths (arbitrary precision, floatexp,
  reference orbit, reference selection), core (viewport, settings,
  formulas, BLA/LA tables, CPU oracle, benchmark locations), render
  (GPU-backend trait, chunked scheduler, preview/refine, timing) with a
  **Metal backend in Rust** (`objc2-metal`), the MSL kernels living in the
  render crate, and an FFI crate.
- **Swift**: SwiftUI views, input handling (pointer, keys, gestures),
  hosting the `CAMetalLayer`. It talks to Rust through UniFFI for
  control-plane calls; the render loop and Metal objects live in Rust.
  High-frequency calls that prove costly get a raw C ABI.
- **Later platforms**: their own UI plus a new backend (Vulkan or wgpu),
  kernels ported or generated from the formula opcode model.

## Reasons

- Performance is on par: the GPU work is the same MSL on the same driver;
  Metal host calls go through `objc_msgSend` from either language; the
  CPU-side bignum code compiles through LLVM in both and Rust avoids ARC
  and copy-on-write overhead.
- Cheapest moment: before arbitrary precision is wired into the renderer
  and before BLA.

## Rules

- Nothing per pixel or per iteration crosses the FFI boundary; a frame
  makes a handful of FFI calls at most.
- Acceptance for the port: the benchmark set in [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
  (GPU ms per location, reference-orbit iterations per second per
  precision, interaction frame rate and latency) matches the Swift
  baseline within measurement noise on the same machine.
- No GMP/MPFR (licence, [Arbitrary precision: own Swift/C implementation, not GMP/MPFR](arbitrary-precision.md)); the own
  arithmetic is ported, with the existing oracle fixtures.

## Consequences

- Build: cargo + UniFFI + an XCFramework (macOS arm64, iOS arm64, iOS
  simulator arm64) consumed by the Swift package and the Xcode project.
- The Swift renderer and AlmondbreadMath remain as the reference until the
  Rust port passes parity, then are deleted.

## Outcome (R1, 2026-10-14)

- Workspace at the repository root: `crates/almondbread-math` (port of
  AlmondbreadMath: BigFloat, BigComplex, FloatExp/DoubleExp, `FloatExpComplex` with the
  MSL layout, decimal parse/format, allocation-free `ReferenceOrbitComputer` with
  cancellation and progress, precision rule, ball/first-order/box/atom-domain period
  detection, Newton nuclei, minibrot size, reference selection; `forbid(unsafe_code)`),
  `almondbread-core` (re-exports math, reference pipeline), `almondbread-render`
  (skeleton for R2), `almondbread-ffi` (UniFFI), `uniffi-bindgen`. math and core pass
  `cargo check` for macOS, iOS, iOS simulator, Linux (gnu, musl) and Windows (msvc).
- Correctness: the Swift oracle tests ported bit-exact on the same fixtures, plus
  property tests against exact rationals; the Rust core matches AlmondbreadMath bit
  for bit through the Swift bindings (`make core-check`).
- Speed: reference orbit 1.05x (2048 bits) to 4x (64 bits) the Swift build on the
  same machine; allocating operators up to 14x (parse) ([almondbread benchmarks and verification record](../synthesis/benchmarks.md),
  [Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md)). The acceptance rule above is met for the
  maths; GPU and interaction parity is R2's.
- Build: `make rust` produces `build/rust/AlmondbreadCoreFFI.xcframework` and the Swift
  bindings; the Swift package has `AlmondbreadCoreFFI` (binary) and `AlmondbreadCore`
  (Swift) targets; `make build`, `make app` and `make ios-sim` link the core
  ([Rust core to Swift: UniFFI and the XCFramework pipeline](../platform/rust-swift-ffi.md)).
- FFI: a scalar call costs 3.6 ns; returning bulk data as a UniFFI `Vec` is ~1700x
  slower than one copy into caller memory, so bulk data never crosses as a `Vec`.
- Open for R2: the render crate with the `objc2-metal` backend and the MSL kernels,
  the chunked scheduler port, Swift reduced to UI, then deletion of AlmondbreadMath
  and the Swift renderer.

## Outcome (R2a, 2026-10-14)

- `almondbread-render`: the `GpuBackend` trait, the chunked progressive scheduler written
  once against it (a decision-for-decision port of the Swift `Renderer`), the MSL kernels
  (`include_str!`, same compile options), the Metal backend on objc2-metal and a
  `CAMetalLayer` surface; no backend on Linux/Windows yet (the crates still `cargo check`
  there). `almondbread-core` gained viewport, settings, palettes, the navigation model,
  CPU oracle and verify metrics; `almondbread-cli` has render/verify/bench/regress
  ([Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md)).
- Acceptance met headlessly: iteration fields bit-identical to Swift at every benchmark
  location and the regress views (extension passes included), GPU ms per location within
  noise (geomean 0.998), regress sessions pass the same guards ([almondbread benchmarks and verification record](../synthesis/benchmarks.md)).
- Open for R2b: export the engine through UniFFI (layer pointer, per-frame call, input
  events), switch both apps, measure interaction parity (including the drawable-pool
  predictor that replaces the blocking `nextDrawable` wait), delete the Swift renderer and
  AlmondbreadMath.

## Outcome (R2b, 2026-10-14)

- Both apps render through Rust. `almondbread-ffi` exports an `Explorer` per window
  (layer address once, `resize`, one `frame(timestamp)` per display refresh, `input`
  with a decoded `InputEvent`, settings as one record, `stats()` at ~10 Hz, viewport,
  coordinates text, palettes, offscreen PNG, interaction-test measurements); it wraps
  the platform-neutral `Host<B>` in `almondbread-render`, which a Windows or Linux UI
  reuses with its own backend and surface ([Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md),
  [Rust core to Swift: UniFFI and the XCFramework pipeline](../platform/rust-swift-ffi.md)).
- Swift is UI only: SwiftUI views, `ExplorerModel` (settings mirror), the platform views
  (CAMetalLayer, display link, event decoding), self-test and interaction test. All
  motion policy is the core `Navigator`.
- Deleted: the Swift renderer and its helpers (Renderer, GPUTypes, Offscreen,
  CPUReference, BenchmarkLocations, KeyControls, Palettes, ReferenceOrbit,
  FractalModel), the Swift kernel copy (`Mandelbrot.metalsrc`), AlmondbreadMath,
  `almondbread-math-check`, the Swift oracle tests (fixtures were already in
  `crates/almondbread-math`; the generator moved there) and the Swift CLI commands
  (`--render/--verify/--bench`, now `almondbread render|verify|bench|regress`).
  `make core-check` is a slim bridge check.
- Acceptance met: interaction frame rate, interval and latency at parity or better in all
  eight scenarios, the 9.4 ms drawable stall gone, shown frames identical to fresh
  renders; final refined frames 10–25 ms later in some scenarios for the same GPU work
  ([almondbread benchmarks and verification record](../synthesis/benchmarks.md), R2b). FFI per frame 2–7 µs.
- The R2a pool predictor needed two corrections (count `acquired − presented`; gate only
  gesture frames before encoding), see [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md).

# Citations

- [No cited source: our own design decision; its basis is the Context and Reasons above](#context)
