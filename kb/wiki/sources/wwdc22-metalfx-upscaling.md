---
title: "WWDC22 session 10103: Boost performance with MetalFX Upscaling"
type: source
authors: ["Apple"]
year: 2022
raw: "web"
status: digested
tags: [metalfx, upscaling, wwdc, jitter, mip-bias]
---

# WWDC22 session 10103: Boost performance with MetalFX Upscaling

**URL:** <https://developer.apple.com/videos/play/wwdc2022/10103/> · **Saved:** not saved (`raw: "web"`); quotes in `kb/raw/research/motion-sample-reuse/metalfx-notes.md` · **Terms:** Apple developer content

## What it is

Apple's introduction of MetalFX: spatial vs temporal upscaling, inputs, jitter, motion vectors, mip bias, reset, exposure.

## What it covers

- Temporal needs jittered colour, motion and depth; MetalFX keeps the history.
- Jitter offsets in [-0.5, 0.5]; worked example: sample at (0.625, 0.78) gives offset (-0.125, -0.28), i.e. centre minus sample. Halton(2,3) with 32 jitters recommended for 2x; "eight jittered samples per output pixel".
- Motion in render-resolution pixels, from the current to the previous position.
- Mip bias advice and its flicker/moire caveat on high-frequency textures; reset on scene cuts; spatial prefers tone-mapped perceptual input.

## Reliability

Primary vendor talk. One sentence on the motion scale (NDC-style "(-960, 540)") disagrees with the docs and sample; the docs win.

## Useful sections

Jitter section (worked example), motion vector section.

## Used by

- [[platform/metalfx]]
