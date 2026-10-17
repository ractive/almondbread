---
title: "MetalFX upscaling (temporal, spatial) for motion sample reuse"
type: platform
status: draft
sources:
  - "[[sources/apple-metalfx-docs]]"
  - "[[sources/apple-metalfx-taa-sample]]"
  - "[[sources/wwdc22-metalfx-upscaling]]"
  - "[[sources/wwdc25-metal4-games]]"
  - "[[sources/apple-metal-feature-set-tables]]"
tags: [metalfx, upscaling, temporal, reprojection, motion-vectors, jitter, apple-gpu, objc2, measured]
---

# MetalFX upscaling (temporal, spatial) for motion sample reuse

Can Apple's MetalFX replace or complement our own gesture-frame compose (jittered
reduced-resolution preview + exactly reprojected history,
[[techniques/frame-reuse-and-progressive]] FL)? This page covers the API, the conventions
(checked by experiment), availability, cost, and what MetalFX cannot know about fractal
content. Raw notes with all URLs and quotes:
`kb/raw/research/motion-sample-reuse/metalfx-notes.md`. Probe code:
`prototypes/metalfx-probe` (Rust, objc2-metal-fx).

## API summary

- **Spatial scaler** (`MTLFXSpatialScalerDescriptor` → `MTLFXSpatialScaler`): one colour
  texture in, one larger texture out; no history. `colorProcessingMode` is perceptual
  (default), linear (`[0,1]`) or HDR (src: [[sources/apple-metalfx-docs]] SDK header
  MTLFXSpatialScaler.h). Apple: it "will perform best if the input has been tone mapped and
  is in a perceptual color space" (src: [[sources/wwdc22-metalfx-upscaling]]).
- **Temporal scaler** (`MTLFXTemporalScalerDescriptor` → `MTLFXTemporalScaler`): needs a
  jittered colour texture, a depth texture and a motion texture each frame; MetalFX keeps
  the upscaled history itself (src: [[sources/wwdc22-metalfx-upscaling]]). The descriptor
  fixes formats and sizes (`colorTextureFormat`, `depthTextureFormat`,
  `motionTextureFormat`, `outputTextureFormat`, `inputWidth/Height`, `outputWidth/Height`).
  Per frame: set `colorTexture`, `depthTexture`, `motionTexture`, `outputTexture`,
  `inputContentWidth/Height`, `jitterOffsetX/Y`, `motionVectorScaleX/Y`, `reset`, then
  `encodeToCommandBuffer:` (src: [[sources/apple-metalfx-docs]] MTLFXTemporalScaler.h).
- **Texture rules**: each scaler reports the minimum `MTLTextureUsage` per input
  (`colorTextureUsage`, `depthTextureUsage`, `motionTextureUsage`, `outputTextureUsage`);
  the output texture must have private storage (src: [[sources/apple-metalfx-docs]]). On
  M1 Pro the temporal scaler asks for ShaderRead (0x1) on colour, depth and motion and
  ShaderRead | ShaderWrite | RenderTarget (0x7) on the output (measured 2026-10-14, M1 Pro,
  release, `metalfx-probe create`).
- **Exposure**: either `autoExposureEnabled` or a 1×1 R16Float `exposureTexture` whose R
  value multiplies the input colour; `preExposure` divides a pre-multiplied input (src:
  [[sources/apple-metalfx-docs]]). A "drastically wrong value … can lead to flickering and
  ghosting"; the value is only a hint and does not change output brightness (src:
  [[sources/wwdc25-metal4-games]]). We pass 1.0 (linear colours in `[0,1]`).
- **Reactive mask** (macOS 14.4 / iOS 17.4): per-pixel `[0,1]`, 1 = ignore history there
  (src: [[sources/apple-metalfx-docs]]). Apple: "Only use the reactive mask if going to higher
  input resolutions is not an option" (src: [[sources/wwdc25-metal4-games]]).
- **Dynamic resolution**: `inputContentPropertiesEnabled` with `inputContentMinScale` /
  `inputContentMaxScale`, then any `inputContentWidth/Height` inside the fixed-size input
  texture; the aspect ratio must not change (src: [[sources/apple-metalfx-docs]]). Apple
  advises not to allow more than 2× if not needed (src: [[sources/wwdc25-metal4-games]]).
  This matters for us: our preview ratio changes per frame (0.125–1 per axis), so a scaler
  would need dynamic resolution or one scaler per ratio.
- **Creation**: "make an instance … at launch or when a display changes resolutions"
  (src: [[sources/apple-metalfx-docs]]). `requiresSynchronousInitialization` (default false)
  returns fast and compiles a faster upscaler in the background, same image quality.
- **Newer (macOS/iOS 26)**: `MTLFXTemporalDenoisedScaler` (denoise + upscale for ray
  tracing; needs normals, albedos, roughness; Apple9+ GPUs) and `MTLFXFrameInterpolator`
  (generates an in-between frame from two frames + motion + depth + `deltaTime` + camera
  near/far/FOV) (src: [[sources/wwdc25-metal4-games]], [[sources/apple-metal-feature-set-tables]]).
  Neither fits: we have no normals/albedo, and interpolation adds a frame of latency to an
  interaction whose acceptance criterion is latency (inference). **OS 27** adds
  `isJitteredMotionVectorsEnabled` and `isOutputResolutionMotionVectorsEnabled`, and WWDC26
  announced a redesigned temporal upscaler on the Neural Engine for M5 Pro/Max (raw notes §2,
  §7; not in objc2-metal-fx 0.3.2).

## Conventions (verified)

Our probe's API takes a jitter `j ∈ [0,1)²` (sample position inside its input pixel, y down,
`(0.5, 0.5)` = centre) and motion `p_prev − p_cur` in input pixels. Verified by experiment
(measured 2026-10-14, M1 Pro, release; `metalfx-probe test`):

- **Jitter**: `jitterOffset = (0.5, 0.5) − j`, i.e. pixel centre minus sample position, in
  input pixels, y down. A sweep over the four sign combinations on a static scene (16 Halton
  frames, 320×200 → 640×400) gave RMSE 0.086 for `(−,−)`, 0.125 / 0.194 / 0.208 for the
  others, 0.137 with no jitter, 0.130 for a bilinear upscale; scaling the offset by 2 or 0.5
  was worse (0.124, 0.096). This matches WWDC22's example (sample at (0.625, 0.78) → offset
  (−0.125, −0.28)) (src: [[sources/wwdc22-metalfx-upscaling]]) and Apple's sample, which flips
  y because its projection is y up (src: [[sources/apple-metalfx-taa-sample]]).
- **Motion**: with `motionVectorScale = (1, 1)`, motion is in input pixels and points from the
  current position to the previous one: "an object that moves down and to the right … by 10
  pixels would be (-10,-10)" (src: [[sources/apple-metalfx-docs]]). Apple's sample instead
  stores UV-space vectors and sets the scale to the input size (src:
  [[sources/apple-metalfx-taa-sample]]). Our pan test (content moving 1.3, 0.7 output px per
  frame, 32 frames): correct motion 0.119, zero 0.242, negated 0.255, ×2 0.253, ×0.5 0.227.
- **Motion must be de-jittered** (camera motion only): "MetalFX expects dejittered motion
  vectors" (WWDC26 session 359, raw notes §7). Our affine motion is exact and jitter-free.
- **Depth**: required by the temporal scaler; we have none. A constant depth32Float texture
  works: 0.0, 0.5 and 1.0, with `depthReversed` true or false, gave bit-identical results.
  The probe uses 1.0, not reversed.
- **Affine motion for a zoom/pan** (pure scale + translation, both frames the same size):
  `motion(p) = (p − c)·(step − 1) + shift`, with `c` the image centre, `step = h_cur / h_prev`
  (complex units per pixel) and `shift` the view-centre move in previous-frame input pixels
  (`prototypes/metalfx-probe/src/lib.rs` `affine_motion`).

## Requirements and availability

- **OS**: temporal and spatial since macOS 13 / iOS 16; spatial also visionOS; temporal not
  on visionOS (src: [[sources/apple-metalfx-docs]]). iOS gained MetalFX support in practice
  with iOS 17, together with "up to 3X upscaling" (WWDC23, raw notes §5).
- **GPU**: spatial Apple3+, temporal **Apple7+ (A14, M1 and later)**, frame interpolation
  Apple5+, denoised upscaler Apple9+ (A17 Pro, M3) (src:
  [[sources/apple-metal-feature-set-tables]]). Intel Macs: contradictory (sample README says
  supported; forum reports nil temporal scalers); always gate on `supportsDevice:` (raw notes
  §8).
- **Scale range**: `supportedInputContentMinScale/MaxScale` = 1.0 / 3.0 on M1 Pro
  (measured 2026-10-14, M1 Pro, macOS 26.5). Godot hard-codes the same defaults
  (raw notes §8). `newTemporalScalerWithDevice` returns nil beyond 3× per axis (528×330 →
  1600×1000, ratio 3.03, fails; 533×333, ratio 3.00, works; 4× fails) and works at 1×
  (anti-aliasing only). The spatial scaler accepted 4× and 8× (measured 2026-10-14, M1 Pro,
  `metalfx-probe create limits`). Our preview ratios below 1/3 (0.125–0.33) are outside the
  temporal scaler's range.
- **From Rust**: `objc2-metal-fx = "0.3.2"` (generated from Xcode 26.0.1 SDK) with
  `objc2 0.6`, `objc2-metal 0.3.2`, `objc2-foundation 0.3.2` — the same generation as
  `almondbread-render`. It links MetalFX.framework itself; we only add CoreGraphics for
  `MTLCreateSystemDefaultDevice`. Every MetalFX call is `unsafe`; the scaler protocols'
  getters/setters come from the `MTLFXTemporalScalerBase` / `MTLFXSpatialScalerBase` traits.
  No Swift needed. `newTemporalScalerWithDevice_compiler` (Metal 4 path) also exists.

## GPU cost (measured)

GPU time of a command buffer that holds only the scaler encode, rgba16Float in/out,
rg16Float motion, depth32Float depth (measured 2026-10-14, M1 Pro, macOS 26.5, release,
`metalfx-probe bench`). The GPU was shared with other agents' builds and renders (load
average 25–47), so medians moved by 2–3× between runs; `min` was stable across three runs
(±15 %) and is the best estimate of the intrinsic cost. Median and p90 are from the quietest
run (200 frames × 3 rounds, configurations interleaved round robin, one frame committed and
waited at a time).

| Scaler | Input → output | Ratio/axis | min ms | median ms | p90 ms |
| --- | --- | --- | --- | --- | --- |
| temporal | 1067×667 → 1600×1000 | 1.5 | 1.3–1.5 | 2.8 | 5.3 |
| temporal | 800×500 → 1600×1000 | 2 | 1.0–1.1 | 2.0 | 3.9 |
| temporal | 533×333 → 1600×1000 | 3 | 0.75–0.90 | 1.8 | 4.3 |
| temporal | 2304×1440 → 3456×2160 | 1.5 | 6.3–6.6 | 11.4 | 14.7 |
| temporal | 1728×1080 → 3456×2160 | 2 | 4.7–4.9 | 9.2 | 12.6 |
| temporal | 1152×720 → 3456×2160 | 3 | 3.3–3.8 | 7.2 | 9.8 |
| spatial | 1067×667 → 1600×1000 | 1.5 | 0.40–0.43 | 2.1 | 4.3 |
| spatial | 800×500 → 1600×1000 | 2 | 0.39–0.41 | 1.1 | 3.8 |
| spatial | 533×333 → 1600×1000 | 3 | 0.38–0.42 | 0.5 | 3.7 |
| spatial | 2304×1440 → 3456×2160 | 1.5 | 1.7–1.8 | 2.1 | 5.0 |
| spatial | 1728×1080 → 3456×2160 | 2 | 1.7 | 3.3 | 5.5 |
| spatial | 1152×720 → 3456×2160 | 3 | 1.6–1.7 | 3.3 | 5.4 |

- The temporal cost scales with the output (~1 ms per Mpx output at 1/2 ratio, min) and
  falls with the input size; at full Retina 16" resolution it is 4.7–6.6 ms at best, which
  is a large part of a 120 Hz frame (8.3 ms) and of our gesture budget (half the refresh
  interval, [[techniques/frame-reuse-and-progressive]] FL). Every configuration was
  supported (temporal 1.5×, 2×, 3×; spatial all three).
- Committing 10 scaler command buffers back to back did not lower `min` (it is the same
  within 10 %) but raised the median under contention.
- A forum report measured ~10 ms on M1 Pro for 1904×1370 → 2856×2055 temporal, and 4 ms on
  M1 Max at low GPU clocks (raw notes §9); consistent with our numbers.
- **Memory** (`MTLDevice.currentAllocatedSize` delta across `newTemporalScalerWithDevice`):
  temporal 41–62 MB at 1600×1000 output, 175–278 MB at 3456×2160 (more for a larger
  input); spatial 12.6 MB and 57.9 MB. Our own input/output textures come on top
  (26–166 MB here, mostly the rgba16Float output and readback buffer).
- **Creation**: temporal with `requiresSynchronousInitialization = true` 170–600 ms (mostly
  180–360 ms), spatial ~1 ms (one outlier of 127 ms). In a fresh process an async temporal
  scaler took 177–200 ms to create versus 295 ms sync, but in a process that had already
  created ten scalers it took 2.7–3.6 s in three of three runs (unexplained; perhaps waiting
  on background compiles). Either way creation is far too slow for per-gesture or per-ratio
  creation; create at launch and on resize only.

## What MetalFX cannot know about our content

- **No depth.** MetalFX uses depth to tell foreground from background (src:
  [[sources/wwdc22-metalfx-upscaling]]); with a constant plane it only has colour statistics
  to reject stale history. In our motion model there is no occlusion at all, so this loses
  nothing structural, but it also means MetalFX cannot exploit the fact.
- **Our reprojection is exact; MetalFX resamples.** Our compose knows each pixel's history
  position exactly (`origin + pixel·step`) and samples with Catmull-Rom because bilinear blurs
  within a second ([[techniques/frame-reuse-and-progressive]] FL). MetalFX gets per-pixel
  motion at input resolution and does its own (undocumented) resampling and history
  rejection. On our synthetic high-frequency pattern, sub-pixel motion lost almost all of
  the static gain: after 32 frames, temporal vs bilinear-of-the-current-frame RMSE was
  0.111 vs 0.126 at 0.1 px/frame, 0.127 vs 0.119 at (0.5, 0.5), 0.119 vs 0.118 at
  (1.3, 0.7), but 0.064 vs 0.121 at exactly 1 input pixel per frame and 0.082 vs 0.123 at 2
  (measured 2026-10-14, M1 Pro, release). Zoom in at 3 %/frame: 0.049 vs 0.053. Fractal
  gestures are almost always fractional motion, so this is the key risk.
- **Sample positions.** We know the exact complex coordinate of every sample; MetalFX only
  knows the frame's single jitter offset. Per-pixel jitter or variable sample density
  (adaptive sampling, [[synthesis/dense-region-sampling]]) cannot be expressed.
- **Confidence.** Our history carries per-pixel confidence (alpha) and refinement fades in
  exact pixels; MetalFX history is opaque. Exact (fully iterated) pixels cannot be injected
  as "trusted"; the reactive mask can only make MetalFX trust history less.
- **Reactive regions.** Colour cycling, palette changes and depth tint change colours
  without motion; those would need `reset` or a reactive mask (inference).
- **Resolution ratio.** Fixed per scaler (or a dynamic range up to 3×); our preview ratio
  goes to 1/8 per axis on dense views.
- **Unresolved pixels / minibrot halos** look like noise or flat regions to MetalFX; nothing
  tells it they are placeholders (inference).

## Quality on fractal content

Measured on real flights with `prototypes/motion-reuse` (feature `metalfx`). Both MetalFX and
our FL compose got the same input: the jittered preview, shaded at the same ratio, with exact
affine motion and constant depth. The metrics are OKLab ΔE × 100 against a 16-sample converged
reference, plus temporal error, the change of the error from frame to frame along the motion
([[synthesis/motion-sample-reuse]]). Measured 2026-10-15, M1 Pro, release.

| flight | ratio/axis | our compose: ΔE / p99 / temporal | temporal scaler | spatial scaler |
|---|---|---|---|---|
| seahorse dive 1 → 1e9, 1280×720 | 1/2 | 1.66 / 13.8 / 0.79 | 1.61 / 14.5 / 0.81 | 3.28 / 25.6 / 4.33 |
| seahorse dive | 1/2.67 | 1.83 / 15.1 / 0.81 | 1.92 / 17.1 / 0.86 | 3.66 / 28.2 / 4.73 |
| WASD pan at 1e5, 960×540 | 1/2 | 1.06 / 10.5 / 0.59 | 1.14 / 11.0 / 0.62 | 2.74 / 23.0 / 3.66 |
| WASD pan at 1e5 | 1/2.67 | 1.19 / 11.8 / 0.60 | 1.51 / 14.5 / 0.66 | 3.12 / 26.1 / 3.98 |

- **Temporal scaler**: equal to our compose in mean ΔE on zooms and worse on every other
  measure. On pans, where every frame moves by a fractional number of pixels, it is worse on
  every metric (+8–27 % ΔE). Crops at 100 % look alike, both soft, at 2×. This matches the
  synthetic pan sweep above: with fractional motion the history adds little.
- **Spatial scaler**: grainy and aliased, with 2–3× the ΔE and 5–6× the temporal error of
  the bilinear preview path. Unusable for this content.
- **The sample cache (since deleted, 2026-10-17) did better at FL's own iteration budget** (`hy-b1`): ΔE 1.45 on the
  seahorse dive and 0.72 on the pan.

Verdict: MetalFX does not pay for almondbread's fractal content. It gives no quality gain over
our compose, cannot go below 1/3 resolution, costs 5–7 ms at Retina resolution, and needs
A14+ on iOS. [[synthesis/motion-sample-reuse]] has the recommendation.

## Pitfalls

- `requiresSynchronousInitialization = false` is not reliably fast: 0.2 s in a fresh
  process, 2.7–3.6 s after other scalers were created in the same process (see GPU cost).
- Under heavy GPU contention one scaler command buffer failed with
  `kIOGPUCommandBufferCallbackErrorImpactingInteractivity` (the system's interactivity
  watchdog); an app must treat a failed upscale as a dropped frame, not a fatal error
  (measured 2026-10-14, M1 Pro).
- The scaler's `inputContentMinScale/MaxScale` read back as the fixed ratio when dynamic
  resolution is off (1.5/1.5, 2/2, 3/3).
- `jitterOffset` is centre minus sample, not sample minus centre; the wrong sign still
  "works" but is blurrier than no jitter (RMSE 0.208 vs 0.137).
- GPU time from a command buffer that is committed and waited on alone runs at low GPU
  clocks; a forum report saw ~400 MHz and 4 ms on M1 Max for 768² → 1536² (raw notes §9).

## Related

[[techniques/frame-reuse-and-progressive]], [[techniques/anti-aliasing]],
[[platform/render-architecture]], [[platform/apple-gpu-architecture]],
[[platform/ios-ipad-considerations]], [[synthesis/dense-region-sampling]]
