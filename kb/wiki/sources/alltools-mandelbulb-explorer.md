---
title: "AllTools: Mandelbulb 3D Explorer (WebGL)"
type: source
authors: ["AllTools"]
year: 2026
raw: "web"
status: skimmed
tags: [3d, mandelbulb, float32, ray-marching, precision]
---

# AllTools: Mandelbulb 3D Explorer (WebGL)

**URL:** <https://alltools.dev/tools/visualizations/mandelbulb-3d-explorer/> · known from a search
summary on 2026-10-19, not read in full

## What it covers

- A float32 WebGL ray-marched Mandelbulb whose notes say pixel-grid artefacts and false
  structure appear by about 1e4–1e5 zoom, because the sphere tracer can no longer decide
  reliably whether it has hit the surface.
- Claims CPU renderers with double precision "plus perturbation" go much deeper (no evidence
  given; we found no published 3D perturbation scheme).

## Reliability

Low: an anonymous tool page. The 1e4–1e5 float32 figure agrees with our own estimate in
[Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md); the perturbation claim is unsupported.

## Used by

[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
