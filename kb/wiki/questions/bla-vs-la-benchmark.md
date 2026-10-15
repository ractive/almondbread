---
title: "How much faster is Imagina-style LA than binary-tree BLA on a GPU?"
type: question
status: answered
sources: ["[[sources/mathr-deep-zoom-theory-again]]", "[[sources/philthompson-bla]]", "[[sources/imagina-hinfla-evaluator]]", "[[sources/fractalshark-readme]]", "[[sources/fraktaler-3-site]]", "[[sources/fractalforums-interior-detection]]"]
tags: [bla, linear-approximation, benchmark, iteration-skipping]
---

# How much faster is Imagina-style LA than binary-tree BLA on a GPU?

## Question

On Apple GPUs in float32, how do mathr's binary-tree BLA ([[techniques/bla]]) and
Imagina's period-aligned LA with AT ([[techniques/linear-approximation-imagina]])
compare in time per frame, steps per pixel, memory and image error, by depth and
iteration count? And does series approximation still win anywhere
([[techniques/series-approximation]])?

## Why it matters

The roadmap puts BLA first and LA + AT later as "the largest win at minibrot
depths" ([[synthesis/deep-zoom-rendering-techniques]]). That ordering rests on code
reading, not measurement.

## What sources say

- mathr (2022): benchmarks are needed "to see how it compares speed-wise before
  declaring an overall winner" between BLA and SA
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- Very Plotter (JavaScript, M1): BLA beat SA by 10.2x at 2.9e89 and 36.2x at
  4.99e152, but SA was faster at a 9.5e693 floatexp location
  (src: [[sources/philthompson-bla]]).
- Imagina's evaluator contains only LA/AT, no SA (src: [[sources/imagina-hinfla-evaluator]]).
- FractalShark has two CUDA LA implementations but publishes no per-pixel LA speedups
  (src: [[sources/fractalshark-readme]] §3. CUDA-based linear approximation implementations).
- Fraktaler 3 uses BLA with a step limit (src: [[sources/fraktaler-3-site]]).
- No source compares LA with BLA.

## How to resolve

Implement both behind the same kernel interface (function constant), run the
benchmark location set from [[questions/float32-error-long-orbits]], and record:
wall time, mean/max steps per pixel, table build time and size, and pixel diff
against skip-off. Collect mathr-style statistics (steps BLA vs perturbation,
rebases due to smallness vs running out of reference)
(src: [[sources/fractalforums-interior-detection]]).

## Answer (M6, measured 2026-10-15, M1 Pro, release)

Both are built behind function constants ([[techniques/bla]], [[techniques/linear-approximation-imagina]]
"As built (M6)"); numbers in [[synthesis/benchmarks]] M6.

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
