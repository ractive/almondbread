---
title: "Apple sample: Applying temporal antialiasing and upscaling using MetalFX"
type: source
authors: ["Apple"]
year: 2022
raw: "web"
status: digested
tags: [metalfx, upscaling, jitter, motion-vectors, sample-code]
---

# Apple sample: Applying temporal antialiasing and upscaling using MetalFX

**URL:** <https://developer.apple.com/documentation/metalfx/applying-temporal-antialiasing-and-upscaling-using-metalfx> · **Saved:** not saved (`raw: "web"`); quotes in `kb/raw/research/motion-sample-reuse/metalfx-notes.md` · **Terms:** Apple developer content

## What it is

Apple's Swift/Metal sample app for the temporal and spatial scalers (zip from docs-assets.developer.apple.com). Read for its exact jitter and motion vector formulas.

## What it covers

- Jitter: Halton(2,3) minus 0.5, 32-frame cycle; projection offset `-2 * jitter / size`; y flipped for the scaler because the projection is y up; `jitterOffset` = the flipped pixel jitter.
- Motion: de-jittered UV difference `prevUV - uv` in rg16Float with `motionVectorScale` = input size.
- Formats (bgra8Unorm_srgb colour, depth32Float, rg16Float motion), reset on first frame, scaler recreated on render-scale change, mip bias -2 for temporal.
- README: macOS 13 with M1 or Intel Mac; iOS 16 with an M1 iPad.

## Reliability

Primary, runnable vendor code; the formulas are the de facto convention. Apple sample code licence (not copyleft); we describe it, we do not copy it.

## Useful sections

`AAPLRenderer.swift` (updateJitterOffsets), `Shaders.metal` (motion vectors).

## Used by

- [MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md)

# Citations

- <https://developer.apple.com/documentation/metalfx/applying-temporal-antialiasing-and-upscaling-using-metalfx>
