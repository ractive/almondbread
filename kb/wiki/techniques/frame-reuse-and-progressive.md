---
title: "Frame reuse and progressive refinement"
type: technique
status: draft
sources: ["[[sources/wikipedia-xaos]]", "[[sources/xaos-readme]]", "[[sources/kf-manual]]", "[[sources/zoomasm]]", "[[sources/fractalshark-readme]]", "[[sources/fraktaler-3-site]]"]
tags: [interaction, reprojection, progressive, xaos, reuse-reference]
---

# Frame reuse and progressive refinement

How to make a deep-zoom explorer feel instant: show something useful at once,
then refine, and never recompute what is still valid (pixels, reference orbit,
skip table).

## XaoS (Hubička algorithm)

- Saves only the previous frame, kept aligned in rows and columns; remembers a
  real coordinate per column and an imaginary coordinate per row instead of
  per-pixel positions (src: [[sources/wikipedia-xaos]] §XaoS).
- Choosing which saved rows/columns to draw where is treated as an
  optimisation problem; mistakes show as distortion
  (src: [[sources/wikipedia-xaos]] §XaoS).
- Rows/columns without a good match take the colour of the nearest one, then are
  recalculated as CPU time allows, prioritising the zoom target (the edges when
  zooming out) (src: [[sources/wikipedia-xaos]] §XaoS).
- Recalculating every frame also causes 'twinkle' without AA; reuse avoids it
  (src: [[sources/wikipedia-xaos]] §XaoS).
- Still maintained (GPL-2.0, Qt, web version since 4.3 in 2023)
  (src: [[sources/xaos-readme]]). It has no perturbation; see [[implementations/xaos]].

The approximation step costs about `xsize + ysize` work per frame rather than
`xsize × ysize` (notes: pixel_and_image_techniques §6).

## KF2+

- Progressive previews with Adam7-style interlacing and circular sorting
  (centre first); an option disables the sorting to save time and memory
  (src: [[sources/kf-manual]]).
- "Reuse reference" keeps the reference orbit for further zooming; works with
  auto glitch solving since 2.15.3 (src: [[sources/kf-manual]]).
- Guessing races with progressive interlacing (src: [[sources/kf-manual]]).

## Others

- FractalShark's "perturbed perturbation" caches intermediate-resolution
  reference orbits across zooms (src: [[sources/fractalshark-readme]]).
- Fraktaler 3 notes that reusing BLA tables across a zoom-out sequence is not
  generally applicable, because radii depend on the image's max `|c|`
  (src: [[sources/fraktaler-3-site]]).
- zoomasm re-renders only on change (2.0+) and renders the fractal once as
  exponential-map strips for videos (src: [[sources/zoomasm]]); see
  [[techniques/exponential-map]].

## Design for a Metal explorer (inference)

Per-pixel store of the last completed view: `n` (uint32), `frac`, `de_px`,
flags, phase. Colour is derived from it in a separate pass.

```text
on pan/zoom (new view V', old view V with buffer B):
    1. placeholder: draw B through the affine map V -> V' (texture sampler),
       mark every pixel "stale"
    2. exact reuse: if zoom is x2 about a pixel centre, 1/4 of new pixel centres
       coincide with old ones -> copy their data, mark "exact"
    3. if V' still within reference validity: keep reference + skip table
       (rebuild BLA radii only if max|c| grew), else schedule new reference
       on the CPU (double-buffered shared MTLBuffer)
    4. refine coarse-to-fine: 1/16 -> 1/4 -> full resolution, tiles ordered
       by distance to the zoom focus; each level a chunked dispatch
    5. idle: progressive jittered AA samples (techniques/anti-aliasing)
```

- With rebasing, the reference only needs to lie near the view, so it stays
  valid for many zoom steps; BLA radii depend on max `|c|`, so a zoom-in (max
  `|c|` shrinks) keeps the table valid, a zoom-out or big pan may not
  (inference from [[techniques/rebasing]] and [[techniques/bla]]).
- Off-grid zooms allow only approximate reuse; never present approximate
  pixels as final.
- Refinement fits the chunked, resumable kernel architecture
  ([[platform/gpu-watchdog-and-chunking]]): each refinement level is a work list.

No published 2025–2026 interactive deep-zoom renderer documents temporal
reprojection with perturbation (notes: pixel_and_image_techniques §6 Gaps).

## As built in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, 2560×1640 px window at 120 Hz, release
(`--interaction-test`; tables in [[synthesis/benchmarks]]).

- **Gesture frames.** While input is younger than 150 ms, every display refresh shows
  its own view: a preview level whose scale (1/8 steps, 0.25…1) is the largest whose
  measured cost per texel fits ~6 ms of GPU, iterated to completion in
  ceil(texels / live capacity) chunks with K = maxIter (falling back to budgeted
  progressive chunks if the estimate exceeds 50 ms). 113–117 fps on pans and zooms at
  the home view and at 1e5; the home view stays at full resolution 50–90 % of gesture
  frames.
- **Bilinear preview.** The preview is coloured at its own resolution into a linear
  rgba16Float texture and sampled with a linear sampler, instead of nearest-neighbour
  iteration texels (blocky).
- **Refinement.** When input idles, the same job gets the full level (with
  supersampling) and refines progressively over the preview: unfinished pixels show
  the bilinear preview; pixels still iterating with no preview show the interior colour.
  The fully refined full-resolution frame arrives 150–480 ms after the last input.
- **Pitfalls found:** (1) holding the previous image until a new job's first level
  completes never shows anything during a continuous gesture (0 frames presented in 3 s);
  hold only for idle setting changes. (2) A HUD bound to the viewport re-rendered every
  frame and throttled the Metal view to ~67 fps. (3) Re-presenting an unchanged frame at
  the zoom clamp only exhausted drawables.
- Not done in M2 (done in FL, below): reprojection of the previous frame and
  focus-first refinement.

## As built in almondbread (FL: history reprojection and temporal upsampling)

Every presented interactive frame is composed by one kernel (`compose`,
`crates/almondbread-render/src/shaders/mandelbrot.metal`) from a **history**: the last shown
image in linear light (rgba16f, alpha = per-pixel confidence) plus the viewport it shows
(our design, built 2026-10-14; measured M1 Pro, release, [[synthesis/benchmarks]] FL).

- **Reprojection is exact.** A pan or zoom between two views is a pure scale and
  translation, so each output pixel's history position is `origin + pixel · step` with
  `step = s_now / s_hist` (complex units per pixel). Sampled with 5-tap Catmull-Rom (a
  bilinear resample per frame blurs visibly within a second of panning); identity mappings
  read texels directly.
- **Gesture frames: temporal upsampling.** The preview level (dynamic resolution as in M2)
  is jittered by a Halton(2,3) offset per frame, so its texels land on new sub-pixel
  positions every frame. Each texel is a point sample adding weight `exp(−d²/2σ²)` (σ = 0.5
  output pixel) to nearby pixels; the history adds its confidence, decayed by `step²` on
  zoom-in (a history texel is stretched over `1/step²` pixels) and capped at 6. Where neither
  exists (newly exposed area) the bilinear preview shows. Result: slow pans and zooms
  converge towards full resolution within a few frames even at preview ratio 0.25–0.5;
  continuous zooms keep the detail the history still holds (stills in the review videos:
  mid-dive frames are sharp where M2 showed a ratio-0.4 bilinear blur).
- **Refinement fades in per pixel.** When input idles (150 ms) the full level iterates
  over the history; every finished pixel moves towards its exact value by
  `1 − e^(−dt/τ)` per presented frame (τ = 30 ms), so there is no pop and the fade is the
  same at any refresh rate. The refined-frame time keeps M2's meaning (all pixels exact).
- **Focus-first refinement.** Admission is in 16×16 tiles (texel index → tile rank → tile
  via an order buffer; the inverse rank buffer tells colorize/compose what is admitted).
  Refinement levels order tiles the history does not cover (newly exposed by a pan or a
  zoom-out) first, then by distance from the zoom target. Tiles also replaced scanline
  admission everywhere (raster tile order for previews and offscreen); iteration results
  are unchanged (fresh-render class mismatch 0–0.001 % in every interaction scenario).
- **Budget follows the display.** The gesture frame budget is half the measured refresh
  interval (4–12 ms) instead of M2's fixed 6 ms, which assumed 120 Hz: at 60 Hz previews
  get ~2× the texels. 70 % of the interval measured 49 fps in home zoom at 60 Hz.
- **Colour changes and cycling.** The history stores colours, so it is valid only for the
  colour parameters it was made with (palette phase aside: depth tint changes it slightly
  every zoom frame). A colour change while idle recolours from the exact samples; colour
  cycling advances only while the view rests, so flying keeps reprojection valid.
- **A recolour is not a new job, so it must keep the display link alive itself.** A colour
  change (palette, look, a switch between non-flat interior colourings) keeps the compute key:
  the only work is one exact recompose, then AA again (the AA samples hold the old colours and
  are dropped). Until 2026-10-19 neither kept the frame loop running once the view had settled:
  a recolour refresh with no drawable reported idle and the display link paused on the old
  colours, and a done recolour left the job "settled" with AA not rerun (aliased edges). Now a
  planned frame without a target asks for the next refresh, and a recolour clears the settled
  flag ([[decisions/controls]] "The colours tour"). Rule: anything the plan still wants to
  compose must show up in the idle test.

Pitfalls found: (1) a jittered gesture level reused as the refined frame breaks the
shown-frame check (the refined level must be centre samples, so gesture levels always live
in the preview allocation, even at ratio 1). (2) The fresh-render check's renderer took the
new 9-sample default and rendered a 3× grid (38 % "mismatch") until it was pinned to 1
sample. (3) Unresolved pixels (past the nominal limit) form large halos around minibrots in
gesture frames, which get no extension passes; M2's 3 % grey made them flat grey slabs, so
they now take the outline colour.

## For almondbread

Reproject the stored previous frame as an immediate placeholder, copy exact
pixels on centred ×2 zooms, keep the reference and skip table across zooms
(KF "reuse reference"), and refine coarse-to-fine from the focus with chunked
dispatches, then accumulate AA. Related: [[techniques/reference-selection]],
[[techniques/auto-iterations]].

## R4: reuse samples, not the image

FL computes every gesture frame's preview fresh. R4 measured an alternative: a sample cache in
fractal coordinates that keeps every computed sample at its exact position and re-shades it
each frame. It matches FL's image with 2–50 % of FL's iterations per frame, or is sharper at
the same iterations, except in the deep, sample-starved zoom. MetalFX upscaling gave no gain
over the compose above. (The cache was built opt-in and deleted again 2026-10-17, never default.) Details: [[techniques/sample-reuse]], [[synthesis/motion-sample-reuse]].
