---
title: "3D worlds (Mandelbox first): not recommended now"
type: plan
milestone: 3D
status: proposed
priority: 10
effort: XL
appeal: high
audience: [casual, artists]
depth: "~1e4–1e5 float32; ~1e10–1e12 with double-float at 7–12× cost (inference)"
realtime-ipad: unknown
reuse: "~20–30 % (Metal host, frame loop, palettes, overlay text, UI shell)"
depends-on: []
risks:
  - "no published perturbation scheme for 3D fractals: a hard depth ceiling"
  - "the opposite of almondbread's selling point (unlimited depth)"
  - "crowded niche (shadertoys, Mandelbulber, App Store viewers; count unverified)"
  - "a new renderer, camera, lighting and content: months of work"
sources: ["[[synthesis/other-fractal-types]]", "[[sources/quilez-mandelbulb]]", "[[sources/wikipedia-mandelbox]]", "[[sources/hvidtfeldt-de-3d-fractals]]", "[[sources/skytopia-mandelbulb]]", "[[sources/alltools-mandelbulb-explorer]]", "[[sources/mandelbulber2-readme]]", "[[sources/apple-metalfx-docs]]", "[[decisions/landscape-mode]]", "[[techniques/double-float]]"]
tags: [3d, mandelbox, mandelbulb, ray-marching, verdict]
created: 2026-10-19
updated: 2026-10-19
---

# 3D worlds (Mandelbox first): not recommended now

Status: proposed and parked, last in priority. Verdict from [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
§2c and §5: do not integrate true 3D into the deep-zoom explorer; if it is ever wanted, build it
as a separate mode. The 2.5D landscape ([Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md)) is the cheaper route to
"flying through a fractal". Also in [Ideas](../ideas.md) (2026-10-19).

## Goal

A separate "3D worlds" mode: fly through ray-marched 3D fractals, the Mandelbox first
(conformal folds, scalar DE, architectural spaces that suit flying), then the Mandelbulb.

## Scope

If it is ever started:

- Ray marcher with a distance estimator (Mandelbox: running scalar derivative, src:
  [Hvidtfeldt: Distance Estimated 3D Fractals (blog series)](../sources/hvidtfeldt-de-3d-fractals.md); Mandelbulb: potential gradient, src:
  [Inigo Quilez: Mandelbulb](../sources/quilez-mandelbulb.md)), DE normals, soft shadows, AO, fog, orbit-trap colouring.
- A 6-DOF camera and flight model (the `Navigator` is 2D pan/zoom/rotate).
- float32 with temporal upscaling (MetalFX, src: [Apple docs and SDK headers: MetalFX framework](../sources/apple-metalfx-docs.md)) and reuse of
  the previous frame's march distance.
- 3D places and a tour.

## Non-goals

- Perturbation for 3D (none published; a research project, not a feature).
- Sharing the deep-zoom engine: reference orbits, BLA/LA, scheduler levels, interior probe,
  seek, π, journeys and the target lock do not apply.

## Design sketch

- Frame cost (our estimate, to be measured): Mandelbulb DE ~10–15 iterations × ~80 flops, ~60–150
  march steps per primary ray plus a shadow ray, ~0.1–0.2 Mflop per pixel; at 2560×1600 that is
  0.4–0.8 Tflop per frame, 80–150 ms on an M1 Pro (~5.3 TFLOPS). 60 Hz needs a quarter of the
  pixels plus temporal upscaling; 120 Hz at full quality is not realistic; an iPad has about half
  to two thirds of that GPU ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2c, all inference).
- Depth: float32 ray marching breaks down at about 1e4–1e5 (src:
  [AllTools: Mandelbulb 3D Explorer (WebGL)](../sources/alltools-mandelbulb-explorer.md), low reliability; agrees with
  [Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md)). Double-float costs 7–12× float32 (measured M7,
  [Double-float (float-float) arithmetic](../techniques/double-float.md)) and would reach ~1e10–1e12 (inference).
- The Mandelbox is the only plausible perturbation candidate (piecewise linear folds, ball fold
  as a piecewise inversion, `z₀ = 0` as a rebasing zero), the Mandelbulb's trig form is singular
  at the origin and on the z-axis (inference).

## Affected modules

A new crate (e.g. `almondbread-3d`) with its own kernels, camera and scheduler; reused:
`almondbread-render`'s Metal host and `CAMetalLayer` surface, frame loop and hang guards,
`almondbread-core`'s palettes and looks, overlay text, the Swift UI shell.

## Verification

- A CPU DE oracle and image diffs per formula; frame-time table on the M1 Pro and the iPad
  (first thing to measure: the estimate above).
- The 2D app's fast path is untouched by construction (separate crate); bench A/B ≈ 1.0 anyway.

## Review gate

Not to be started without an explicit user decision that the 3D look matters more than depth.
Before that, the landscape mode (L-track) gets its review.

## Risks

- No depth: ~1e4–1e5 against our 1e-3000.
- Crowded niche without the depth differentiator; White's own verdict is that 2D keeps more
  variety under zoom than any 3D candidate (src: [Daniel White: Mandelbulb (skytopia)](../sources/skytopia-mandelbulb.md)).
- XL effort for a separate product feel.

## Open questions

- Measured 3D cost on M1 Pro and iPad (only estimates so far).
- Would a landscape mode with the deep kernel (lifting its 1e-12 limit) satisfy the wish for 3D?

## Effort breakdown

XL (two to four months of agent work to reach the 2D app's polish,
[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5).

- Ray marcher, DE formulas, lighting: 3–4 weeks.
- Camera, flight model, temporal reuse and upscaling: 3–4 weeks.
- Content (places, tours), UI integration: 2–3 weeks.
- Verification and tuning on the iPad: 2 weeks.
