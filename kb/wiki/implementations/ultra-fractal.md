---
title: "Ultra Fractal 6"
type: implementation
status: draft
license: "commercial"
sources: ["[[sources/ultrafractal-perturbation-calculations]]", "[[sources/ultrafractal-perturbation-equations]]"]
tags: [implementation, commercial, perturbation, glitch-detection]
---

# Ultra Fractal 6

Frederik Slijkerman's commercial fractal program (Windows and macOS, unverified) with a formula
language. Version 6 added perturbation deep zooming.

## Algorithms

- Arbitrary precision for any formula; perturbation for formulas that support
  it. One full-precision reference at the image centre, other pixels as
  double-precision deviations (src:
  [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md) §How does this work?).
- Automatic detection of precision problems and automatic extra references;
  users cannot place references by hand. A Perturbation setting (Precise,
  Fast, Very Fast) replaces periodicity checking and sets the tolerance; the
  Statistics window shows the share of arbitrary-precision work skipped,
  normally ~99% (src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)).
- Series approximation only in the built-in Mandelbrot formula; "hard to
  generalize" and with smaller benefit than perturbation
  (src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)). No BLA or
  rebasing is mentioned.
- Formula authors write `perturbinit`/`perturbloop` sections using `#dz` and
  `#dpixel` (Mandelbrot: `#dz = 2*#z*#dz + sqr(#dz) + #dpixel`) and declare
  validity with `perturb`; positive integer powers reduce, non-integer or
  complex powers do not (src: [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)).
- Credits K.I. Martin and SuperFractalThing for the algorithm.

## Performance (vendor claim)

A 2e243 Mandelbrot zoom in 42 s (UF "What's new" page, not saved; notes:
implementations_and_formulas §Q1).

## Status

Commercial, current. Last checked 2026-10-14 via its help pages.

## Contradictions

The research notes say the UF docs "do not mention BLA or SA"; the
perturbation calculations page does mention series approximation for the
built-in Mandelbrot formula (src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)).
The notes' statement was about the formula-writing page only.

## What almondbread can learn

Hand-written perturbed equations per formula are the low-tech alternative to
an opcode compiler ([Hybrid formulas](../formulas/hybrids.md)). Its glitch approach is the legacy
multi-reference one ([Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](../techniques/glitch-detection.md)).

# Citations

- [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)
- [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)
