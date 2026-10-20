---
title: "fractalset.com viewer and gradient editor"
type: source
authors: ["FractalSet"]
year: 2026
raw: "web"
status: digested
tags: [palette, web-viewer, density]
---

# fractalset.com viewer and gradient editor

**URL:** <https://fractalset.com/> · **Saved:** not saved; notes in `raw/research/fractal-palettes/fractalset.md` · **Terms:** proprietary, no licence stated ("© 2026 FractalSet"); described, no colour data copied

## What it is

Web Mandelbrot/Julia/Burning Ship viewer with 24 built-in palettes, a custom gradient slot and a community gradient editor.

## What it covers

- Palette density is a log slider: factor `2^(d/10)`, d in -25..25 (about 0.18x to 5.7x).
- Palette rotation in degrees (-180..180) is a phase offset of the cycle, not a hue rotation.
- Smooth mode maps `t = 0.5 + 0.5 sin(2π μ 2^(d/10) / 400 + rot)`: a ping-pong mapping, so any palette is seamless; the ends dwell longer than the middle.
- Built-ins are short key-colour lists (about 5-10 keys) interpolated linearly in RGB; some are stacked layers with repeat counts and blend modes.
- Mood names (Aurora, Inferno, Ocean, Sunset, Gold, Copper, Pastels, Deep Sea, Lava, ...); interior is plain black.
- Animation: colour scroll, density pulse, hue shift. Gradient editor with HSL stops and generated two-word names.

## Reliability

Read from the live site and its minified client bundles; behaviour is our reading of the code, not documentation.

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)
