---
title: "Fractal Zoomer README"
type: source
authors: ["Christos Kalonakis"]
year: 2026
raw: "web/fractalzoomer-readme.md"
status: digested
tags: [fractal-zoomer, java, perturbation, guessing, gpl]
---

# Fractal Zoomer README

**URL:** <https://raw.githubusercontent.com/hrkalona/Fractal-Zoomer/master/README.md> · **Saved:** `raw/web/fractalzoomer-readme.md` · **Terms:** GPL-3.0 repository

## What it is

The short README of Fractal Zoomer (GitHub `hrkalona/Fractal-Zoomer`, GPL-3.0, pushed 2026-10-13), a Java fractal program with hundreds of formulas. The project's SourceForge page (sourceforge.net/projects/fractalzoomer), which FractalShark links to, returned HTTP 403 on 2026-10-14 and was not saved.

## What it covers

- Version shown: 1.0.9.5. Described as "the most complete fractal generating software using java".
- Listed optimisations: Boundary Tracing, Mariani/Silver, Successive Refinement, Perturbation Theory.
- Over 500 fractal functions, user formulas and user code, plane transformations, rotation, initial perturbation and value, bailout tests, palette editor, Julia sets and Julia map, polar projection, 3D height map, domain colouring, many colouring algorithms, colour cycling, orbits, image filters.
- It does not mention BLA, linear approximation, series approximation or extended-range floats. Other sources say Fractal Zoomer has these: FractalShark ported its float/exp and LA code (src: [FractalShark README](fractalshark-readme.md) §What other projects are used for inspiration?).

## Reliability

Primary but a feature list only. It understates the deep-zoom engine relative to what other projects credit to it. Facts on LA and number types need the code or the SourceForge docs, which were unavailable (HTTP 403, 2026-10-14).

## Useful sections

- §Optimizations (list) — boundary tracing, Mariani/Silver, successive refinement, perturbation.
- §Functions / §Includes (lists) — formula and colouring breadth.

## Used by

- [Fractal Zoomer](../implementations/fractal-zoomer.md)
- [Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md)
- [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)

# Citations

- <https://raw.githubusercontent.com/hrkalona/Fractal-Zoomer/master/README.md>
- [FractalShark README](fractalshark-readme.md)
