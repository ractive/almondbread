---
title: "zoomasm (zoom video assembler)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2021
raw: "web/zoomasm.html"
status: digested
tags: [exponential-map, zoom-video, exr, coloring-methods, frame-reuse]
---

# zoomasm (zoom video assembler)

**URL:** <https://mathr.co.uk/zoomasm/> · **Saved:** `raw/web/zoomasm.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Manual of zoomasm 3.4 (2025-06-11; manifest year 2021 is the 3.0 release), a GPU (OpenGL) tool that
assembles zoom videos from exponential-map EXR keyframes rendered by KF2+ or Fraktaler 3, with GLSL
colouring on the raw iteration data. AGPL-3.0+.

## What it covers

- Core idea: render exponentially spaced rings around the zoom centre once (exponential-map keyframes), then reproject to flat video frames; much cheaper than rendering every flat frame.
- KF recipe: KF 2.15.1+ for exponential map export; enable derivatives (analytic DE and slopes) and jitter (avoids moiré); window 1152x128 (9:1). Keyframe sizes per output: 1280x720 -> 4608x512, 1920x1080 -> 6912x768, 3840x2160 -> 12288x1360; 360 output 4096x2048 -> 4096x448. Bottom edge of the keyframe should be uniform (usually interior colour).
- GPU memory: 16 keyframe layers enough for 2D projection, 25+ for 360 (more layers if small circles appear near the focus). 8 GB VRAM recommended, works with 512 MB at low resolution.
- Keyframes in zoom-out order need "Reverse"; "Flip" for vertical orientation of other software.
- Colouring: GLSL `vec3 colour(void)` returning linear RGB; `getInterior()`, `getGlitch()`, `getN()` (smooth iteration count as `float49`, more than float's 24 bits, manipulated with `add()`, `sub()`, `to_float()`), raw `getN0/N1/NF` biased by `IterationsBias`, `getT()` phase, `getDE()` (rescaled to screen space for any projection), `getRGB()`, `getCoord()`, `KF_Colour()` emulating KF palettes.
- Samples per pixel reduce sparkle in the centre; shutter speed gives motion blur.
- Projections: 2D (central focus, rotation) and 360 equirectangular.
- Timeline waypoints with monotone cubic interpolation of zoom depth; speed shown in 2x and 10x zooms per second.
- Rendering keyframes is "often the slow part"; output via FFmpeg.

## Reliability

Primary documentation by the author. Practical, not theoretical; most relevant for the
exponential-map pipeline and the float49 iteration-count idea, not for the iteration kernel.

## Useful sections

- §zoomasm — the exponential-ring rationale.
- §Using Kalle's Fraktaler 2 + — keyframe sizes, derivatives, jitter, 9:1 aspect.
- §Input — layers, reverse, flip, VRAM estimate.
- §Colour — presets, samples per pixel, projections.
- §GLSL API — accessor list; §getN() — float49 iteration count.
- §Releases — version dates.

## Used by

[[techniques/exponential-map]], [[techniques/frame-reuse-and-progressive]], [[techniques/coloring-methods]],
[[techniques/smooth-coloring]], [[techniques/anti-aliasing]], [[implementations/kalles-fraktaler-2]],
[[implementations/fraktaler-3]]
