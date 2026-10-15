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
| FL | feel & look: inertia/easing, gamepad, frame reprojection while zooming, exposed-area-first refinement, progressive AA, designed colour presets, DE colouring and lighting by default, colour cycling; recorded flight videos for review | 8 | done 2026-10-14, accepted by the user 2026-10-14 ("zooming and panning feels good"; fine-tuning later) (eased velocities, flings, animated zoom steps and gamepad in the `Navigator`; history reprojection with temporal upsampling, per-pixel refinement fade, focus-first tiles, adaptive progressive AA; drawable pre-acquisition thread; OKLab looks with palette anti-aliasing; `almondbread fly` review videos; [[synthesis/benchmarks]] FL) |
| L1 | real-time landscape flight prototype (only if L0 convinces) | — | done 2026-10-14, ready for user review (`prototypes/landscape-rt/`: Metal geometry clipmap, camera-independent DE samples, resumable GPU DE tiles, eased flight in units of S with dives towards a coast-snapped aim, layered σ against melting; 118–120 fps at 120 Hz; [[decisions/landscape-mode]] L1, [[synthesis/benchmarks]] L1) |
| L2 | landscape detail and robustness after the L1 review ("ok, not wow") | — | done 2026-10-15, for user review (no fallback slabs: parent-first scheduling and fallback through all levels, 0 no-data pixels vs 45 % starved / 99.9 % in L1 dives; screen-space-error priority; zoom no longer self-throttled (1e-3 → 1e-7 in 28 s vs 40 s); parent-sample reuse 1.15–1.27× DE; world-anchored detail noise; equipotential ridges on plains; depth limit 1e-12 kept, what lifting it takes documented; [[decisions/landscape-mode]] L2, [[synthesis/benchmarks]] L2) |
| M3b | integrate: big viewport, background reference, rescaled float32 / floatexp deltas | 1, 6 | done 2026-10-14 (BigComplex centre and extended-range scale end to end; big references with nucleus selection on a worker thread, reused while precision and offset allow; deep kernel with rescaled float32 deltas, floatexp full steps and exponent-carrying derivatives; extended-range BLA 1.5–26× at depth; deep CPU oracle checked against direct BigFloat; 51–59 fps zooming 1e-12 → 4e-39 and panning/zooming at 1e-300; stills to 1e-2900; [[synthesis/benchmarks]] M3b) |
| M4 | BLA tree with ε = 2^-24, extended-range coefficients, skip-off diff | 5 | done 2026-10-14, but no gain at reachable depths (table with per-pixel `c` validity, built on demand/on a thread; kernel `kBLA` pipeline used only where the table's forecast says it pays: never at the f64 reference's depths, where lookups cost more than the 2–8-step blocks skip; 2.4× at 1e−20/px, 47× at 1e−24/px around a test reference; skip-off diff clean; plain pipeline bit-identical; floatexp coefficients done in M3b; [[techniques/bla]] As built, [[synthesis/benchmarks]] M4) |
| AP | product: autopilot (Z; pointer steers like a joystick, adaptive speed from the renderer's feedback, interior/featureless safety, optional assist) and fly to coordinates (van Wijk & Nuij zoom-pan path, any depth, exact landing) ([[decisions/controls]] Autopilot, Fly to) | — | done 2026-10-14, for user review (all policy in the core `Navigator`: `autopilot.rs`, `flyto.rs`; GPU survey pass for safety and assist; steering and flights agree at 30/60/120 Hz; fly-to lands bit-exactly at 1e-34 and 1e-300 in `make regress` with 2 and 6 references; 89–90 fps autopilot and 86 fps fly-to in the 120 Hz interaction test; review videos `review/autopilot-seahorse.mp4`, `review/fly-to-seahorse.mp4`; [[synthesis/benchmarks]] Autopilot and fly to) |
| M5 | local series approximation (quadtree tile/pixel seeds; R3 measured 5.2× fewer iterations for the refined frame and 7.2× for settled AA at dense-5e-14, CPU f64 only), then interior-only tile fill; first a float32 GPU check ([[questions/local-sa-float32]], [[synthesis/dense-region-sampling]]). Reference reuse, reprojection, progressive AA and learned max iterations were already delivered by M3b/FL | 7, 8 | planned |
| M6 | Imagina-style LA + AT for power-2 Mandelbrot | 9 | planned |
| M7 | float-float repair mode | 10 | planned |
| M8 | more formulas via an opcode model | 11 | planned |
| M9 | extras: exponential-map export, orbit compression, iOS polish | 12 | planned |
| UX1 | control center (searchable, Settings window, persistence), interior colourings, calm dense regions, orbit overlay and mini-map with click-to-jump ([[decisions/controls]], [[techniques/coloring-methods]]) | — | done 2026-10-15 (GPU cycle analysis at any depth; overlays in Rust/Metal, zero cost when off; [[synthesis/benchmarks]] "Interior colourings, overlays") |
