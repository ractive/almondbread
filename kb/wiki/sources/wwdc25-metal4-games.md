---
title: "WWDC25 session 211: Go further with Metal 4 games"
type: source
authors: ["Apple"]
year: 2025
raw: "web"
status: digested
tags: [metalfx, frame-interpolation, denoiser, wwdc, metal4]
---

# WWDC25 session 211: Go further with Metal 4 games

**URL:** <https://developer.apple.com/videos/play/wwdc2025/211/> · **Saved:** not saved (`raw: "web"`); quotes in `kb/raw/research/motion-sample-reuse/metalfx-notes.md` · **Terms:** Apple developer content

## What it is

Apple session on MetalFX in the Metal 4 generation: dynamic-resolution upscaling, exposure debugging, reactive mask advice, frame interpolation and the denoised upscaler.

## What it covers

- Dynamic input sizes; "should not set the maximum scale to higher than 2x scaling if it is not needed".
- Exposure is a hint; a wrong value causes flicker and ghosting; `MTLFX_EXPOSURE_TOOL_ENABLED` debug overlay.
- Reactive mask only if higher input resolution is not an option.
- Frame interpolation (two frames + motion + depth + delta time + camera parameters, UI handling, at least 30 fps before interpolation); denoised upscaler inputs (normals, albedos, roughness).

## Reliability

Primary vendor talk; advice-level, no measured costs.

## Useful sections

Upscaler and frame interpolation chapters.

## Used by

- [MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md)
