---
title: "XaoS"
type: implementation
status: draft
license: "GPL-2.0-or-later"
sources: ["[[sources/xaos-readme]]", "[[sources/wikipedia-xaos]]"]
tags: [implementation, realtime, frame-reuse, interactive]
---

# XaoS

A real-time interactive fractal zoomer, originally by Jan Hubička and Thomas
Marsh, now maintained by J.B. Langston and Zoltán Kovács; Qt-based for
Windows, macOS and Linux, plus a web version at xaos.app since 4.3
(September 2023) (src: [[sources/xaos-readme]]).

## Algorithms

- The Hubička (XaoS) algorithm: keep only the previous frame plus per-row and
  per-column coordinates; choose which saved rows and columns map where by
  treating it as an optimisation problem; fill rows/columns lacking good data
  with the nearest one, then recalculate them as CPU time allows, prioritising
  the zoom target (the edges when zooming out)
  (src: [[sources/wikipedia-xaos]]). Approximation cost is about
  `xsize + ysize` instead of `xsize * ysize`.
- Formulas: Mandelbrot powers 2-6, Octo, Barnsley, Newton (orders 3 and 4),
  Phoenix, Magnet; Julia sets from selected parts; custom formulas
  (src: [[sources/wikipedia-xaos]]).
- No perturbation or deep zoom: believed limited to hardware long double
  (unverified).

## Status

Active: last push 2026-09-11 (gh api); copyright 1996-2026, GPL-2.0 or later
(src: [[sources/xaos-readme]]).

## What almondbread can learn

The reuse idea, not the code: show the reprojected previous frame at once and
recompute what is missing, focus first. On a GPU a per-pixel affine
reprojection of stored per-pixel data replaces row/column matching (inference).
See [[techniques/frame-reuse-and-progressive]].
