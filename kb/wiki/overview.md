---
title: Overview
type: meta
---

# Overview

This wiki collects what almondbread needs to know to render the Mandelbrot
set and related escape-time fractals very fast and very deep on macOS with
Apple Silicon: the maths, per-pixel and image-level tricks, colouring, the
Apple GPU and Metal, arbitrary precision on the CPU, and what existing
renderers do. The app is a Rust core and renderer (Metal compute kernels,
float32 only, since Apple GPUs have no fp64; arbitrary-precision reference
orbits on the CPU) with a Swift + SwiftUI/AppKit UI on Apple platforms
([App stack: Swift, SwiftUI/AppKit and Metal](decisions/app-stack.md), [Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md), [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md)).

## The settled pipeline (as of 2026)

1. **One reference orbit** in arbitrary precision on the CPU, ideally at a
   minibrot nucleus found by period detection and Newton
   ([Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md), [Reference orbit: precision, cost, storage, compression](techniques/reference-orbit-precision.md),
   [Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md)).
2. **Per-pixel perturbed deltas** in hardware floats on the GPU
   ([Perturbation](techniques/perturbation.md)), with **rebasing** so glitches cannot
   happen and one reference serves the image ([Rebasing (Zhuoran, 2021)](techniques/rebasing.md)).
   Pauldelbrot glitch detection and multi-reference correction are legacy
   ([Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](techniques/glitch-detection.md)).
3. **Iteration skipping** with a precomputed table: mathr's binary-tree
   [Bivariate linear approximation (BLA)](techniques/bla.md) (general, works for Burning Ship, Mandelbar, hybrids)
   or Imagina's period-aligned [Linear approximation with period-aligned stages and AT (Imagina)](techniques/linear-approximation-imagina.md)
   (Mandelbrot only; measured in M6: 1.9–2.4× over BLA at deep needle minibrots, no gain on
   centre references or in float32 views, so almondbread uses LA only on deep nucleus references
   and BLA elsewhere; the AT shortcut gave no gain on the GPU and its code was deleted
   2026-10-17). A preview waits for a new reference's table. Series
   approximation and nanoMB are legacy ([Series approximation (legacy)](techniques/series-approximation.md)).
4. **Range handling** for float32: plain deltas to about 1e-25, then
   rescaled float32 with floatexp scale and floatexp skip-table entries
   ([Extended range: floatexp, rescaled iterations, depth thresholds](techniques/extended-range-floats.md)); [Double-float (float-float) arithmetic](techniques/double-float.md) only
   as a precision repair.
5. **Cheap per-pixel extras**: derivative interior detection, completed by the closed-form
   cardioid / period-2 test and the attracting-cycle probe (Newton proof of an interior cycle),
   exterior distance estimation, split smooth counts, a separate colouring pass
   ([Interior detection](techniques/interior-detection.md), [Distance estimation](techniques/distance-estimation.md),
   [Smooth (continuous) iteration count](techniques/smooth-coloring.md), [Colouring methods](techniques/coloring-methods.md)). What stays unresolved at the
   limit is drawn as fog ([Fog over unresolved pixels](techniques/unresolved-fog.md)).
6. **Image-level work**: progressive jittered anti-aliasing, frame reuse,
   coarse-to-fine refinement, auto iterations, exponential-map movies
   ([Anti-aliasing and supersampling](techniques/anti-aliasing.md), [Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md),
   [Automatic maximum-iteration control](techniques/auto-iterations.md), [Exponential map (log-polar) rendering](techniques/exponential-map.md)). Auto iterations end with
   "compute further": background passes on the unresolved texels up to an Upper limit.
   Solid guessing is weak on GPUs ([Guessing, border tracing and rectangle subdivision](techniques/guessing-and-subdivision.md)).

## What is specific to almondbread

- float32-only GPU: ε = 2^-24 in skip radii, norms that do not underflow,
  per-quantity exponents in skip tables, and a verification harness,
  because float32 errors are silent ([How large is float32 perturbation error over long orbits?](questions/float32-error-long-orbits.md)). M7
  measured them: smaller than moving the pixel grid by 0.001–0.2 px at every view tested
  to 1e-1003, nothing visible, so float-float stays a verification tool.
- macOS GPU watchdog (about 5 s per command buffer, community report):
  resumable chunked kernels with compaction and indirect dispatch
  ([GPU watchdog and chunked, resumable kernels](platform/gpu-watchdog-and-chunking.md), [Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md),
  [Apple GPU architecture for compute](platform/apple-gpu-architecture.md)).
- iPad/iPhone port (built since 2026-10-18; touch controls in [Controls: keyboard, pointer and touch](decisions/controls.md) "Touch",
  [iOS and iPadOS on a device: first run, signing, memory, thermal, background](platform/ios-device.md), [iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md)): same kernels, tighter memory and
  thermal limits ([iOS and iPadOS port considerations](platform/ios-ipad-considerations.md)).

## Formulas and renderers

Mandelbrot first, then Multibrot, Mandelbar, Burning Ship, hybrids, Julia
([Mandelbrot set (power 2)](formulas/mandelbrot.md), [Multibrot (integer power p)](formulas/multibrot.md), [Mandelbar (Tricorn)](formulas/mandelbar.md),
[Burning Ship and abs variations](formulas/burning-ship.md), [Hybrid formulas](formulas/hybrids.md), [Julia sets](formulas/julia.md),
[Other formulas: feasibility of deep zoom](formulas/others-feasibility.md)). The renderer landscape is in
`implementations/`: Fraktaler 3, Imagina, FractalShark and Fractal Zoomer
carry the modern methods; KF2+ is frozen; no mature macOS-native deep zoomer
exists, though small Swift/Metal projects such as
[swift-mandelbrot (drquicksilver)](implementations/swift-mandelbrot.md) cover the basics.

## Status and controls

Milestones 2 to 7 are built (chunked resumable iterate pass, interior detection, BLA, local
series approximation, Imagina LA, float-float as a verification tool); measurements live in
[almondbread benchmarks and verification record](synthesis/benchmarks.md). [Controls: keyboard, pointer and touch](decisions/controls.md) holds the interaction design: WASD/arrows pan,
Space zoom, pointer and touch gestures, and since 2026-10-17 the driving levels (manual, cruise,
autopilot, journey; Q / E are pedals, reverse cruise zooms home, a cockpit with clickable icons
shows the level), the autopilot's target lock and rescue (survey steering by default, lane assist in
cruise), journeys with stops, the compass, full screen, a render-progress line, help popovers,
the cosmic size scale (your window stays, the set grows, then imaginary universes nest),
guided tours (the π meter only through its tour since 2026-10-18) with a first-launch
walkthrough, and on iPhone / iPad a throttle lever, round buttons for the Mac toolbar's items
and "rest to steer, swipe to move" while something flies.
Places, including endless dives on Misiurewicz points, are in [Places: a catalogue of famous views to visit](synthesis/places.md).
Smoothness while moving is measured, not assumed: gesture-resolution probes follow the GPU
clocks ([Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md)), hang prevention guards every
loop that depends on computed values ([Hang prevention (loop guards, extreme-value tests, main-thread watchdog)](techniques/hang-prevention.md)).

Decided choices are the only code: the behaviour env switches (SA, fill, BLA, LA, fog, compute
further, probes, warm-up, ...) and the opt-in motion sample cache were deleted 2026-10-17
([Sample reuse while zooming and panning (sample cache in fractal coordinates)](techniques/sample-reuse.md) keeps the research). The app is MIT-licensed open source
([Open-source release: licence, distribution, CI](decisions/open-source-release.md)).

## Roadmap and open questions

The ranked implementation roadmap (rebasing kernel → chunked dispatch →
verification harness → interior/DE/colouring → BLA → rescaling → nucleus
reference → interaction → Imagina LA/AT → float-float → more formulas) is in
[Deep zoom Mandelbrot rendering techniques](synthesis/deep-zoom-rendering-techniques.md). Open questions and
contradictions between sources are in `questions/` (see [Index](index.md)).

State (2026-10-19): the research run is ingested and the app is built through M7 plus the 2026-10-16 to 19 features (touch, tours, walkthrough); open items are in [Roadmap](roadmap.md) and `questions/`.
