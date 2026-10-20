---
title: Fog over unresolved pixels
type: technique
status: draft
sources: ["sources/fractalforums-interior-detection"]
tags: [coloring, unresolved, max-iterations, gpu, as-built]
---

# Fog over unresolved pixels

How almondbread draws a pixel that reached the iteration limit without escaping and without
being proven interior (iteration code −3, *unresolved*; [Automatic maximum-iteration control](auto-iterations.md)). Such
pixels may still resolve: extension passes and compute further keep iterating them. Without an
interior test every interior pixel runs to the limit (src:
[fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md) §opening post); with one (almondbread's derivative
test and cycle probe, [Interior detection](interior-detection.md)), what is left at the limit is mostly
slow escape or slow convergence near parabolic points. The look is our own design (user decision
2026-10-17); no source describes it. Built 2026-10-17.

## History

- Interior colour lifted 3 % (M2), then 0.5 %: read as interior. Near the −3/4 neck almost the
  whole view is unresolved, so the view looked like a solid set.
- A flat dark grey veil over the outline colour (2026-10-17, morning): distinct from the
  interior, but a hard-edged flat patch next to the fractal.
- Fog (this page): "foggy, cloudy, darker and fuzzier towards the inner part" (user).

## The look

- **At the border** the fog takes the colours of the resolved pixels around it (blurred), 20 %
  of the way towards the deep fog, so the region's edge glows in the fractal's own colours.
- **Further in** it darkens and gets softer: darkness `t = (log2(d) / 7)^0.7`, saturating at
  d ≈ 128 points from the nearest resolved pixel; the colour comes from coarser and coarser
  pyramid levels, so it is blurrier with distance.
- **Deep inside** it is almost black with a faint cool blue-grey tint (linear
  `(0.0065, 0.0080, 0.0125)`, sRGB ≈ (19, 21, 27)), plus 2 % of the very blurred colours around
  (light in a cloud). The tint is only there so a large patch reads as "not finished" next to
  a black interior; it does not need to be precise.
- Distances are in display points (`points_per_pixel / texels per pixel`), so the look is the
  same at any zoom and on any display scale.

## Algorithm (fog pyramid)

Built per colour pass, only when the view has unresolved pixels (`Job::has_unresolved`: a full
level or extension left some, and compute further has not resolved them all). Three kernels
in `mandelbrot.metal`, encoded in the colour pass's own command buffer (`fog.rs`):

```text
fogSeed:    level 0, one texel per 2x2 iteration texels:
            sample ONE texel of the block (position cycling over 2x2 blocks)
            a = 1 if it is finished and resolved, else 0
            store (shade(texel) * a, a)                  # premultiplied colour, coverage
fogDown:    level L = box mean of 2x2 texels of level L-1, L = 1 .. N-1
            (N: down to 2x2 texels, at most 12 levels; even levels in atlas A,
             odd levels in atlas B, so no pass reads and writes one texture)
fogResolve: per texel of level 1 (4x4 iteration texels):
            survive = 1, octaves = 0, border = 0
            if level 4 shows no coverage here: start at level 5, octaves = 5
            for L = first .. N-1, while survive > 1e-3:
                v = bilinear sample of level L at this position
                found = saturate(3 v.a)                  # resolved texels within ~2^(L+1)
                border += survive * found * v.rgb / v.a  # colours at the first scale found
                survive *= 1 - found
                octaves += survive                       # expected first level ~ log2 distance
            border += survive * deep
            lg = octaves + 1 + log2(points per iteration texel)
            t = (saturate(lg / 7))^0.7
            fog = mix(border, deep + 0.02 border, 0.2 + 0.8 t)
colour pass: an unresolved texel draws the fog texture, bilinear
```

The "expected first level" is a soft distance: a level whose footprint just touches resolved
texels contributes partly, so the fog has no steps. Thin resolved filaments inside a large
region count (any coverage at a scale stops the walk), unlike a mean-coverage distance.

## Where it is drawn

- `colorize` (offscreen `render`, every view size and supersampling factor) and `compose`
  modes 1 (refine fade), 3 (exact, colours changed) and 4 (compute further after AA). Mode 4
  recomposes marked texels that are still unresolved too: the fog fades to its new shape as
  the region shrinks.
- **Not** in gesture frames (mode 0), preview colour passes and AA samples (the sample cache, deleted 2026-10-17, was another):
  they draw the plain deep fog (`fogDeep()` in `shade()`), at no cost. The history carries the
  full fog into gesture frames by reprojection.
- The off switch for before/after checks (`ALMONDBREAD_FOG=0`) was removed 2026-10-17; fog is always on.

## Cost

Measured 2026-10-17, M1 Pro, release, 3456 × 2234 (`ALMONDBREAD_COLORIZE_BENCH=50
almondbread render ...`: the colour pass 50 times, GPU min; and compose frames of `verify
--history jump`):

| view (iterations) | unresolved | colour pass without | with fog | extra |
| --- | --- | --- | --- | --- |
| cardioid edge `0.37418 + 0.19342 i`, 1e-7 (1 000) | ~60 % | 1.09 ms | 2.10 ms | 1.0 ms |
| compose mode 1, same view | ~60 % | 1.6 ms | 2.6–3.0 ms | 1.0–1.4 ms |

Breakdown (offscreen): seed 0.45 ms, down passes 0.18 ms, resolve 0.15 ms, sampling in the
colour pass 0.2 ms. A view without unresolved pixels: 0. What did not work:

- Seed reading all four texels of a block: 0.6 ms. The pass is bound by memory (whole cache
  lines of the rg32f iteration texture), and shading is only 0.1 ms of it. One texel per block
  in a cycling pattern: 0.45 ms, and the fog looks the same.
- Resolving per iteration texel (or per level-0 texel): 2.5 ms and 0.6 ms. At level 1 (4 × 4
  texels) it is 0.15 ms; the fog is soft, and bilinear upsampling hides the resolution.

## Texture from the orbit (tried, dropped)

The plan was a faint cloud texture from per-pixel data the kernel has for unresolved pixels.
Looked at, 2026-10-17, by drawing them as false colour:

- `log2 |z|` where the orbit stopped: almost constant near the −3/4 neck and at the satellite
  head (the orbits sit near the parabolic or attracting cycle), with only very slow
  variation that the rg16f de texture quantises into contour lines. Near the cardioid edge it
  is noise in some tiles and smooth in others.
- The derivative / interior-probe multiplier `w`: rings around minibrots (nice), but straight
  tile and chunk seams, because the probe's state depends on how far its chunk got.
- The final `r2` is often 0: probe chunks do not run the main loop.

None is a property of `c` alone, so all show chunk seams or nothing. The blurred colours of the
surrounding fractal (the 2 % tint) give the deep fog its only variation.

## Checks

- `verify --history ... --further-budget s`: of the texels compute further resolved after AA (its
  mask, read back as `SimulationResult::further_marked`), those whose shown colour differs from
  a fresh render of the same field by more than 12 levels (both draw the same sample with the
  same dither). It replaced the check "resolved pixels in the unresolved colour", which matched
  one colour; the fog has none. The new check found that the compose of the last pass's texels
  stopped as soon as compute further ended (12 % of them still partly fog, also with the old
  veil, which the old check could not see): compose mode 4 now runs on for 4 τ after the last
  pass (`further_done_at`), and the view counts as settled only after that. 0 % since
  (`make regress`, neck 400 × 250 with AA).
- Interaction test and selftest compare iteration classes, not colours: unaffected.

## Images (2026-10-17)

Before / after, 1600 × 1000, `render --extend 1`: the −3/4 neck at 4.48e−12 (20 000), the
satellite head (3 000), `cusp-1e5` (3 000), the cardioid edge `0.37418 + 0.19342 i` at 2e−7
(1 000); plus `verify --history` frames of the neck and the cardioid edge (compose path).
Not committed (scratch).

# Citations

- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
