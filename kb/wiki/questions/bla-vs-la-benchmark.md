---
title: "How much faster is Imagina-style LA than binary-tree BLA on a GPU?"
type: question
status: open
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
