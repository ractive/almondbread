---
title: "Ultra Fractal help: Anti-aliasing and drawing methods"
type: source
authors: ["Frederik Slijkerman"]
year: 2026
raw: "web"
status: digested
tags: [ultra-fractal, adaptive-sampling, anti-aliasing, guessing, implementations]
---

# Ultra Fractal help: Anti-aliasing and drawing methods

**URL:** <https://www.ultrafractal.com/help/exporting/antialiasing.html> · **Saved:** not saved · **Terms:** copyright Ultra Fractal (Frederik Slijkerman); commercial product documentation

## What it is

The Ultra Fractal online help pages on anti-aliasing for render-to-disk, the
fractal window's simpler anti-aliasing, the drawing method setting and final
rendering. Read together because they describe one policy.

## What it covers

- Render-to-disk adaptive AA: a pixel is anti-aliased only when the
  difference between it and its neighbours, summed over red + green + blue, is
  at least the threshold. Threshold 0 turns adaptivity off (slower). Use 0 when
  adaptive AA performs poorly, for example on images with many thin lines.
- Depth (>= 1, default 1): each extra level "can easily double or triple the
  calculation time". Subdivisions 9 (3x3, default) or 4 (2x2, for quick renders
  or depth > 1). A separate "Non-adaptive" option can look better on some
  images but is much slower.
- Fractal window AA (<http://www.ultrafractal.com/help/fractalwindows/antialiasing.html>):
  renders internally at twice width and height and downsamples. Render to disk
  is the better method.
- Drawing method (<https://www.ultrafractal.com/help/writing/reference/settings/method.html>):
  guessing (default), multipass linear, onepass linear.
- Rendering (<https://www.ultrafractal.com/help/exporting/renderingimages.html>):
  "Check the Force Linear drawing method option for the highest image
  quality"; it forces one-pass linear in every layer, so guessing is used only
  while editing.
- Periodicity checking setting 0–3, default 2 ("Normal")
  (<https://www.ultrafractal.com/help/writing/reference/settings/periodicity.html>).

## Reliability

Official product documentation, current. Describes behaviour, not algorithms:
the neighbour pattern for the AA test and how guessing works are not stated.

## Useful sections

- Anti-aliasing (render to disk): threshold, depth, subdivisions, non-adaptive.
- Rendering images: Force Linear for final quality.

## Used by

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md)
