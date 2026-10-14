---
title: "Wikibooks: Mandelbrot set interior"
type: source
authors: ["Wikibooks contributors"]
year: 2024
raw: "web"
status: unread
tags: [interior-detection, coloring-methods, multiplier]
---

# Wikibooks: Mandelbrot set interior

**URL:** <https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set_interior> · **Saved:** not saved (`raw: "web"`) · **Terms:** CC BY-SA (Wikibooks)

## What it is

Wikibooks chapter (Adam Majewski's "Fractals" book) on the interior of the Mandelbrot set: how to
detect interior points and how to colour them. Not saved or read for this wiki; summary from the
research notes only. The year is a placeholder.

## What it covers

- Multiplier test: a point is interior (in a hyperbolic component) when the multiplier of its attracting cycle, `(f^p)'(z0)`, has modulus below 1; requires the period and a cycle point (compare [[sources/mathr-interior-distance]]).
- Neighbour heuristic: use the result of neighbouring pixels to decide when to run costly interior checks (compare the "biased" method in [[sources/mathr-interior-distance]]).
- Interior colouring methods (for example by multiplier, period, or interior distance).

## Reliability

Secondary, community-written, long and uneven in quality; not read directly (status unread). Use
the primary sources ([[sources/mathr-interior-distance]], [[sources/cheritat-mandelbrot-rendering]],
[[sources/mathr-deep-zoom-page]] §6 Interior Detection) for anything load-bearing.

## Useful sections

- Not recorded (page not read).

## Used by

[[techniques/interior-detection]], [[techniques/coloring-methods]]
