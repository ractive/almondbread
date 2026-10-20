---
title: "How much faster is Imagina-style LA than binary-tree BLA on a GPU?"
type: question
status: answered
sources: ["sources/mathr-deep-zoom-theory-again", "sources/philthompson-bla", "sources/imagina-hinfla-evaluator", "sources/fractalshark-readme", "sources/fraktaler-3-site", "sources/fractalforums-interior-detection"]
tags: [bla, linear-approximation, benchmark, iteration-skipping]
---

# How much faster is Imagina-style LA than binary-tree BLA on a GPU?

## Question

On Apple GPUs in float32, how do mathr's binary-tree BLA ([Bivariate linear approximation (BLA)](../techniques/bla.md)) and
Imagina's period-aligned LA with AT ([Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md))
compare in time per frame, steps per pixel, memory and image error, by depth and
iteration count? And does series approximation still win anywhere
([Series approximation (legacy)](../techniques/series-approximation.md))?

## Why it matters

The roadmap puts BLA first and LA + AT later as "the largest win at minibrot
depths" ([Deep zoom Mandelbrot rendering techniques](../synthesis/deep-zoom-rendering-techniques.md)). That ordering rests on code
reading, not measurement.

## What sources say

- mathr (2022): benchmarks are needed "to see how it compares speed-wise before
  declaring an overall winner" between BLA and SA
  (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Bilinear Approximation).
- Very Plotter (JavaScript, M1): BLA beat SA by 10.2x at 2.9e89 and 36.2x at
  4.99e152, but SA was faster at a 9.5e693 floatexp location
  (src: [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)).
- Imagina's evaluator contains only LA/AT, no SA (src: [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)).
- FractalShark has two CUDA LA implementations but publishes no per-pixel LA speedups
  (src: [FractalShark README](../sources/fractalshark-readme.md) §3. CUDA-based linear approximation implementations).
- Fraktaler 3 uses BLA with a step limit (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- No source compares LA with BLA.

## How to resolve

Implement both behind the same kernel interface (function constant), run the
benchmark location set from [How large is float32 perturbation error over long orbits?](float32-error-long-orbits.md), and record:
wall time, mean/max steps per pixel, table build time and size, and pixel diff
against skip-off. Collect mathr-style statistics (steps BLA vs perturbation,
rebases due to smallness vs running out of reference)
(src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)).

## Answer (M6, measured 2026-10-15, M1 Pro, release)

Both are built behind function constants ([Bivariate linear approximation (BLA)](../techniques/bla.md), [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)
"As built (M6)"); numbers in [almondbread benchmarks and verification record](../synthesis/benchmarks.md) M6.

- **Time.** LA is 1.9–2.4× faster than BLA at deep needle minibrots (nucleus references of period
  79–834 whose limit spans 120–250 periods), 1.1× at sea-8007 (5 periods), even on centre
  references (sea-1e20, dendrites) and slower everywhere the plain kernel runs (1.4–5× if forced:
  a lookup at every near-return of a non-linear orbit). The renderer uses LA only in the first
  case (deep kernel, nucleus reference, ≥ 4 periods in the limit) and keeps BLA elsewhere.
- **Steps.** 1.5–2.1× fewer than BLA at the needles, 0.9–1.2× elsewhere in the CPU model; the AT
  removes another 4–10 % but costs as much as it saves on the GPU (off by default).
- **Memory and build.** Small: 17–190 KB and 0.3–10 ms on a thread for the benchmark references.
- **Error.** Same as BLA: class mismatch against the CPU oracle within 0.001 % of BLA's at all 21
  locations, no exterior painted interior.
- **SA.** Local SA (M5) is what pays in dense and spiral views; no linear skip (BLA or LA) helps
  where deltas reach the orbit's size within ~2 000 iterations (the spiral).

# Citations

- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](../sources/philthompson-bla.md)
- [Imagina HInfLAEvaluator (source code)](../sources/imagina-hinfla-evaluator.md)
- [FractalShark README](../sources/fractalshark-readme.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
