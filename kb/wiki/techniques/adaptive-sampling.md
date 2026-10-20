---
title: "Adaptive and sparse sampling, reconstruction, early termination"
type: technique
status: draft
sources: ["[[sources/mathr-adaptive-supersampling-de]]", "[[sources/mitchell-1987-adaptive-sampling]]", "[[sources/ultrafractal-antialiasing]]", "[[sources/xaos-algorithms]]", "[[sources/muency-adjacency-optimization]]", "[[sources/fractint-passes-help]]", "[[sources/kopf-2007-joint-bilateral-upsampling]]", "[[sources/schied-2017-svgf]]", "[[sources/kf-manual]]", "[[sources/fraktaler-3-site]]", "[[sources/mathr-interior-distance]]"]
tags: [adaptive-sampling, anti-aliasing, guessing, reconstruction, dense-regions, measured]
---

# Adaptive and sparse sampling, reconstruction, early termination

Ways to spend fewer samples (or fewer iterations per sample) for the same perceived image:
decide where samples are needed, reconstruct the rest, or stop a sample early. This page
collects what renderers and graphics papers do and what R3 measured for Mandelbrot views with
the default look ([Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md) has the tables and the recommendation).
The approach that did pay, sharing orbits rather than skipping samples, is on
[Local series approximation (tile and pixel seeds)](local-series-approximation.md).

## What existing renderers do

- **DE-adaptive supersampling (mathr 2014).** Colour `tanh(DE_px)`; split a pixel in four only
  while `DE_px <= 4`, by Koebe 1/4 every dark pixel is near the boundary and every pixel near
  the boundary is dark. On a sparse view depth 4 costs 1233 % of 1 spp; on a dense view 23466 %,
  and adaptivity only beats uniform 4^d from depth 3 (src: [mathr: Adaptive super-sampling using distance estimate (2014)](../sources/mathr-adaptive-supersampling-de.md)).
  FL's adaptive AA list (DE < 4 px) is the same rule ([Anti-aliasing and supersampling](anti-aliasing.md)).
- **Contrast-driven adaptive sampling (Mitchell 1987).** About 1 spp in blue noise; per cell
  `C = (Imax − Imin)/(Imax + Imin)` per channel, supersample where R > 0.4, G > 0.3 or
  B > 0.6; coarse sampling "can miss minute isolated features"
  (src: [Mitchell 1987: Generating Antialiased Images at Low Sampling Densities](../sources/mitchell-1987-adaptive-sampling.md) §3).
- **Ultra Fractal adaptive AA**: supersample a pixel when its neighbour difference over
  R+G+B passes a threshold; threshold 0 for images with many thin lines; its guessing modes
  are for previews, "Force Linear drawing" for final quality
  (src: [Ultra Fractal help: Anti-aliasing and drawing methods](../sources/ultrafractal-antialiasing.md)).
- **Guessing / solid guessing / Mariani–Silver / boundary tracing** (Fractint, XaoS, KF):
  fast, and wrong where islands or sub-pixel features sit inside a uniform border
  (src: [Fractint / Xfractint help: drawing passes](../sources/fractint-passes-help.md); [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](../sources/muency-adjacency-optimization.md);
  [XaoS: Algorithms document](../sources/xaos-algorithms.md) §Calculating New Pixels). KF guesses only between pixels of the
  same glitch status; ignoring it "caused lines when dragging the view"
  (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)). Details on [Guessing, border tracing and rectangle subdivision](guessing-and-subdivision.md).
- **Fraktaler 3** does no guessing or adaptive sampling: uniform jittered subframes,
  accumulated (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md) §Quality Window).
- **XaoS dynamic resolution**: compute what fits the frame budget, bigger uncomputed blocks and
  the zoom centre first (src: [XaoS: Algorithms document](../sources/xaos-algorithms.md) §Dynamic Resolution); FL's dynamic
  preview ratio is the same idea.
- **Interior checks gated by coherence**: XaoS runs periodicity checking only after guessing
  found an interior pixel (src: [XaoS: Algorithms document](../sources/xaos-algorithms.md) §Calculation of the Mandelbrot Set);
  mathr's "biased" checking defers interior tests after exterior neighbours
  (src: [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md)).
- **Reconstruction from sparse samples** (graphics): joint bilateral upsampling needs a cheap
  full-resolution guide whose edges match the result
  (src: [Kopf et al. 2007: Joint Bilateral Upsampling](../sources/kopf-2007-joint-bilateral-upsampling.md) §3); SVGF filters 1 spp noise with
  variance from temporal moments and a noise-free G-buffer
  (src: [Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)](../sources/schied-2017-svgf.md) §4). A fractal has no cheap guide: the iteration data is
  the expensive signal, and its "noise" is detail (inference).

## Early termination per pixel

- **Footprint cut (ours, measured: does not pay).** Stop a sample once its footprint image
  `|dz/dc| · pixel` reaches T and take the region's expected colour. In dense regions the
  footprint stays below 1e−3 for ~97 % of the orbit and explodes only just before escape
  (bailout 2¹⁶ means `|dz/dc| · pixel ≈ 1.5e6 / DE_px` at escape), so a cut at T = 1e−3 keeps
  85–97 % of the iterations in exterior views. No sooner point decides the colour.
- **Derivative caps** in the classic DEM/M code treat a derivative overflow as "boundary";
  the Wikibooks DEM/M page says that is untrue, and Wolf Jung's `|dz| > 1e40` stop "would not be
  a good idea if most points escape anyway" (literature notes §1.13; *The Science of Fractal
  Images* wording not verified).
- **Palette/outline saturation.** With palette filtering a pixel with DE ≪ 1 px gets the
  palette's band average and the outline colour, but its lighting depends on the normal at
  escape, and the sub-pixel fibres show in the converged image; there is no shortcut to the
  colour without the escape (measured: replacing such pixels by a local mean costs ΔE 3.8 vs
  1.96 for FL AA, see below).
- **Periodicity / interior shortcuts**: none of the dense views had interior pixels; the
  derivative interior test ([Interior detection](interior-detection.md)) is already in the kernel.
  Brent-style periodicity checking in a GPU loop costs throughput unless many pixels are
  interior (weak evidence, literature notes §1.14).

## Measured (R3, CPU f64, 2026-10-14, M1 Pro; [Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md))

Error is OKLab ΔE × 100 of the default look against a 16-sample converged reference; cost is
iterations relative to exact 1 spp.

| approach | dense-5e-14 | other views | verdict |
|---|---|---|---|
| colour guessing 8→1 (τ 5 / 10) | 0.96 / 0.73 cost, τ 10 changes colours (p99 vs exact 16.7) | 0.24–0.61 at τ 10, but p99 and max errors grow; neck max ΔE 83 | not for final pixels; never in dense regions |
| Mariani–Silver, interior only | no interior | elephant 0.63, minibrot-p4 0.76, 0 wrongly filled | safe, worth it for interior-heavy views |
| sparse grid + texture fill (DE < 0.1–0.5 px) | 2.1–4.2 with AA, ΔE mean 2.5–4.3 vs FL AA 1.96; small flowers vanish | worse than FL AA everywhere except the Feigenbaum haze (ΔE 1.28 vs 1.59 at 0.4× the cost), where it is blotchy at 300 % and erases one-pixel minibrots | no |
| texture fill in motion | flicker 0.85 vs 1.43, but ΔE 3.56 vs 3.13 (blur) | same pattern | no (a blur, not a saving) |
| footprint cut T = 1e−3 | keeps 97 % | keeps 85–97 % at the deep exterior views; at shallow ones (elephant: 2 %) the threshold is reached early, where the colour is anything but decided | no |
| variance-stopped AA (τ 1.5 / 3) | AA cost −9 % / −35 %, ΔE 1.96 → 2.16 / 2.59 | −5 to −28 % at small ΔE cost | minor; not worth a pass |
| local series approximation | 1 spp 0.19, settled AA 1.24 of 1 spp (FL: 8.94), same error | 0.18–0.96 | **yes**, [Local series approximation (tile and pixel seeds)](local-series-approximation.md) |

What the converged image of a dense region is: the box mean of a chaotic signal whose samples
spread over ~10–20 ΔE. Its streaks and fibres are real 1–2 px structure, so the information per
pixel is the local mean at pixel resolution; estimating it needs either many samples per pixel
or pooling across pixels, and pooling removes the structure. Samples can be made cheaper, not
fewer.

## Pitfalls

- A reference that shares samples with the strategy flatters it: the first prototype run
  compared against the full 5 × 5 box, which contains the 3 × 3 AA grid; an independent
  re-draw then looked 15 % worse than the exact AA. Use samples no strategy uses.
- In chaotic regions individual sample outcomes change under any rounding-level change
  (36–48 % of escape counts at the dense view for a 1e−16 relative change); judge equivalence by
  statistics and images, with a rounding-level control.
- DE-based rules alone (`DE_px < k`) select nearly every pixel in dense views: adaptivity by
  DE or variance has nothing to skip there.

## For almondbread

Keep FL's sampling (DE-adaptive AA list, temporal accumulation); make each sample cheaper with
[Local series approximation (tile and pixel seeds)](local-series-approximation.md); add interior-only tile fill
([Guessing, border tracing and rectangle subdivision](guessing-and-subdivision.md)) for interior-heavy views. Do not guess exterior
colours or reconstruct texture from sparse samples.

# Citations

- [mathr: Adaptive super-sampling using distance estimate (2014)](../sources/mathr-adaptive-supersampling-de.md)
- [Mitchell 1987: Generating Antialiased Images at Low Sampling Densities](../sources/mitchell-1987-adaptive-sampling.md)
- [Ultra Fractal help: Anti-aliasing and drawing methods](../sources/ultrafractal-antialiasing.md)
- [XaoS: Algorithms document](../sources/xaos-algorithms.md)
- [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](../sources/muency-adjacency-optimization.md)
- [Fractint / Xfractint help: drawing passes](../sources/fractint-passes-help.md)
- [Kopf et al. 2007: Joint Bilateral Upsampling](../sources/kopf-2007-joint-bilateral-upsampling.md)
- [Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)](../sources/schied-2017-svgf.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [mathr: Practical interior distance rendering (2014)](../sources/mathr-interior-distance.md)
