---
title: "Apple Metal Feature Set Tables (May 2026)"
type: source
authors: ["Apple"]
year: 2026
raw: "web"
status: digested
tags: [metal, apple-gpu, gpu-families, metalfx]
---

# Apple Metal Feature Set Tables (May 2026)

**URL:** <https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf> · **Saved:** not saved (`raw: "web"`); quotes in `kb/raw/research/motion-sample-reuse/metalfx-notes.md` · **Terms:** Apple developer content

## What it is

Apple's PDF listing Metal features per GPU family, with the family of each Apple chip.

## What it covers

- MetalFX rows: spatial upscaling Apple3+, temporal upscaling Apple7+, frame interpolation Apple5+, denoised upscaling Apple9+.
- Families: A14/M1 = Apple7; A15/A16/M2 = Apple8; A17 Pro/A18/M3/M4 = Apple9; A19/M5 = Apple10.
- Older families (edition of October 23, 2026): A10 = Apple3, A11 = Apple4, A12 = Apple5, A13 = Apple6.
- Compute rows (same edition): SIMD-scoped reduction operations Apple7+, SIMD-scoped permute operations Apple6+, nonuniform threadgroup size Apple4+.

## Reliability

Primary vendor reference, read as dated May 21, 2026 and again as October 23, 2026. Does not cover Intel/AMD Macs.

## Useful sections

MetalFX rows, the feature availability table and the GPU family table.

## Used by

- [iOS and iPadOS on a device](../platform/ios-device.md) "GPU floor"

- [MetalFX upscaling (temporal, spatial) for motion sample reuse](../platform/metalfx.md)

# Citations

- <https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf>
