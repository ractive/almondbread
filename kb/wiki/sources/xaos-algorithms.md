---
title: "XaoS: Algorithms document"
type: source
authors: ["Jan Hubička", "XaoS project"]
year: 2026
raw: "web"
status: digested
tags: [xaos, guessing, frame-reuse, periodicity, dynamic-resolution, implementations]
---

# XaoS: Algorithms document

**URL:** <https://raw.githubusercontent.com/xaos-project/XaoS/master/src/engine/algorithms.md> · **Saved:** not saved · **Terms:** part of the GPL-2.0 XaoS repository; paraphrased, no code

## What it is

The design notes in the XaoS source tree describing how the real-time zoomer
avoids computing every pixel: frame reuse, solid guessing on new rows and
columns, interlacing, gated periodicity checking and time-budgeted dynamic
resolution.

## What it covers

- Main idea: a pixel from earlier frames is reused if it lies within one pixel
  step of where it is needed.
- Boundary detection (trace a boundary, fill the inside) is used only for a
  fresh image. The text says the claim that it never errs is false: a lake
  connection too thin to be seen at the sampling resolution makes it miss a
  whole area.
- Solid guessing on added lines: if the nearest three pixels on the lines above
  and below agree, a 9x9 block is assumed equal. Where new rows and columns
  cross, only four pixels are tested. Errors happen when a sharp feature enters
  from an uncomputed side.
- Interlacing: compute odd lines without guessing, then guess the even ones
  from both neighbours. Saves about 30% on the main Mandelbrot image.
- Periodicity checking loop is much slower, so it is used only after guessing
  has found an inside-set pixel.
- Inner loop unrolled 8x with one bailout test per block.
- Dynamic resolution: compute only what fits the frame time; missing rows and
  columns copy the nearest. Priority = size of the uncomputed block (binary
  split), raised for rows/columns near the zoom centre. Frame budget = 5x the
  rest of the program's time, clamped to 15–35 fps while moving, 5–15 fps when
  still.

## Reliability

Primary design notes by the original author, but old (written for 1990s CPUs)
and partly informal ("It is hoped that it does not introduce many errors").
No measurements beyond the 30% figure. Row/column reuse assumes axis-aligned
zoom without rotation.

## Useful sections

- §Calculating New Pixels: solid guessing on lines, its failure case, interlacing.
- §Calculation of the Mandelbrot Set: gated periodicity checking.
- §Dynamic Resolution: priority order and frame-time budget.

## Used by

[[techniques/adaptive-sampling]]
