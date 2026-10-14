---
title: "Ottosson: A perceptual color space for image processing (OKLab)"
type: source
authors: ["Björn Ottosson"]
year: 2020
raw: "web"
status: digested
tags: [colour-space, oklab, palette]
---

# Ottosson: A perceptual color space for image processing (OKLab)

**URL:** <https://bottosson.github.io/posts/oklab/> · **Saved:** not saved; notes in `raw/research/fractal-palettes/oklab-interpolation.md` · **Terms:** code public domain or MIT

## What it is

Blog post defining OKLab (and its polar form OKLCH): matrices from linear sRGB via LMS and a cube root.

## What it covers

- Gradients in OKLab keep chroma and even lightness; sRGB lerp greys out between complementary hues; HSV/HSL lightness swings.
- CSS Color 4 interpolates in OKLab/OKLCH and gamut-maps by chroma reduction, which almondbread follows for OKLCH stops.

## Reliability

Primary; widely adopted (CSS Color 4).

## Used by

[[techniques/palette-design]], [[synthesis/fractal-palettes]]
