---
title: "Apple docs and SDK headers: MetalFX framework"
type: source
authors: ["Apple"]
year: 2026
raw: "web"
status: digested
tags: [metalfx, upscaling, metal, apple-docs]
---

# Apple docs and SDK headers: MetalFX framework

**URL:** <https://developer.apple.com/documentation/metalfx> · **Saved:** not saved (`raw: "web"`); quotes in `kb/raw/research/motion-sample-reuse/metalfx-notes.md` · **Terms:** Apple developer content

## What it is

Apple's MetalFX reference (MTLFXTemporalScaler, MTLFXSpatialScaler, their descriptors and base protocols, MTLFXTemporalDenoisedScaler, MTLFXFrameInterpolator), read through the JSON endpoint `https://developer.apple.com/tutorials/data/documentation/metalfx/<path>.json`, plus the doc comments in the Xcode 26.5 SDK headers (`MetalFX.framework/Headers/MTLFX*.h`), which carry the same text.

## What it covers

- Descriptor and per-frame properties of both scalers, texture usage and storage rules (output must be private), exposure texture (1x1 R16Float), reactive mask (macOS 14.4 / iOS 17.4), dynamic resolution (`inputContentPropertiesEnabled`), `requiresSynchronousInitialization`.
- Conventions: `jitterOffsetX/Y` ("the subpixel sampling coordinate ... pixel offset this scaler samples to return to the frame's reference frame") and `motionVectorScaleX/Y` (scale 1 = pixel units, pointing to the previous position; down-right motion of 10 px is (-10,-10)).
- Availability: macOS 13 / iOS 16 for both scalers; macOS/iOS 26 for the denoiser and frame interpolator; OS 27 adds jittered and output-resolution motion vector flags (web docs only).

## Reliability

Primary vendor documentation. The doc comments are terse: the jitter sign is ambiguous in prose and was settled by experiment ([[platform/metalfx]]).

## Useful sections

Header doc comments of `MTLFXTemporalScalerBase` (jitter, motion, reset, exposure); framework overview page.

## Used by

- [[platform/metalfx]]
