---
title: "Wikipedia: XaoS"
type: source
authors: ["Wikipedia contributors"]
year: 2026
raw: "web/wikipedia-xaos.html"
status: digested
tags: [xaos, frame-reuse, real-time-zoom, implementations]
---

# Wikipedia: XaoS

**URL:** <https://en.wikipedia.org/wiki/XaoS> · **Saved:** `raw/web/wikipedia-xaos.html` (+ `.txt`) · **Terms:** CC BY-SA 4.0 (Wikipedia)

## What it is

Encyclopedia article on XaoS, the GPL real-time fractal zoomer by Jan Hubička and Thomas Marsh,
mainly notable here for describing the Hubička (XaoS) zooming algorithm that reuses the previous
frame.

## What it covers

- XaoS: GPL, C/C++, Qt-based, cross-platform (Linux, Windows, macOS, web at xaos.app); stable release 4.3.7 (26 July 2026). Formulas include Mandelbrot powers 2-6, Newton, Phoenix, Magnet, custom formulas.
- Hubička algorithm: keep only the previous frame, with pixels kept aligned as whole rows and columns; at each zoom step choose which saved rows/columns map to which new positions (treated as an optimisation problem, after heuristics failed); missing rows/columns are first filled from the nearest one and recomputed as CPU time allows.
- Priority: when zooming in, recompute the zoom target area first (on screen longest, where the user looks); when zooming out, the edges.
- Motivation: full recomputation per frame was too slow for interactive zoom and causes twinkling without antialiasing.
- Algorithm reused in other zoomers (e.g. rtzme).

## Reliability

Secondary, short and descriptive; no algorithmic detail (no cost model, no exact matching method).
Use the XaoS source/readme for specifics. The rows/columns trick only applies to axis-aligned zooms
without rotation (inference).

## Useful sections

- §Hubička algorithm — row/column reuse, optimisation matching, recompute priority.

## Used by

[XaoS](../implementations/xaos.md), [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)
