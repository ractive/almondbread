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
- Not done yet: reprojection of the previous full-resolution frame and coarse-to-fine
  levels from the focus point.

## For almondbread

Reproject the stored previous frame as an immediate placeholder, copy exact
pixels on centred ×2 zooms, keep the reference and skip table across zooms
(KF "reuse reference"), and refine coarse-to-fine from the focus with chunked
dispatches, then accumulate AA. Related: [[techniques/reference-selection]],
[[techniques/auto-iterations]].
