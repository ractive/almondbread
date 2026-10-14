---
title: "Rust core with a Rust Metal backend; Swift for UI only"
type: decision
date: 2026-10-14
status: accepted
tags: [architecture, rust, metal, swift, portability]
---

# Rust core with a Rust Metal backend; Swift for UI only

Amends [[decisions/app-stack]] (Swift stays the UI language on Apple
platforms; everything else moves to Rust).

## Context

The user wants all non-UI code reusable, so that Windows and Linux UIs can
be added later. Milestones 1–3a were written in Swift (renderer, chunked
scheduler, verification harness, arbitrary-precision library
[[decisions/arbitrary-precision]]).

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
- Acceptance for the port: the benchmark set in [[synthesis/benchmarks]]
  (GPU ms per location, reference-orbit iterations per second per
  precision, interaction frame rate and latency) matches the Swift
  baseline within measurement noise on the same machine.
- No GMP/MPFR (licence, [[decisions/arbitrary-precision]]); the own
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
  same machine; allocating operators up to 14x (parse) ([[synthesis/benchmarks]],
  [[platform/mpfr-gmp-on-apple-silicon]]). The acceptance rule above is met for the
  maths; GPU and interaction parity is R2's.
- Build: `make rust` produces `build/rust/AlmondbreadCoreFFI.xcframework` and the Swift
  bindings; the Swift package has `AlmondbreadCoreFFI` (binary) and `AlmondbreadCore`
  (Swift) targets; `make build`, `make app` and `make ios-sim` link the core
  ([[platform/rust-swift-ffi]]).
- FFI: a scalar call costs 3.6 ns; returning bulk data as a UniFFI `Vec` is ~1700x
  slower than one copy into caller memory, so bulk data never crosses as a `Vec`.
- Open for R2: the render crate with the `objc2-metal` backend and the MSL kernels,
  the chunked scheduler port, Swift reduced to UI, then deletion of AlmondbreadMath
  and the Swift renderer.
