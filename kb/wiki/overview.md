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
([[decisions/app-stack]], [[decisions/rust-core]], [[platform/render-architecture]]).

## The settled pipeline (as of 2026)

1. **One reference orbit** in arbitrary precision on the CPU, ideally at a
   minibrot nucleus found by period detection and Newton
   ([[techniques/reference-selection]], [[techniques/reference-orbit-precision]],
   [[platform/mpfr-gmp-on-apple-silicon]]).
2. **Per-pixel perturbed deltas** in hardware floats on the GPU
   ([[techniques/perturbation]]), with **rebasing** so glitches cannot
   happen and one reference serves the image ([[techniques/rebasing]]).
   Pauldelbrot glitch detection and multi-reference correction are legacy
   ([[techniques/glitch-detection]]).
3. **Iteration skipping** with a precomputed table: mathr's binary-tree
   [[techniques/bla]] (general, works for Burning Ship, Mandelbar, hybrids)
   or Imagina's period-aligned [[techniques/linear-approximation-imagina]]
   (Mandelbrot only; measured in M6: 1.9–2.4× over BLA at deep needle minibrots, no gain on
   centre references or in float32 views, so almondbread uses LA only on deep nucleus references
   and BLA elsewhere; the AT shortcut gave no gain on the GPU and its code was deleted
   2026-10-17). A preview waits for a new reference's table. Series
   approximation and nanoMB are legacy ([[techniques/series-approximation]]).
4. **Range handling** for float32: plain deltas to about 1e-25, then
   rescaled float32 with floatexp scale and floatexp skip-table entries
   ([[techniques/extended-range-floats]]); [[techniques/double-float]] only
   as a precision repair.
5. **Cheap per-pixel extras**: derivative interior detection, completed by the closed-form
   cardioid / period-2 test and the attracting-cycle probe (Newton proof of an interior cycle),
   exterior distance estimation, split smooth counts, a separate colouring pass
   ([[techniques/interior-detection]], [[techniques/distance-estimation]],
   [[techniques/smooth-coloring]], [[techniques/coloring-methods]]). What stays unresolved at the
   limit is drawn as fog ([[techniques/unresolved-fog]]).
6. **Image-level work**: progressive jittered anti-aliasing, frame reuse,
   coarse-to-fine refinement, auto iterations, exponential-map movies
   ([[techniques/anti-aliasing]], [[techniques/frame-reuse-and-progressive]],
   [[techniques/auto-iterations]], [[techniques/exponential-map]]). Auto iterations end with
   "compute further": background passes on the unresolved texels up to an Upper limit.
   Solid guessing is weak on GPUs ([[techniques/guessing-and-subdivision]]).

## What is specific to almondbread

- float32-only GPU: ε = 2^-24 in skip radii, norms that do not underflow,
  per-quantity exponents in skip tables, and a verification harness,
  because float32 errors are silent ([[questions/float32-error-long-orbits]]). M7
  measured them: smaller than moving the pixel grid by 0.001–0.2 px at every view tested
  to 1e-1003, nothing visible, so float-float stays a verification tool.
- macOS GPU watchdog (about 5 s per command buffer, community report):
  resumable chunked kernels with compaction and indirect dispatch
  ([[platform/gpu-watchdog-and-chunking]], [[platform/metal-compute]],
  [[platform/apple-gpu-architecture]]).
- iPad/iPhone port later: same kernels, tighter memory and thermal limits
  ([[platform/ios-ipad-considerations]]).

## Formulas and renderers

Mandelbrot first, then Multibrot, Mandelbar, Burning Ship, hybrids, Julia
([[formulas/mandelbrot]], [[formulas/multibrot]], [[formulas/mandelbar]],
[[formulas/burning-ship]], [[formulas/hybrids]], [[formulas/julia]],
[[formulas/others-feasibility]]). The renderer landscape is in
`implementations/`: Fraktaler 3, Imagina, FractalShark and Fractal Zoomer
carry the modern methods; KF2+ is frozen; no mature macOS-native deep zoomer
exists, though small Swift/Metal projects such as
[[implementations/swift-mandelbrot]] cover the basics.

## Status and controls

Milestones 2 to 7 are built (chunked resumable iterate pass, interior detection, BLA, local
series approximation, Imagina LA, float-float as a verification tool); measurements live in
[[synthesis/benchmarks]]. [[decisions/controls]] holds the interaction design: WASD/arrows pan,
Space zoom, pointer and touch gestures, and since 2026-10-17 the driving levels (manual, cruise,
autopilot, journey; Q / E are pedals, reverse cruise zooms home, a cockpit with clickable icons
shows the level), the autopilot's target lock and rescue (survey steering opt-in, lane assist in
cruise), journeys with stops, the compass, full screen, a render-progress line, help popovers,
and the cosmic size scale (your window stays, the set grows, then imaginary universes nest).
Places, including endless dives on Misiurewicz points, are in [[synthesis/places]].
Smoothness while moving is measured, not assumed: gesture-resolution probes follow the GPU
clocks ([[platform/metal-compute]], [[synthesis/benchmarks]]), hang prevention guards every
loop that depends on computed values ([[techniques/hang-prevention]]).

Decided choices are the only code: the behaviour env switches (SA, fill, BLA, LA, fog, compute
further, probes, warm-up, ...) and the opt-in motion sample cache were deleted 2026-10-17
([[techniques/sample-reuse]] keeps the research). The app is MIT-licensed open source
([[decisions/open-source-release]]).

## Roadmap and open questions

The ranked implementation roadmap (rebasing kernel → chunked dispatch →
verification harness → interior/DE/colouring → BLA → rescaling → nucleus
reference → interaction → Imagina LA/AT → float-float → more formulas) is in
[[synthesis/deep-zoom-rendering-techniques]]. Open questions and
contradictions between sources are in `questions/` (see [[index]]).

State (2026-10-17): the research run is ingested and the app is built through M7 plus the 2026-10-16/17 features; open items are in [[roadmap]] and `questions/`.
