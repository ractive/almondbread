---
title: "iOS and iPadOS port considerations"
type: platform
status: draft
sources: ["sources/swift-mandelbrot-readme", "sources/fractalshark-readme", "sources/fraktaler-3-site", "sources/imagina-hinfla-evaluator", "sources/mlx-watchdog-issue"]
tags: [ios, ipad, memory, thermal, port]
---

# iOS and iPadOS port considerations

almondbread started on macOS, with an iPhone/iPad port planned (written before the port; the
port runs since 2026-10-18: [iOS and iPadOS on a device: first run, signing, memory, thermal, background](ios-device.md), [iOS and iPadOS on the simulator: setup, checks, what broke](ios-simulator.md),
[Controls: keyboard, pointer and touch](../decisions/controls.md) "Touch"). Little here is sourced: most items are inferences or
background knowledge and are marked so.

## What stays the same

- The Metal kernels, number-type ladder and skip tables are the same; A-series and
  M-series GPUs share the architecture family ([Apple GPU architecture for compute](apple-gpu-architecture.md)) (inference).
- Prior art exists: swift-mandelbrot runs SwiftUI + Metal on iPhone, iPad and Mac
  with Float, double-float, perturbation, BLA and rebasing, and claims zooms past
  10^1000 (src: [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)). See [swift-mandelbrot (drquicksilver)](../implementations/swift-mandelbrot.md).
- The chunked, resumable kernel ([GPU watchdog and chunked, resumable kernels](gpu-watchdog-and-chunking.md)) is what
  makes the port cheap: short command buffers suit any watchdog (inference).

## Memory

- Skip tables are O(M) for an M-iteration reference. At 48 B per entry and about
  2M entries, a 1e7-iteration reference needs about 1 GB worst case (notes:
  iteration_skipping §6) — fine on a Mac, too much for many iPhones (inference).
- Mitigations with sources:
  - Reference-orbit compression with on-the-fly decompression (from Imagina,
    extended by FractalShark) saved "multiple gigabytes" at period 6e8
    (src: [FractalShark README](../sources/fractalshark-readme.md) §6. Reference orbit compression and on-the-fly decompression).
    Algorithm details unknown: [How does Imagina/FractalShark reference-orbit compression work?](../questions/reference-orbit-compression.md).
  - Fraktaler 3's "BLA skip levels" drop fine table levels to save memory
    (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
  - Imagina's period-aligned LA stages have fewer entries than a full binary tree
    (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)); see [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md).
  - A nucleus reference ([Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md)) stores only p
    iterations, not maxIter.
- iOS enforces a per-app memory limit (jetsam) well below physical RAM, and an
  "increased memory limit" entitlement exists (unverified, background knowledge;
  exact limits vary by device and OS). Design for a budget the app queries at run
  time (`os_proc_available_memory`, unverified) and size tables to it.
- Per-pixel state buffers scale with screen pixels times samples; at 2732x2048
  (iPad Pro) with 32 B per pixel that is about 180 MB for one sample (inference).
  Accumulate AA samples into colour instead of keeping per-sample state.
- **Measured (2026-10-18, M1 Pro, release, iOS scheduler defaults, [iOS and iPadOS on a device: first run, signing, memory, thermal, background](ios-device.md)
  "Memory"):** the renderer holds ~108 MB fixed plus ~67 B per output texel on the GPU (+12 B
  of drawables): 449–470 MB at 2752×2064 (iPad Pro 13"), whether at home or at 1e-3007. AA
  already accumulates into colour (FL), so supersampling adds nothing per pixel. References
  and BLA/LA tables stay under 4 MB at the default 200 k auto limit (nucleus references are a
  period long); a manual 10 M limit costs ~130 B per reference point (+408 MB process
  footprint over 1 M). The app sizes the drawable and caps the iteration limit from
  `os_proc_available_memory()` (the API exists; 0 means over the limit on a device and is
  what the simulator always returns).

## Thermal and power

- Phones and fanless iPads throttle under sustained GPU load (unverified,
  background knowledge). Deep renders should lower the budget per command buffer,
  pause on thermal state `.serious`/`.critical` (`ProcessInfo.thermalState`,
  unverified API) and prefer progressive results over throughput (inference).
- As built (2026-10-18): serious halves the per-refresh GPU budgets, cuts AA, drops "compute
  further" and caps 60 Hz; critical stops refinement after the first full frame and caps
  30 Hz ([iOS and iPadOS on a device: first run, signing, memory, thermal, background](ios-device.md) "Thermal"). Real thermal curves: device only.
- The Imagina-style LA + AT path matters more on mobile: fewer steps per pixel is
  less energy, not just less time (inference).

## Watchdog and background execution

- The macOS figure (~5 s per command buffer) is a community report
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)); iOS behaviour is unknown
  ([What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md)). Assume it is stricter.
- iOS does not allow GPU work from a backgrounded app; command buffers submitted
  after backgrounding fail (unverified, background knowledge). Save the per-pixel
  state buffer on `sceneDidEnterBackground` and resume (inference). As built: on the
  scene's background notification the renderer waits for its work in flight and submits
  nothing until foreground; GPU state stays allocated, the view resumes
  ([iOS and iPadOS on a device: first run, signing, memory, thermal, background](ios-device.md) "Background").

## CPU side

- GMP/MPFR build for arm64 iOS; the reference orbit on fewer, slower cores costs
  more, so the nucleus reference and reference reuse across zooms are more
  important ([Arbitrary precision on Apple Silicon (GMP, MPFR)](mpfr-gmp-on-apple-silicon.md)) (inference).
- Licensing: GMP/MPFR are LGPL; static linking into an App Store app has LGPL
  relinking obligations (unverified, background knowledge). **Moot as built
  (checked 2026-10-18):** almondbread's BigFloat is its own Rust code with no
  dependencies; nothing LGPL is linked into the iOS app (MIT / Apache-2.0 / Zlib crates and
  UniFFI under MPL-2.0; `cargo deny check licenses` passes; [iOS and iPadOS on a device: first run, signing, memory, thermal, background](ios-device.md)).

## Gaps

- No sourced iOS memory limits, watchdog value or thermal curves for GPU compute.

# Citations

- [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)
- [FractalShark README](../sources/fractalshark-readme.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
- [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)
