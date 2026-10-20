---
title: "Fractal Zoomer"
type: implementation
status: draft
license: "GPL-3.0"
sources: ["[[sources/fractalzoomer-readme]]", "[[sources/kf-page]]", "[[sources/fractalshark-readme]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]"]
tags: [implementation, java, perturbation, bla, linear-approximation]
---

# Fractal Zoomer

Christos Kalonakis's (hrkalona, forum name superheal) Java fractal program,
"the most complete fractal generating software using java": over 500
formulas, user formulas and functions, Julia sets and maps, plane
transformations, many colouring algorithms, 3D height maps, domain colouring
(src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)). Needs Java 1.8+
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Algorithms

- Optimisations listed: boundary tracing, Mariani/Silver, successive
  refinement, perturbation theory (src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)).
- Perturbation with BLA (configured by the user) for deep zoom
  (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)); float+exponent type and linear-approximation
  implementations that FractalShark ported to CUDA
  (src: [FractalShark README](../sources/fractalshark-readme.md)).
- Rebasing used for Mandelbrot powers 2-5, Burning Ship, lambda, nova, magnet
  and Newton parameter space, and Julia sets, per the author in 2021
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #8, §Reply #11).
  See [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md).
- Exact option list (BLA variants, NanoMB, SA, which formulas support
  perturbation) not retrieved: the SourceForge page returned HTTP 403 on
  2026-10-14.

## Performance (attributed)

Example minibrot at 5e-433 in 30 s with perturbation/BLA plus boundary
tracing, just over 2 min without boundary tracing, on a 16-thread Ryzen 7
2700X; the fastest program in mathr's list for that location
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Status

Active: last push 2026-10-13 (gh api); README version 1.0.9.5; GPL-3.0.
FractalShark's author says it has "much more polish" than FractalShark
(src: [FractalShark README](../sources/fractalshark-readme.md)).

## What almondbread can learn

Boundary tracing gave a 4x win at the 5e-433 minibrot on CPU, which argues
for tile-level early-out experiments ([Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md)).
Its breadth of perturbed formulas is the evidence for
[Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md). GPL code must not be copied.

# Citations

- [Fractal Zoomer README](../sources/fractalzoomer-readme.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [FractalShark README](../sources/fractalshark-readme.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
