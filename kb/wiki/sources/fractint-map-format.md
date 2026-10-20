---
title: "Fractint documentation: palette maps (.map)"
type: source
authors: ["Stone Soup Group"]
year: 1999
raw: "web"
status: digested
tags: [palette, file-format, fractint]
---

# Fractint documentation: palette maps (.map)

**URL:** <https://github.com/LegalizeAdulthood/fractint-legacy> · **Saved:** not saved; notes in `raw/research/fractal-palettes/fractint-map.md` · **Terms:** Fractint source free for non-commercial use with credit; bundled maps not to be shipped; the format is free to implement

## What it is

fractint.doc §3.2 and the loader loadmap.c of Fractint, plus the sample maps shipped with it.

## What it covers

- One `R G B` triplet per line (0..255), line N = palette index N, up to 256 lines, no header.
- Text after the third number is ignored: that is how comments work (usually on line 1).
- Short files are padded with grey; values are reduced to the 6-bit VGA DAC (a modern reader keeps 8 bits).
- Entry 0 is black in the bundled maps; colour cycling leaves it alone (unverified, from memory of Fractint behaviour).
- UF, ChaosPro, JWildfire and gfract read `.map`; KF and XaoS do not.

## Reliability

Primary (the program's own doc and loader).

## Used by

[Palette design for fractals](../techniques/palette-design.md), [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md)
