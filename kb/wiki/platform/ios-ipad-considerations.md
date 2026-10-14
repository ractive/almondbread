---
title: "iOS and iPadOS port considerations"
type: platform
status: draft
sources: ["[[sources/swift-mandelbrot-readme]]", "[[sources/fractalshark-readme]]", "[[sources/fraktaler-3-site]]", "[[sources/imagina-hinfla-evaluator]]", "[[sources/mlx-watchdog-issue]]"]
tags: [ios, ipad, memory, thermal, port]
---

# iOS and iPadOS port considerations

almondbread starts on macOS, with an iPhone/iPad port planned. Little here is
sourced: most items are inferences or background knowledge and are marked so.

## What stays the same

- The Metal kernels, number-type ladder and skip tables are the same; A-series and
  M-series GPUs share the architecture family ([[platform/apple-gpu-architecture]]) (inference).
- Prior art exists: swift-mandelbrot runs SwiftUI + Metal on iPhone, iPad and Mac
  with Float, double-float, perturbation, BLA and rebasing, and claims zooms past
  10^1000 (src: [[sources/swift-mandelbrot-readme]]). See [[implementations/swift-mandelbrot]].
- The chunked, resumable kernel ([[platform/gpu-watchdog-and-chunking]]) is what
  makes the port cheap: short command buffers suit any watchdog (inference).

## Memory

- Skip tables are O(M) for an M-iteration reference. At 48 B per entry and about
  2M entries, a 1e7-iteration reference needs about 1 GB worst case (notes:
  iteration_skipping §6) — fine on a Mac, too much for many iPhones (inference).
- Mitigations with sources:
  - Reference-orbit compression with on-the-fly decompression (from Imagina,
    extended by FractalShark) saved "multiple gigabytes" at period 6e8
    (src: [[sources/fractalshark-readme]] §6. Reference orbit compression and on-the-fly decompression).
    Algorithm details unknown: [[questions/reference-orbit-compression]].
  - Fraktaler 3's "BLA skip levels" drop fine table levels to save memory
    (src: [[sources/fraktaler-3-site]]).
  - Imagina's period-aligned LA stages have fewer entries than a full binary tree
    (src: [[sources/imagina-hinfla-evaluator]]); see [[techniques/linear-approximation-imagina]].
  - A nucleus reference ([[techniques/reference-selection]]) stores only p
    iterations, not maxIter.
- iOS enforces a per-app memory limit (jetsam) well below physical RAM, and an
  "increased memory limit" entitlement exists (unverified, background knowledge;
  exact limits vary by device and OS). Design for a budget the app queries at run
  time (`os_proc_available_memory`, unverified) and size tables to it.
- Per-pixel state buffers scale with screen pixels times samples; at 2732x2048
  (iPad Pro) with 32 B per pixel that is about 180 MB for one sample (inference).
  Accumulate AA samples into colour instead of keeping per-sample state.

## Thermal and power

- Phones and fanless iPads throttle under sustained GPU load (unverified,
  background knowledge). Deep renders should lower the budget per command buffer,
  pause on thermal state `.serious`/`.critical` (`ProcessInfo.thermalState`,
  unverified API) and prefer progressive results over throughput (inference).
- The Imagina-style LA + AT path matters more on mobile: fewer steps per pixel is
  less energy, not just less time (inference).

## Watchdog and background execution

- The macOS figure (~5 s per command buffer) is a community report
  (src: [[sources/mlx-watchdog-issue]]); iOS behaviour is unknown
  ([[questions/metal-watchdog-limit]]). Assume it is stricter.
- iOS does not allow GPU work from a backgrounded app; command buffers submitted
  after backgrounding fail (unverified, background knowledge). Save the per-pixel
  state buffer on `sceneDidEnterBackground` and resume (inference).

## CPU side

- GMP/MPFR build for arm64 iOS; the reference orbit on fewer, slower cores costs
  more, so the nucleus reference and reference reuse across zooms are more
  important ([[platform/mpfr-gmp-on-apple-silicon]]) (inference).
- Licensing: GMP/MPFR are LGPL; static linking into an App Store app has LGPL
  relinking obligations (unverified, background knowledge). Revisit in a decision
  page before the port.

## Gaps

- No sourced iOS memory limits, watchdog value or thermal curves for GPU compute.
