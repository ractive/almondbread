---
title: "colorcet: the CET perceptually uniform colour maps"
type: source
authors: ["Peter Kovesi", "HoloViz"]
year: 2015
raw: "web"
status: digested
tags: [palette, colour-maps, cyclic, licence]
---

# colorcet: the CET perceptually uniform colour maps

**URL:** <https://colorcet.com/> · **Saved:** not saved; notes in `raw/research/fractal-palettes/perceptual-maps.md` · **Terms:** CC BY 4.0, credit required

## What it is

Kovesi's CET colour maps (linear, diverging, rainbow, cyclic, isoluminant, colour-blind) as CSV and in the colorcet Python package.

## What it covers

- Names encode type, hue letters, lightness range and chroma; `_s25` = cyclic map rotated by 25 %.
- Includes colour-blind-safe cyclic maps (CBC1, CBC2, CBTC1, CBTC2).
- Data: 256 float `r,g,b` triples per map; almondbread's `derive.py` reads the lists from colorcet's `__init__.py`.

## Reliability

Primary data source.

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)
