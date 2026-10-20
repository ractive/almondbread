---
title: "Quilez: procedural colour palettes"
type: source
authors: ["Inigo Quilez"]
year: 2013
raw: "web"
status: digested
tags: [palette, procedural, cosine]
---

# Quilez: procedural colour palettes

**URL:** <https://iquilezles.org/articles/palettes/> · **Saved:** not saved; notes in `raw/research/fractal-palettes/cosine-palettes.md` · **Terms:** article without licence; the formula is free to use, his parameter sets are not copied

## What it is

Short article: `colour(t) = a + b cos(2π (c t + d))` per RGB channel, four vector parameters.

## What it covers

- a = mean (bias), b = amplitude, c = frequency per channel, d = phase per channel.
- Integer c makes the palette periodic in t with period 1 (seamless); the mean colour over a cycle is a, so dense regions average to it.
- Weakness: channels clip at 0 and 1 and lightness is not controlled.

## Reliability

Secondary but exact (the formula is self-evident).

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)
