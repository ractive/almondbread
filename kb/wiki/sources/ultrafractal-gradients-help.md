---
title: "Ultra Fractal help: gradients and colouring settings"
type: source
authors: ["Frederik Slijkerman"]
year: 2024
raw: "web"
status: digested
tags: [palette, gradient, file-format, ugr]
---

# Ultra Fractal help: gradients and colouring settings

**URL:** <https://www.ultrafractal.com/help/gradients/gradients.html> · **Saved:** not saved; notes in `raw/research/fractal-palettes/ultra-fractal-gradients.md` · **Terms:** proprietary help text, paraphrased

## What it is

The Ultra Fractal manual pages on gradients (editor, files, transparency) and colouring settings (density, transfer, offset, repeat).

## What it covers

- A gradient has 400 entries and wraps at its ends; index 1.0 spans the whole gradient.
- Mapping order: index x Color Density, then transfer function (Linear, Sqr, Sqrt, Cube, CubeRoot, Log, Exp, Sin, ArcTan), then Gradient Offset (entries), then repeat (wrap) or clamp.
- `.ugr` files hold any number of entries `name { gradient: title=".." smooth=yes|no rotation=R index=N color=C ... opacity: ... }`.
- `color` is a decimal Windows COLORREF, `R + 256 G + 65536 B` (confirmed by JWildfire's reader and an independent generator).
- Control points may be unsorted (after rotation); `;` starts a comment line; `smooth` chooses spline or linear interpolation.

## Reliability

Primary for UF semantics. The meaning of `rotation` (assumed: index units) is not documented on the pages read.

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)

# Citations

- <https://www.ultrafractal.com/help/gradients/gradients.html>
