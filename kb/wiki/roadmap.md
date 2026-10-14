---
title: Roadmap
type: meta
---

# Roadmap

Milestone status for almondbread. Derived from the ranked roadmap in
[[synthesis/deep-zoom-rendering-techniques]]; updated by the driver session
as milestones start and finish.

> **Priority (user, 2026-10-14):** feel and look first. Flying must feel
> and look stunning at shallow depth before any more depth or game work;
> if it does not, the game makes no sense. Order: M2.5 → FL (feel & look)
> and L0 → user review gate → L1 (real-time landscape prototype, if L0
> convinces) → user review gate → M3b, M4, M6, product and game features.

| Milestone | Scope | Roadmap steps | Status |
| --- | --- | --- | --- |
| M1 | float32 perturbation + rebasing, Double reference, DE, lighting, UI, macOS + iOS targets | 1 (partial), 4 (partial) | done 2026-10-14 |
| M2 | resumable chunked kernel, derivative interior detection, verification harness and benchmark set, interactive frame rate, WASD/QE keys, preview quality | 2, 3, 4, 8 (partial) | done 2026-10-14 (frame reprojection deferred to M5) |
| M3a | own arbitrary-precision library, reference orbit, period detection + Newton nucleus ([[decisions/arbitrary-precision]]) | 1, 7 (partial) | done 2026-10-14 (18 tests vs oracle; ≥ GMP speed to 2048 bits) |
| R1 | Rust workspace: port arbitrary precision + reference + selection, FFI/UniFFI + XCFramework build into the Swift package and Xcode project ([[decisions/rust-core]]) | — | done 2026-10-14 (bit-exact vs Swift, 1.05x to 4x orbit speed; [[platform/rust-swift-ffi]]) |
| R2a | Rust render crate: backend trait, chunked scheduler, Metal backend (objc2-metal), MSL kernels, core input/motion model, Rust CLI (render/verify/bench/regress); headless parity with Swift | — | done 2026-10-14 (fields bit-identical at all 11 locations, GPU ms geomean 0.998 of Swift, regress passes; [[platform/render-architecture]]) |
| R2b | switch the apps to the Rust renderer; Swift UI only; interaction parity; delete the Swift renderer and AlmondbreadMath | — | done 2026-10-14 (`Explorer` over UniFFI, one `frame` call per refresh, 2–7 µs FFI per frame; interaction parity or better, 9.4 ms drawable stall gone; [[platform/render-architecture]], [[synthesis/benchmarks]]) |
| L0 | landscape-mode prototype: still images, height-function variants ([[decisions/landscape-mode]]) | — | done 2026-10-14 (`prototypes/landscape/`; log height recommended; results in [[decisions/landscape-mode]]) |
| M2.5 | interaction polish: pointer/keyboard modes, crosshair zoom target, spring glide, edge panning ([[decisions/controls]] revision) | 8 (partial) | done 2026-10-14 (all motion policy in the core `Navigator`; Core Animation crosshair overlay; zoom-target drift 0 px over 2.5 s; glide 1 % in 265 ms, no overshoot; edge pan at the ramped key speed; [[synthesis/benchmarks]] M2.5) |
| FL | feel & look: inertia/easing, gamepad, frame reprojection while zooming, exposed-area-first refinement, progressive AA, designed colour presets, DE colouring and lighting by default, colour cycling; recorded flight videos for review | 8 | done 2026-10-14, ready for user review (eased velocities, flings, animated zoom steps and gamepad in the `Navigator`; history reprojection with temporal upsampling, per-pixel refinement fade, focus-first tiles, adaptive progressive AA; drawable pre-acquisition thread; OKLab looks with palette anti-aliasing; `almondbread fly` review videos; [[synthesis/benchmarks]] FL) |
| L1 | real-time landscape flight prototype (only if L0 convinces) | — | done 2026-10-14, ready for user review (`prototypes/landscape-rt/`: Metal geometry clipmap, camera-independent DE samples, resumable GPU DE tiles, eased flight in units of S with dives towards a coast-snapped aim, layered σ against melting; 118–120 fps at 120 Hz; [[decisions/landscape-mode]] L1, [[synthesis/benchmarks]] L1) |
| M3b | integrate: big viewport, background reference, rescaled float32 / floatexp deltas | 1, 6 | planned |
| M4 | BLA tree with ε = 2^-24, extended-range coefficients, skip-off diff | 5 | done 2026-10-14, but no gain at reachable depths (table with per-pixel `c` validity, built on demand/on a thread; kernel `kBLA` pipeline used only where the table's forecast says it pays: never at the f64 reference's depths, where lookups cost more than the 2–8-step blocks skip; 2.4× at 1e−20/px, 47× at 1e−24/px around a test reference; skip-off diff clean; plain pipeline bit-identical; floatexp coefficients left to M3b; [[techniques/bla]] As built, [[synthesis/benchmarks]] M4) |
| M5 | nucleus reference reuse, frame reprojection, progressive jittered AA, statistics-driven max iterations | 7, 8 | planned |
| M6 | Imagina-style LA + AT for power-2 Mandelbrot | 9 | planned |
| M7 | float-float repair mode | 10 | planned |
| M8 | more formulas via an opcode model | 11 | planned |
| M9 | extras: exponential-map export, orbit compression, iOS polish | 12 | planned |
