---
title: "Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)"
type: source
authors: ["Robot Moon (robotmoon.com)"]
year: 2026
raw: "web/robotmoon-mandelbrot-set.html"
status: digested
tags: [places, locations, coloring-methods, deep-zoom]
---

# Robot Moon: The Mandelbrot Set

**URL:** <https://robotmoon.com/mandelbrot-set/> · **Saved:** `raw/web/robotmoon-mandelbrot-set.html` (+ `.txt`), retrieved 2026-10-14 · **Terms:** no licence or author stated; personal research copy, not redistributed

## What it is

A long illustrated web page with an in-browser explorer: the iteration, the named
regions of the set, then three galleries of deeper views ("Deep zoom",
"Labyrinthine", "Transfinite", down to 2.86e-111). Each image carries a caption with the
viewer's readout (vertical span, iteration limit). Credits Jussi Härkönen (stripe-average
colouring) and links mathr's deep-zoom article ([mathr: Deep zoom theory and practice (2021)](mathr-deep-zoom-theory.md)).

## What it covers

- Named regions with coordinates: upside-down seahorse (-0.743030 + 0.126433i, span
  1.21e-2), seahorse tail Misiurewicz endpoint (-0.7435669 + 0.1314023i, 1.72e-3),
  double spiral nexus (-0.77568377 + 0.13646737i, 1.2e-3), tail path
  (-0.74364990 + 0.13188204i, 5.54e-4), double hook (-0.74364085 + 0.13182733i, 9.05e-5),
  concentric satellite crowns (-0.743643135 + 0.131825963i, 1.10e-5): these are the
  centres and widths of Wolfgang Beyer's Commons zoom sequence
  ([Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](beyer-mandelbrot-zoom-sequence.md)), quoted with his magnifications.
- Regions only it gives: seahorse valley spiral sanctuary (near -0.747 + 0.108i, span
  5.8e-3), scepter crown (near -1.25066 + 0.02012i, 5.2e-4), snowflake (near
  -0.658448 - 0.466852i, viewer span 8.67e-4).
- Deep views named but without usable coordinates: a period-70 nucleus near
  -1.4303584327 at 1.02e-34, rose window (2.03e-42), quad-spiral star (5.18e-54), needle
  at 1e-60 and 1e-100, scepter star-heart (2.86e-111). The page gives a span and a
  region, not a centre, so they cannot be revisited from it.
- Spans are *vertical* spans of 4:3 images: the shorter side, which is what
  [Places: a catalogue of famous views to visit](../synthesis/places.md) stores.

## Reliability

Secondary and anonymous. Its coordinates for the classic views match Beyer's to every
digit; the places we took from it render as described (checked 2026-10-14, see
[Places: a catalogue of famous views to visit](../synthesis/places.md)). The "double spiral nexus" repeats Wikipedia's 8-digit
approximation of M(23,2), which is preperiod 24 with z_0 = 0
([Wikipedia: Misiurewicz point](wikipedia-misiurewicz-point.md)). Some captions mix span and magnification
(the 1e-100 needle is captioned "10^100×").

## Useful sections

- §Overview and §Deep zoom: the named regions and their coordinates.
- Image captions (`alt` text in the HTML): the viewer readouts.
