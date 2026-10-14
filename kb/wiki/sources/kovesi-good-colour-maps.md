---
title: "Kovesi: Good Colour Maps: How to Design Them"
type: source
authors: ["Peter Kovesi"]
year: 2015
raw: "web"
status: digested
tags: [palette, perceptual, cyclic, colour-maps]
---

# Kovesi: Good Colour Maps: How to Design Them

**URL:** <https://arxiv.org/abs/1509.03700> · **Saved:** not saved; notes in `raw/research/fractal-palettes/cyclic-and-mapping.md` · **Terms:** arXiv non-exclusive licence (paper); the CET map data is CC BY 4.0 (see [[sources/colorcet]])

## What it is

Paper on perceptually uniform colour map design with a test image for uniformity; introduces the CET maps.

## What it covers

- Lightness carries fine detail: colour discrimination collapses at high spatial frequency, so hue-only changes vanish in small features.
- The HSV hue circle has uneven perceptual contrast and false features at the light secondaries.
- Constant-lightness cyclic maps are dull (gamut-limited chroma) and show little detail.
- Vivid cyclic maps: zig-zag between two light and two dark colours, or a 'diamond' (one light, one dark, two mid colours far apart in hue); smooth every lightness reversal or it reads as an edge.

## Reliability

Widely cited design paper.

## Used by

[[techniques/palette-design]], [[synthesis/fractal-palettes]]
