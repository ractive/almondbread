---
title: "Mu-Ency: floatexp"
type: source
authors: ["Robert Munafo"]
year: 2023
raw: "web/muency-floatexp.html"
status: digested
tags: [extended-range-floats, floatexp, glossary]
---

# Mu-Ency: floatexp

**URL:** <https://mrob.com/pub/muency/floatexp.html> · **Saved:** `raw/web/muency-floatexp.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Very short glossary entry (2023-06-21) defining the floatexp number format.

## What it covers

- floatexp = any format with far more exponent range than native hardware types: a normal float mantissa ("usually 32-bit single precision is sufficient") plus an explicit integer exponent (16 or 32 bits).
- Motivation: double reaches only about `1e-308`, so perturbation deltas in double cap zooms near `1e300`; arbitrary precision deltas would cancel the benefit of perturbation.
- Points to "softfloat" (not saved).

## Reliability

Secondary, correct but minimal; no performance data. Implementations differ: KF's original
floatexp uses a double mantissa ([mathr: Deep zoom theory and practice (2021)](mathr-deep-zoom-theory.md) §Rescaling) and added a single
variant in 2.15.3 ([Kalles Fraktaler 2 + manual](kf-manual.md) §kf-2.15.3); fractalshades uses int32 + float64
([fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](fractalforums-interior-detection.md) §Reply #8); Heiland-Allen's newer renderer uses
float + int and Fraktaler 3.1 added `doubleexp` ([Fraktaler 3 (documentation site, v3.1)](fraktaler-3-site.md) §Version 3.1).

## Useful sections

- §Floatexp (single section) — definition and motivation.

## Used by

[Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md), [How many bits does the reference orbit need?](../questions/precision-bits-formula.md)
