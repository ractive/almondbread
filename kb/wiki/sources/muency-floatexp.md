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
floatexp uses a double mantissa ([[sources/mathr-deep-zoom-theory]] §Rescaling) and added a single
variant in 2.15.3 ([[sources/kf-manual]] §kf-2.15.3); fractalshades uses int32 + float64
([[sources/fractalforums-interior-detection]] §Reply #8); Heiland-Allen's newer renderer uses
float + int and Fraktaler 3.1 added `doubleexp` ([[sources/fraktaler-3-site]] §Version 3.1).

## Useful sections

- §Floatexp (single section) — definition and motivation.

## Used by

[[techniques/extended-range-floats]], [[questions/precision-bits-formula]]
