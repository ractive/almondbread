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
   with the AT minibrot shortcut (fastest, Mandelbrot only). Series
   approximation and nanoMB are legacy ([[techniques/series-approximation]]).
4. **Range handling** for float32: plain deltas to about 1e-25, then
   rescaled float32 with floatexp scale and floatexp skip-table entries
   ([[techniques/extended-range-floats]]); [[techniques/double-float]] only
   as a precision repair.
5. **Cheap per-pixel extras**: derivative interior detection, exterior
   distance estimation, split smooth counts, a separate colouring pass
   ([[techniques/interior-detection]], [[techniques/distance-estimation]],
   [[techniques/smooth-coloring]], [[techniques/coloring-methods]]).
6. **Image-level work**: progressive jittered anti-aliasing, frame reuse,
   coarse-to-fine refinement, auto iterations, exponential-map movies
   ([[techniques/anti-aliasing]], [[techniques/frame-reuse-and-progressive]],
   [[techniques/auto-iterations]], [[techniques/exponential-map]]).
   Solid guessing is weak on GPUs ([[techniques/guessing-and-subdivision]]).

## What is specific to almondbread

- float32-only GPU: ε = 2^-24 in skip radii, norms that do not underflow,
  per-quantity exponents in skip tables, and a verification harness,
  because float32 errors are silent ([[questions/float32-error-long-orbits]]).
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

Milestone 2 (2026-10-14) built the chunked, resumable iterate pass, derivative
interior detection and the verification harness; measurements live in
[[synthesis/benchmarks]]. The controls (WASD/arrows pan, Space zoom, Q/E zoom speed,
pointer and touch gestures) are fixed in [[decisions/controls]].

## Roadmap and open questions

The ranked implementation roadmap (rebasing kernel → chunked dispatch →
verification harness → interior/DE/colouring → BLA → rescaling → nucleus
reference → interaction → Imagina LA/AT → float-float → more formulas) is in
[[synthesis/deep-zoom-rendering-techniques]]. Open questions and
contradictions between sources are in `questions/` (see [[index]]).

State: first research run ingested (2026-10-14). No measurements of our own
yet.
