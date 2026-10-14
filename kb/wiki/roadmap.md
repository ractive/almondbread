---
title: Roadmap
type: meta
---

# Roadmap

Milestone status for almondbread. Derived from the ranked roadmap in
[[synthesis/deep-zoom-rendering-techniques]]; updated by the driver session
as milestones start and finish.

| Milestone | Scope | Roadmap steps | Status |
| --- | --- | --- | --- |
| M1 | float32 perturbation + rebasing, Double reference, DE, lighting, UI, macOS + iOS targets | 1 (partial), 4 (partial) | done 2026-10-14 |
| M2 | resumable chunked kernel, derivative interior detection, verification harness and benchmark set, interactive frame rate, WASD/QE keys, preview quality | 2, 3, 4, 8 (partial) | done 2026-10-14 (frame reprojection deferred to M5) |
| M3a | own arbitrary-precision library, reference orbit, period detection + Newton nucleus ([[decisions/arbitrary-precision]]) | 1, 7 (partial) | done 2026-10-14 (18 tests vs oracle; ≥ GMP speed to 2048 bits) |
| R1 | Rust workspace: port arbitrary precision + reference + selection, FFI/UniFFI + XCFramework build into the Swift package and Xcode project ([[decisions/rust-core]]) | — | done 2026-10-14 (bit-exact vs Swift, 1.05x to 4x orbit speed; [[platform/rust-swift-ffi]]) |
| R2 | Rust render crate: backend trait, chunked scheduler, Metal backend (objc2-metal), MSL kernels; Swift becomes UI only; parity with the Swift benchmarks | — | planned, after the wedge fix |
| M2.5 | interaction polish: pointer/keyboard modes, crosshair zoom target, spring glide, edge panning ([[decisions/controls]] revision) | 8 (partial) | planned, after the wedge bug fix |
| M3b | integrate: big viewport, background reference, rescaled float32 / floatexp deltas | 1, 6 | planned |
| M4 | BLA tree with ε = 2^-24, extended-range coefficients, skip-off diff | 5 | planned |
| M5 | nucleus reference reuse, frame reprojection, progressive jittered AA, statistics-driven max iterations | 7, 8 | planned |
| M6 | Imagina-style LA + AT for power-2 Mandelbrot | 9 | planned |
| M7 | float-float repair mode | 10 | planned |
| M8 | more formulas via an opcode model | 11 | planned |
| M9 | extras: exponential-map export, orbit compression, iOS polish | 12 | planned |
