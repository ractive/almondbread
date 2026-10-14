---
title: "Ultra Fractal help: Perturbation calculations"
type: source
authors: ["Frederik Slijkerman"]
year: 2021
raw: "web/ultrafractal-perturbation-calculations.html"
status: digested
tags: [ultra-fractal, perturbation, glitch-detection, series-approximation, commercial]
---

# Ultra Fractal help: Perturbation calculations

**URL:** <https://www.ultrafractal.com/help/formulas/perturbationcalculations.html> · **Saved:** `raw/web/ultrafractal-perturbation-calculations.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

A user-facing help page of Ultra Fractal (commercial Windows/macOS fractal program) on how its perturbation mode works and how to tune it.

## What it covers

- Ultra Fractal has arbitrary precision; past the "regular precision level" renders become hours or days. Perturbation (credited to K.I. Martin, first in SuperFractalThing) makes deep zooms "orders of magnitude" faster.
- Perturbation is automatic when the formula supports it; there is no switch.
- Method: one full-precision reference orbit at the image centre, storing every `Z`; other pixels iterate their deviation from it in double precision.
- When double precision cannot follow a pixel, Ultra Fractal detects it and adds new reference orbits automatically. Users cannot add reference points by hand.
- The Statistics window shows e.g. "Arbitrary (Perturbation: 99.7%)": the share of arbitrary-precision work skipped.
- With perturbation on, the Periodicity Checking setting becomes a Perturbation tolerance: Precise (default), Fast, Very Fast. Precise can add so many references that efficiency drops to ~60 %; Fast options trade accuracy for speed.
- Series approximation is used only in the built-in Mandelbrot formula. The page says SA is hard to generalise and gives much less benefit than perturbation itself.
- Formula authors can add perturbation support (see [[sources/ultrafractal-perturbation-equations]]).

## Reliability

Primary for Ultra Fractal's behaviour, but marketing-level: it does not say how glitches are detected or how new references are chosen. Year 2021 is from the manifest; the page itself is undated. No BLA mentioned.

## Useful sections

- §How does this work? — reference orbit, double-precision deltas, automatic new references.
- §Notes — statistics readout, Precise/Fast/Very Fast tolerance, SA only in built-in Mandelbrot.

## Used by

- [[implementations/ultra-fractal]]
- [[techniques/perturbation]]
- [[techniques/glitch-detection]]
- [[techniques/series-approximation]]
