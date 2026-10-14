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
