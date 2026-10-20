---
title: "BIDS colormap: viridis, magma, inferno, plasma"
type: source
authors: ["Nathaniel J. Smith", "Stéfan van der Walt", "Eric Firing"]
year: 2015
raw: "web"
status: digested
tags: [palette, colour-maps, perceptual, licence]
---

# BIDS colormap: viridis, magma, inferno, plasma

**URL:** <https://github.com/BIDS/colormap> · **Saved:** not saved; notes in `raw/research/fractal-palettes/perceptual-maps.md` · **Terms:** CC0 1.0 (credit requested)

## What it is

The matplotlib default colour maps, designed in CAM02-UCS with monotonic lightness; 256 float triples each.

## What it covers

- Sequential (dark to light): as fractal palettes they need mirror mapping or they show a seam each cycle.
- The same lists ship in matplotlib's `_cm_listed.py`, which also has cividis, turbo and twilight (twilight: matplotlib licence only, the author's repository has none).

## Reliability

Primary data source.

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)
