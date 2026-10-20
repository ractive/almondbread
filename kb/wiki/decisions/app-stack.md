---
title: "App stack: Swift, SwiftUI/AppKit and Metal"
type: decision
date: 2026-10-14
status: accepted
tags: [architecture, metal, swift]
---

# App stack: Swift, SwiftUI/AppKit and Metal

## Context

The explorer must zoom very deep, very fast, tuned for macOS on Apple
Silicon. Options weighed: a web app with WASM (and WebGPU), Rust with
Tauri, Rust with wgpu and egui, C++ with metal-cpp, Objective-C, Swift.

## Decision

Swift with SwiftUI/AppKit for the app, Metal compute kernels for per-pixel
work, and arbitrary-precision reference orbits on the CPU (MPFR/GMP via C
interop, or a Rust core if portability becomes a goal). Built as a SwiftPM
package from the terminal; shaders compiled at runtime from source, so the
Xcode app is needed only for GPU profiling.

## Reasons

- Apple GPUs have no fp64, so deep zoom needs perturbation with float32
  deltas; the speed comes from the algorithm, and Metal gives full control
  of the GPU (SIMD groups, threadgroup tuning, MetalFX) and the profiler.
- Unified memory: `storageModeShared` buffers let the CPU write the
  reference orbit and the GPU read it with no copy.
- Tauri puts a web view between the renderer and the screen; WASM is
  slower at arbitrary precision and also lacks fp64 on WebGPU.
- Rust + wgpu stays the fallback if cross-platform matters later.

## Consequences

- No offline `metal` compiler without Xcode: shader errors surface at
  runtime; `makeLibrary(source:)` costs a little at startup.
- A small script builds the `.app` bundle.

# Citations

- [No cited source: our own design decision; its basis is the Context and Reasons above](#context)
