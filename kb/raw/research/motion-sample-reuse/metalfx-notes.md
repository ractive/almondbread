# MetalFX research notes (raw, R4 motion sample reuse), 2026-10-14

Raw notes, not edited after writing. Sources: Apple docs read through the JSON endpoint
`https://developer.apple.com/tutorials/data/documentation/metalfx/<path>.json` ("DOC: x" =
`https://developer.apple.com/documentation/metalfx/x`); the Xcode 26.5 SDK headers
(`/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.5.sdk/System/Library/Frameworks/MetalFX.framework/Headers/`,
"HDR: file"); the Apple sample code zip; WWDC session pages; forums. "[inference]" marks our
own reasoning. Measurements of our own are in prototypes/metalfx-probe (separate section at the end).

## 1. SDK headers (Xcode 26.5, macOS 26.5 SDK)

Header files: MetalFX.h, MTLFXSpatialScaler.h, MTLFXTemporalScaler.h,
MTLFXTemporalDenoisedScaler.h, MTLFXFrameInterpolator.h, MTL4FXSpatialScaler.h,
MTL4FXTemporalScaler.h, MTL4FXTemporalDenoisedScaler.h, MTL4FXFrameInterpolator.h, MTLFXDefines.h.

HDR: MTLFXTemporalScaler.h
- `MTLFXTemporalScalerDescriptor` API_AVAILABLE(macos(13.0), ios(16.0)), unavailable visionOS.
- Descriptor properties: colorTextureFormat, depthTextureFormat, motionTextureFormat,
  outputTextureFormat, inputWidth/Height, outputWidth/Height, autoExposureEnabled (default NO;
  "Temporal scaler instances that use auto exposure ignore their exposureTexture"),
  requiresSynchronousInitialization (default NO: "quickly create and return ... then compile a
  faster upscaler in the background ... the effect can take more time to upscale textures while
  the framework compiles"; "The image quality of the effect's output texture is consistent"),
  inputContentPropertiesEnabled ("uses dynamic resolution ... inputContentMinScale and
  inputContentMaxScale represent the input and output resolution both the width and height";
  "The scaler assumes that aspect ratio of the input and output textures doesn't change"),
  inputContentMinScale, inputContentMaxScale, reactiveMaskTextureEnabled and
  reactiveMaskTextureFormat API_AVAILABLE(macos(14.4), ios(17.4)).
- `newTemporalScalerWithDevice:`; `newTemporalScalerWithDevice:compiler:` (MTL4, macos 26 / ios 26).
- `+supportedInputContentMinScaleForDevice:` / `MaxScale` API_AVAILABLE(macos(14.0), ios(17.0)):
  "Returns the smallest/largest temporal scaling factor the device supports".
- `+supportsDevice:`; `+supportsMetal4FX:` (macos 26 / ios 26).
- `MTLFXTemporalScalerBase` (conforms to `MTLFXFrameInterpolatableScaler`, so a temporal scaler can
  be handed to the frame interpolator's `scaler` property):
  - colorTextureUsage, depthTextureUsage, motionTextureUsage, reactiveTextureUsage,
    outputTextureUsage: "minimal texture usage options that your app's ... texture needs".
  - inputContentWidth/Height: "region within the color texture the scaler uses as its input".
  - colorTexture, depthTexture, motionTexture, outputTexture ("You are responsible for providing a
    texture with a private storageMode to this property").
  - exposureTexture: "Create and assign a 1x1 MTLPixelFormatR16Float texture ... MetalFX reads the
    R channel of the texel at position (0,0) ... uses this value to multiply the input color."
  - reactiveMaskTexture: "0.0 tells MetalFX to follow its normal behavior ... 1.0 tells MetalFX to
    ignore temporal history for the corresponding pixel ... (0.0, 1.0) proportionally blends".
  - preExposure: "If the input color texture ... is pre-multiplied by fixed value, set this
    property to that same fixed value so MetalFX divides input color by it."
  - jitterOffsetX/Y: "The horizontal component of the subpixel sampling coordinate you use to
    generate the color texture input. This property indicates the horizontal pixel offset this
    scaler samples to return to the frame's reference frame."
  - motionVectorScaleX/Y: "The scaler converts the horizontal component of each value in
    motionTexture into fragment (pixel) coordinates by multiplying it by this property's value.
    If you set this property's value to 1.0, this temporal scaler expects that each pixel's motion
    vector points to that pixel's location in the colorTexture at the time of the last call to
    encode this scaler's work. For example, in Metal's standard device coordinates, where (0,0)
    represents the upper-left corner of the framebuffer, the motion vectors for an object that
    moves down and to the right in the colorTexture by 10 pixels would be (-10,-10)."
  - reset: "whether the temporal scaler discards historical data from previous frames."
  - depthReversed: "whether the depth texture uses zero to represent the farthest distance."
  - read-only formats/sizes, inputContentMinScale/MaxScale, fence.
- `MTLFXTemporalScaler`: `encodeToCommandBuffer:`.

HDR: MTLFXSpatialScaler.h
- `MTLFXSpatialScalerColorProcessingMode`: Perceptual = 0 ("sRGB"), Linear = 1 ("linear color
  space in the [0,1] range"), HDR = 2 ("beyond the [0,1] range ... reversible tone mapping").
- Descriptor: color/output formats, input/output sizes, colorProcessingMode (default
  Perceptual), newSpatialScalerWithDevice:, supportsDevice:, supportsMetal4FX:. macos 13 / ios 16.
- Base: colorTextureUsage, outputTextureUsage, inputContentWidth/Height, colorTexture,
  outputTexture (private storage), fence.

HDR: MTLFXTemporalDenoisedScaler.h
- Descriptor API_AVAILABLE(macos(26.0), ios(18.0)) [sic in header; base protocol says ios 26].
- Inputs: color, depth, motion, diffuseAlbedo, specularAlbedo, normal, roughness, optional
  specularHitDistance, denoiseStrengthMask, transparencyOverlay, reactive mask; "You typically use
  this in situations where your pixels are expensive to produce, such as elaborate ray or path
  tracing".

HDR: MTLFXFrameInterpolator.h (macos 26 / ios 26)
- Inputs: colorTexture, prevColorTexture, depthTexture, motionTexture, motionVectorScaleX/Y (same
  convention text as the temporal scaler, relative to prevColorTexture), deltaTime ("between time
  of current and previous frame"), nearPlane, farPlane, fieldOfView ("vertical field of view
  angle, in degrees"), aspectRatio, uiTexture, jitterOffsetX/Y, shouldResetHistory, outputTexture
  (private), depthReversed ("default value is true"), optional `scaler`
  (id<MTLFXFrameInterpolatableScaler>).
- "generates an extra frame at a fraction of the cost" between two rendered frames.

## 2. Apple docs (web)

- DOC: metalfx (https://developer.apple.com/documentation/metalfx): "If you can provide pixel
  color, depth, and motion information, add an MTLFXTemporalScaler instance... Otherwise, add an
  MTLFXSpatialScaler instance, which only requires a pixel color input texture." "Because the
  scaling effects take time to initialize, make an instance of either effect at launch or when a
  display changes resolutions."
- Availability on the web docs: temporal/spatial iOS/iPadOS/Mac Catalyst 16.0, macOS 13.0, now
  also tvOS 27.1; spatial also visionOS 1.0 (DOC: mtlfxtemporalscalerdescriptor,
  mtlfxspatialscalerdescriptor).
- DOC: mtlfxtemporalscalerbase: "doesn't track that you assign the same texture instances ... the
  only requirement is that you provide textures that match the pixel formats and dimensions".
  Also lists per-texture `*ContentOffsetX/Y`, `outputOffsetX/Y` (not in the 26.5 SDK headers we
  have; presumably OS 27).
- New in OS 27 (DOC: mtlfxtemporalscalerdescriptor/isjitteredmotionvectorsenabled,
  .../isoutputresolutionmotionvectorsenabled), iOS/macOS 27.0:
  - isJitteredMotionVectorsEnabled: "When you set this property to true, the scaler internally
    subtracts the jitter from the motion vectors using the jitter offset values provided each
    frame... When false (the default), the scaler uses the motion vectors directly."
  - isOutputResolutionMotionVectorsEnabled: "Set this property to true when your app provides
    motion vectors at the output resolution... the scaler interprets the motionTexture dimensions
    to match outputWidth and outputHeight."

## 3. Apple sample "Applying temporal antialiasing and upscaling using MetalFX"

URL: https://developer.apple.com/documentation/metalfx/applying-temporal-antialiasing-and-upscaling-using-metalfx
(zip: https://docs-assets.developer.apple.com/published/cd4b8936af25/ApplyingTemporalAntialiasingAndUpscalingUsingMetalFX.zip)
- README: "macOS 13 or later, and a Mac with the M1 chip or an Intel-based Mac; iOS 16 or
  later, and an iPad with the M1 chip."
- Jitter (AAPLRenderer.swift updateJitterOffsets): index = frame % 32 + 1;
  pixelJitter = (halton(i,2) - 0.5, halton(i,3) - 0.5); ndcJitter = -2 * pixelJitter /
  renderResolution ("The sign of the jitter flips because the translation has the opposite
  effect"), added to the projection matrix; then pixelJitter.y *= -1 ("Flip the y-coordinate
  direction because the bottom left is the origin of a texture"); scaler.jitterOffsetX/Y =
  pixelJitter. On reset jitter = 0.
- [inference, by the research agent] from the shear sign: jitterOffset = pixel centre - sample
  position, input pixels, y down.
- Motion vectors (Shaders.metal): uv and prevUV in 0..1 UV, top-left origin, both de-jittered;
  motion = prevUV - uv, rg16Float; motionVectorScale = render (input) size, positive.
- Formats: color bgra8Unorm_srgb, depth depth32Float, motion rg16Float. reset on first frame or
  user toggle; render-scale change recreates the scaler; mip bias -2 for temporal ("The expected
  spatial upscaler mipmap bias is about one unit higher than with the temporal upscaler").

## 4. WWDC22 10103 "Boost performance with MetalFX Upscaling"

https://developer.apple.com/videos/play/wwdc2022/10103/
- Temporal "requires a color input that is jittered, as well as motion and depth data."
  "MetalFX keeps track of the upscaled output, so you only need to pass the color, motion, and
  depth from the current rendered frame." Depth "indicates what's in the foreground and what's in
  the background" (no more detail).
- "Usually, eight jittered samples per output pixel produces a high-quality anti-aliased upscaled
  output. In the case of 2x upscaling, the recommendation is to use a Halton (2,3) sequence with
  32 jitters." "jitter offsets are always in the range of -0.5 to 0.5." Example: sample at
  (0.625, 0.78), centre (0.5, 0.5), "the jitter offset is (-0.125, -0.28)" (offset = centre -
  sample). Verify jitter by rendering "a scene without camera and object motion using a sequence
  of different jitter offsets".
- "MetalFX expects motion data in the render resolution pixel space with direction that goes
  from the current frame's position to the previous frame's position." (The talk's prose also
  mentions a scale "(-960, 540)" for 1080p NDC-style vectors, which differs from the sample.)
- Mip bias: temporal log2(render/target) - 1; spatial log2(render/target); "-2 generates
  flickering and moire effects" on high-frequency textures.
- Reset "when your application loads the first frame or when there is a scene cut." Temporal
  goes "before any post processing effects." Spatial "will perform best if the input has been
  tone mapped and is in a perceptual color space." Creating scalers is "expensive".
- No hardware list or ms costs in the talk. Demo: 1080p -> 4K.

## 5. WWDC23 / WWDC24

- WWDC23 10125 "Bring your game to Mac, Part 3: Render with Metal"
  (https://developer.apple.com/videos/play/wwdc2023/10125/): "New features this year include
  support for iOS, up to 3X upscaling, and support in Metal-cpp." "Temporal upscaling requires
  jitter sequence and motion vectors." Exposure texture or autoexposure. "Don't forget to reset
  the history on camera cuts and extreme camera movements."
- WWDC24 10089 "Port advanced games to Apple platforms"
  (https://developer.apple.com/videos/play/wwdc2024/10089/): "you only need to specify your
  textures for color, depth, motion and upscaled output and also provide jitter offsets. This
  year, we are adding the new reactive mask ... may help ... fast-moving objects with inaccurate
  motion information, which may happen when using alpha blending."

## 6. WWDC25 211 "Go further with Metal 4 games"

https://developer.apple.com/videos/play/wwdc2025/211/
- "The MetalFX temporal upscaler now supports dynamically sized inputs ... your game should not
  set the maximum scale to higher than 2x scaling if it is not needed."
- Exposure: "a drastically wrong value ... can lead to flickering and ghosting"; it is "just a
  hint ... doesn't change the brightness of the output"; debug: env MTLFX_EXPOSURE_TOOL_ENABLED
  ("When your exposure value is correct the grid pattern is a constant mid gray").
- "Only use the reactive mask if going to higher input resolutions is not an option."
- Frame interpolation "to all Apple platforms": two frames + motion + depth, UI modes, "get the
  delta time and camera parameters right", "Try to have at minimum 30 frames per second before
  interpolation."
- Denoised upscaler: "a super set of the upscaler API"; needs normals, diffuse albedo, roughness,
  specular albedo; "no need to tune parameters".

## 7. WWDC26

- https://developer.apple.com/wwdc26/guides/metal/: "A redesigned temporal upscaler harnesses
  both the Neural Engine and Neural Accelerators on M5 Pro and M5 Max. MetalFX can now
  reconstruct fine detail from significantly lower render resolutions." Also "Process
  subrectangles for dynamic resolution, pass motion vectors to save preprocessing time, and
  handle postprocess effects with distortion fields."
- Session 359 "Build real-time neural rendering pipelines with Metal"
  (https://developer.apple.com/videos/play/wwdc2026/359/): "MetalFX expects dejittered motion
  vectors, meaning without the sub-pixel shifts. Without this, MetalFX might receive motion
  vectors that might be up to one pixel wrong, creating edge shimmering."
- No dedicated WWDC26 MetalFX session found; new max scale values not found.

## 8. Device support and scale limits

- Metal Feature Set Tables (https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf,
  May 21, 2026): spatial upscaling Apple3+, temporal upscaling Apple7+, frame interpolation
  Apple5+, denoised upscaling Apple9+. Families: A14/M1 = Apple7; A15/A16/M2 = Apple8;
  A17 Pro/A18/M3/M4 = Apple9; A19/M5 = Apple10. So temporal needs A14/M1 or newer, the denoiser
  A17 Pro/M3 or newer. Intel/AMD not covered by the PDF.
- Intel: forum thread 715215 (https://developer.apple.com/forums/thread/715215, Sep 2022): on a
  2019 Intel MacBook Pro temporal unavailable, spatial "4ms+" for 1280x720 -> 2560x1440. Thread
  735693 (https://developer.apple.com/forums/thread/735693): makeTemporalScaler returned nil on a
  Mac mini (13.3.1, CPU unstated), worked on M1 Mac mini.
- iOS support came with iOS 17 (WWDC23); forum 707667
  (https://developer.apple.com/forums/thread/707667) reports no MetalFX on A15/iOS 16 in 2022.
- Godot (https://github.com/godotengine/godot/blob/master/drivers/metal/metal_device_properties.cpp):
  fallback "Defaults taken from macOS 14+": temporal min scale 1.0, max scale 3.0; queries
  supportedInputContentMin/MaxScale on macOS 14+/iOS 17+. Godot's MetalFX effect
  (servers/rendering/renderer_rd/effects/metal_fx.cpp) uses depthReversed = true, motion vector
  scale = input size, jitter = taa_jitter * 0.5 with "Flip y-axis as bottom left is origin".

## 9. Reported costs and artifacts

- Forum 728545 (https://developer.apple.com/forums/thread/728545, 2023): M1 Pro, 1904x1370 ->
  2856x2055: "the upscaler adds an additional ~10ms" on an otherwise ~600 us frame. Reply (M1
  Max, 768^2 -> 1536^2): "MetalFX is taking 4 ms when I'm rendering at 120 Hz"; GPU stayed at
  ~400 MHz under light load; "heavier workloads will access higher clock speeds and decrease
  MetalFX upscaling time proportionally" (user's claim).
- Resident Evil Village via Digital Foundry, reported by Macwelt
  (https://www.macwelt.de/article/1379277/resident-evil-village-fur-mac-apples-metalfx-uberzeugt.html):
  Quality mode good, keeps small detail; transparency/water weak; Performance mode "strong
  artifacts". Cyberpunk 2077 Mac: M3 Ultra "no visible ghosting, shimmer, or motion artifacts"
  (https://tbreak.com/cyberpunk-2077-on-mac-apple-silicon/); Performance-mode artifacts on
  lower-end Macs (https://www.galaxus.fr/fr/page/cyberpunk-2077-sur-mac-apple-peut-il-rivaliser-avec-nvidia--38900).
- No first-hand reports found on thin lines or text with MetalFX.

## 10. objc2-metal-fx

- https://docs.rs/objc2-metal-fx/0.3.2/objc2_metal_fx/ ; crates.io: 0.3.2 released 2025-10-04,
  generated from Xcode 26.0.1 SDK
  (https://github.com/madsmtm/objc2/blob/main/docs/FRAMEWORKS_CHANGELOG.md; main branch moves
  to Xcode 27.0). Deps: objc2 >=0.6.2,<0.8, objc2-foundation ^0.3.2, objc2-metal ^0.3.2.
  Default features include every scaler (MTLFX* and MTL4FX*). All methods `unsafe`. Links
  MetalFX.framework itself (`#[link(name = "MetalFX", kind = "framework")]` in generated/mod.rs).
  Does not have the OS 27 jittered / output-resolution motion vector flags.

## 11. Our probe (prototypes/metalfx-probe), 2026-10-14, M1 Pro, macOS 26.5, release

- Device: supportsDevice temporal true, spatial true; supportedInputContentMinScale 1.0,
  MaxScale 3.0; supportsMetal4FX true for both.
- The scaler's inputContentMinScale/MaxScale (dynamic resolution off) both equal the fixed
  output/input ratio (1.5, 2.0, 3.0).
- Jitter sign sweep (static 320x200 -> 640x400, 16 Halton frames, RMSE vs 4x4 supersampled
  reference; bilinear 0.1296, spatial 0.1267): jitterOffset = -(j - 0.5) -> 0.0860;
  +(j - 0.5) -> 0.2083; (+,-) 0.1944; (-,+) 0.1254; -2(j-0.5) 0.1244; -0.5(j-0.5) 0.0957;
  no jitter 0.1374. Confirms offset = centre - sample, input pixels, y down.
- Motion (pan 1.3, 0.7 output px/frame content motion, 32 frames, final RMSE): correct
  (p_prev - p_cur in input px, scale 1) 0.1189; zero 0.2423; negated 0.2545; x2 0.2531;
  x0.5 0.2270; bilinear of the last frame alone 0.1178.
- Zoom in 3 %/frame: correct 0.0489, zero 0.0616, negated 0.0616, bilinear alone 0.0525.
- Pan speed sweep (correct motion, temporal vs bilinear-alone): (0.1,0.05) 0.1105 vs 0.1257;
  (0.25,0) 0.1144 vs 0.1247; (0.5,0.5) 0.1265 vs 0.1194; (1.3,0.7) 0.1189 vs 0.1178;
  (2,0) [= 1 input px] 0.0637 vs 0.1210; (4,0) 0.0819 vs 0.1232; (8,3) 0.1005 vs 0.1538.
- Constant depth 1.0 / 0.0 / 0.5, depthReversed false/true: identical results (0.1189).
