---
title: "Fractint / Xfractint help: drawing passes"
type: source
authors: ["Stone Soup Group"]
year: 2006
raw: "web"
status: digested
tags: [fractint, guessing, boundary-tracing, mariani-silver, implementations]
---

# Fractint / Xfractint help: drawing passes

**URL:** <https://sources.debian.org/src/xfractint/3.04-1/help3.src> · **Saved:** not saved · **Terms:** Fractint/Xfractint freeware with source, help text copyright the Stone Soup Group; paraphrased

## What it is

The help source of Xfractint 3.04, the Unix port of Fractint, documenting the
`passes=` drawing modes, including solid guessing, boundary tracing and
tesseral (Mariani/Silver). Year is the 3.04 release era (approximate).

## What it covers

- Modes: single pass, dual pass, triple pass, solid guessing (default), solid
  guessing after pass n, boundary tracing, tesseral, diffusion (successive
  refinement).
- Solid guessing: two to four visible passes. The first computes one pixel per
  4x4, 8x8 or 16x16 box and guesses the next level; later passes skip blocks
  surrounded by one colour. "Solid-guessing can guess wrong, but it sure
  guesses quickly!"
- Boundary tracing: accurate only for fractal types without islands of colour
  (fine for Mandelbrot, not for Newton).
- Tesseral: fill rectangles whose boundary is one colour; subject to errors
  when islands of colour sit inside a rectangle.
- Single and dual pass give identical images; single pass is the accuracy
  reference.

## Reliability

Primary documentation of the program that popularised these methods. Describes
behaviour, not the exact guessing rule. Discrete colour bands only, no smooth
colouring or perturbation.

## Useful sections

- The "passes" option entries: solid guessing, boundary tracing, tesseral.

## Used by

[[techniques/adaptive-sampling]]
