---
title: "Exponential map (log-polar) rendering"
type: technique
status: draft
sources: ["[[sources/kf-manual]]", "[[sources/zoomasm]]", "[[sources/fraktaler-3-site]]", "[[sources/wikibooks-fraktaler-3]]"]
tags: [exponential-map, video, zoomasm, log-polar]
---

# Exponential map (log-polar) rendering

A zoom video spends most of its frames re-rendering detail that was already
visible at a larger scale. The exponential map renders the fractal once as
exponentially spaced rings around the zoom centre, stored as wide strips, and
reprojects them into any number of flat frames on the GPU
(src: [zoomasm (zoom video assembler)](../sources/zoomasm.md) §zoomasm).

## Practice

- **KF2+** added the exponential map coordinate transform in 2.15.1. For more
  conformal results use a wide aspect ratio: 9:1 works well (window 1152×128 or
  1536×170). Store zoom-out sequences with zoom size 2 and combine them in
  zoomasm; "much more efficient than storing flat keyframes". 2.15.1.3 fixed
  OpenCL exponential-map DE scaling, which caused visible rings in zoomasm
  output (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §Exponential Map).
- **Fraktaler 3**: exponential map is in the Transform window
  (`transform.exponential_map`); for zoomasm use zoom-out factor 2, a very wide
  aspect ratio, raw data and 1 subframe; a vertical flip option suits other
  assemblers. "Not currently very usable in the user interface"
  (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)). The CLI renders zoom-out sequences
  (src: [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)).
- **zoomasm** keyframes are EXR strips with N (split N0/N1/NF), T (phase),
  DEX/DEY, RGB, interior and glitch flags; DE is rescaled to screen space.
  Recommended strips: 1920×1080 output from 6912×768 strips; 3840×2160 from
  12288×1360 (window 1536×170). Keep 16 layers in video memory for 2D
  projection, 25 or more for 360° projection; raise it if small circles appear
  near the zoom focus. The bottom edge of a strip should be a uniform colour
  (usually interior). Colouring is a GLSL `colour()` function; samples per
  pixel and shutter speed are configurable (src: [zoomasm (zoom video assembler)](../sources/zoomasm.md)).

## Geometry (inference, standard derivation)

Strip of width `W` (angle) and height `H` (log-radius), zoom centre `c₀`,
outer radius `r₀`. Pixel `(u, v)`, `v = 0` at the outer edge:

```text
theta = 2*pi * u / W
rho   = -2*pi * v / W            // log-radius step equals angle step: square pixels
c     = c0 + r0 * exp(rho) * (cos(theta), sin(theta))
```

Equal steps in angle and log-radius make each pixel locally square
(conformal). A `W×H` strip spans a zoom factor `exp(2π·H/W)`; for 9:1 that is
`exp(2π/9) ≈ 2.01`, matching KF's "zoom size 2" per keyframe. The
`6912×768` strip is also 9:1.

Reprojection for output frame at zoom `r` (inference): for an output pixel at
offset `d` from the centre, `log|d·r|` selects the keyframe and row, `arg d`
the column; blend between neighbouring keyframes.

Perturbation fits naturally: one reference at `c₀` serves every pixel of every
strip; pixel offsets `c − c₀` span many decades within one strip, so the
inner rows need deltas far smaller than the outer rows. That calls for the
extended-range delta types of [Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md) even when
the outer edge is shallow (inference). DE must be computed in the strip's
local pixel scale (`r0·exp(rho)·2π/W`), the bug KF fixed in 2.15.1.3
(inference from [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

## Uses beyond video (inference)

- Instant zoom preview: one strip covering the current zoom range gives a
  continuous zoom animation with no per-frame fractal work.
- Overview of all scales around a point in one image.

## For almondbread

Later feature (roadmap extras): an export mode that renders 9:1 strips (zoom
factor 2 each) around the reference, writing (n, frac, phase, DE, flags), plus
a Metal reprojection pass. Compatibility with zoomasm's EXR layout is cheap to
add and lets users assemble videos with existing tools. Related:
[Frame reuse and progressive refinement](frame-reuse-and-progressive.md), [Smooth (continuous) iteration count](smooth-coloring.md).

# Citations

- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [zoomasm (zoom video assembler)](../sources/zoomasm.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)
