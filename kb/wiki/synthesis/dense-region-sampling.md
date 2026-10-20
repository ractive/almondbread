---
title: "Dense boundary regions: fewer pixel-iterations for the same image (R3)"
type: synthesis
date: 2026-10-14
sources: ["sources/mathr-adaptive-supersampling-de", "sources/mitchell-1987-adaptive-sampling", "sources/ultrafractal-antialiasing", "sources/xaos-algorithms", "sources/muency-adjacency-optimization", "sources/fractint-passes-help", "sources/kopf-2007-joint-bilateral-upsampling", "sources/schied-2017-svgf", "sources/kf-manual", "sources/fraktaler-3-site", "sources/wikipedia-plotting-algorithms"]
tags: [dense-regions, adaptive-sampling, series-approximation, anti-aliasing, guessing, measured, decision]
---

# Dense boundary regions: fewer pixel-iterations for the same image (R3)

**Question.** In dense, chaotic boundary regions (the user's slow view `dense-5e-14`:
−1.4080321340304542 + 0.13680236432900417i, 4.6e−14/px, ~3000 iterations per pixel, 85–160 ms
GPU at 1600 × 1000) BLA and AT cannot skip ([Bivariate linear approximation (BLA)](../techniques/bla.md) M4) and the kernel has ~1.5×
headroom. Can we compute fewer pixel-iterations for the same perceived image, while flying and
when settling after a stop?

**Answer.** Yes, but not by sampling fewer pixels. Every pixel-sparse or "guess the colour"
scheme lost visible structure in exactly these regions. What works is sharing the coherent
part of neighbouring orbits: a **local series approximation** around tile and pixel seeds
([Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md)). Measured on the CPU (f64), the refined frame
needs 5.2× fewer iterations at the dense view, and the settled FL image (3 × 3 adaptive AA)
7.2× fewer, with error statistics against a converged reference equal to the exact render's.
Gains are 1.5–5.6× on the other exterior-heavy views and ≤ 1.15× on interior-heavy ones, where
interior-only Mariani–Silver tile fill adds 1.3–1.6× safely.

## Method

Prototype `prototypes/dense-sampling/` (CPU, Rust, depends on `almondbread-core`; raw output in
`kb/raw/research/dense-region-sampling/runs/`). Measured 2026-10-14, M1 Pro, release; all numbers
are deterministic counts, no timing, no GPU.

- Samples: direct f64 iteration from 0 with the kernel's outputs (n, smooth offset, DE in
  pixels, normal angle) and its derivative interior test; limit = 4 × the location's limit
  (the app's extension passes).
- Colour: CPU port of the kernel's `shade` with the default look (Aurora: palette filter in
  OKLab, DE outline 0.7 at 1.6 px, lighting 0.75, highlight 0.35). Error: OKLab ΔE × 100
  (rule of thumb: ~1–2 just noticeable, unverified).
- A 5 × 5 lattice of samples per output pixel over an 800 × 500 crop at each location's
  1600-wide pixel size, extended over a scripted pan (1.2, 0.6 px per frame, 12 frames).
  **Reference** = box mean of the 16 lattice samples per pixel that no strategy uses (the
  centre and the 3 × 3 AA grid are excluded, so shared samples do not flatter a strategy).
  The reference's own noise is about 0.7 × an 8-vs-8 split: mean ΔE 1.7 at the dense view.
- Strategies cost = sum of iterations of the samples they iterate, relative to the exact
  1 spp render (`iters/1spp`). Flicker = mean ΔE between each frame and the previous one
  reprojected (bilinear); the converged reference has a floor from the reprojection itself.
- Locations: the benchmark set (dense-5e-14, deep-5e-14, seahorse-1e5, elephant, minibrot-p4,
  neck-2e5, root-third, feigenbaum at 400 × 250), the FL filament cruise
  (−1.25066 + 0.02012i, 3.75e−9/px) and a minibrot-rich view 100× shallower than the dense one
  (minibrots-5e-12).

## Results per location

`cost` = iterations relative to exact 1 spp; `dE` = mean / p99 against the reference.
"FL AA" is the app's settled image (centre + 8 samples on the adaptive list).

| location | iter/px (1 spp) | DE < 1 px | unresolved px (their share of iterations) | exact 1 spp dE | FL AA: cost, dE | local SA 1 spp (quadtree 32): cost | local SA + AA: cost, dE | escape-n differs: SA / rounding control |
|---|---:|---:|---:|---|---|---:|---|---|
| dense-5e-14 | 3143 | 86 % | 0 % | 5.41 / 18.1 | 8.94, 1.96 / 7.1 | **0.194** | **1.24**, 1.96 / 7.1 | 46 % / 36 % |
| deep-5e-14 | 1391 | 53 % | 0 % | 3.40 / 16.8 | 6.83, 1.24 / 6.4 | **0.243** | **1.81**, 1.23 / 6.5 | 16 % / 12 % |
| filament-1e6 | 562 | 24 % | 0 % | 1.60 / 14.7 | 4.43, 0.59 / 5.7 | **0.177** | **1.10**, 0.60 / 5.7 | 5 % / 0 % |
| minibrots-5e-12 | 4666 | 80 % | 11.9 % (44 %) | 5.20 / 19.7 | 5.56, 2.04 / 11.5 | 0.674 | 2.22, 2.04 / 11.3 | 14 % / 9 % |
| seahorse-1e5 | 177 | 36 % | 0.1 % (4 %) | 2.31 / 15.7 | 5.83, 0.85 / 6.1 | 0.540 | 3.36, 0.85 / 6.1 | 8 % / 0 % |
| neck-2e5 | 3229 | 7 % | 19.5 % (50 %) | 1.20 / 23.8 | 2.40, 0.92 / 23.4 | 0.516 | 1.41, 0.90 / 23.0 | 1 % / 0 % |
| root-third | 5340 | 11 % | 28.8 % (65 %) | 1.93 / 26.3 | 2.51, 1.47 / 26.1 | 0.643 | 1.70, 1.42 / 25.4 | 2 % / 0 % |
| elephant | 1012 | 22 % | 1.9 % (38 %) | 1.48 / 15.2 | 1.86, 0.57 / 6.0 | 0.963 | 1.75, 0.57 / 5.9 | 6 % / 0 % |
| minibrot-p4 | 1017 | 10 % | 0.1 % (19 %) | 0.76 / 13.2 | 5.01, 0.31 / 5.3 | 0.873 | 4.48, 0.31 / 5.3 | 1 % / 0 % |
| feigenbaum (400 × 250) | 142534 | 72 % | 8.2 % (46 %) | 4.22 / 12.8 | 2.27, 1.59 / 7.1 | 0.937 | 1.94, 1.60 / 7.2 | 44 % / 12 % |

The rounding control re-iterates the AA samples from the pixel orbit after 28 iterations
(ε = 1e−9): a pure rounding-level change. Where it already changes a third of escape counts,
individual samples are chaotic and only their statistics mean anything; the SA images match
the exact ones in every statistic and at 200 % (`prototypes/dense-sampling/gallery/`).

### Other strategies (cost relative to exact 1 spp; dE mean / p99)

| location | colour guessing 8→1, τ 10 | Mariani–Silver interior fill: cost, filled, wrongly filled | sparse g4 + texture fill + AA | footprint cut at 1e−3 keeps |
|---|---|---|---|---:|
| dense-5e-14 | 0.73, 5.21 / 18.3 | 1.000, 0 %, 0 | 2.91, 3.23 / 17.2 | 96.9 % |
| deep-5e-14 | 0.61, 3.54 / 17.2 | 1.000, 0 %, 0 | 2.56, 1.78 / 15.9 | 90.0 % |
| filament-1e6 | 0.41, 1.78 / 14.9 | 1.000, 0 %, 0 | 2.44, 0.94 / 12.3 | 85.1 % |
| minibrots-5e-12 | 0.45, 4.96 / 19.8 | 1.000, 0 %, 0 | 1.49, 3.02 / 18.7 | 97.2 % |
| seahorse-1e5 | 0.58, 2.51 / 16.1 | 0.994, 0.1 %, 0 | 2.32, 1.29 / 14.3 | 52.4 % |
| neck-2e5 | 0.29, 1.35 / 24.7 | 1.000, 0 %, 0 | 1.78, 1.12 / 23.7 | 46.0 % |
| root-third | 0.30, 2.15 / 27.4 | 1.000, 0 %, 0 | 1.56, 1.78 / 26.2 | 32.3 % |
| elephant | 0.24, 1.61 / 15.4 | **0.628**, 8.9 %, 0 | 1.26, 0.79 / 11.6 | 1.7 % |
| minibrot-p4 | 0.52, 0.94 / 13.6 | **0.757**, 37.3 %, 0 | 3.73, 0.50 / 11.3 | 31.6 % |
| feigenbaum | 0.25, 3.60 / 12.2 | **0.736**, 15.8 %, 0 | 0.89, 1.28 / 6.1 (FL AA 2.27, 1.59 / 7.1) | 97.7 % |

### Motion (12 frames, 1.2 px per frame; mean over frames 3–11)

| location | 1 spp: dE / p99 / flicker | 1 spp + accumulation (FL-like) | texture fill + accumulation | local SA 1 spp + accumulation (cost) | reference flicker |
|---|---|---|---|---|---:|
| dense-5e-14 | 5.39 / 18.1 / 7.04 | 3.13 / 12.1 / 1.43 | 3.56 / 13.2 / 0.85 | 3.15 / 12.1 / 1.43 (0.19) | 2.49 |
| deep-5e-14 | 3.39 / 16.7 / 4.57 | 2.41 / 12.9 / 0.96 | 2.49 / 13.4 / 0.61 | 2.42 / 12.9 / 0.96 (0.24) | 1.80 |
| filament-1e6 | 1.60 / 14.7 / 2.25 | 1.31 / 11.1 / 0.48 | 1.39 / 11.7 / 0.37 | 1.31 / 11.1 / 0.49 (0.18) | 0.95 |
| minibrots-5e-12 | 5.23 / 19.8 / 6.61 | 2.77 / 11.7 / 1.32 | 3.13 / 12.7 / 0.73 | 2.78 / 11.7 / 1.32 (0.67) | 2.20 |
| seahorse-1e5 | 2.30 / 15.7 / 3.18 | 1.79 / 12.3 / 0.67 | 1.86 / 12.7 / 0.47 | 1.79 / 12.3 / 0.67 (0.54) | 1.32 |
| neck-2e5 | 1.16 / 23.5 / 1.56 | 0.93 / 13.3 / 0.31 | 0.97 / 13.4 / 0.30 | 0.94 / 13.3 / 0.31 (0.51) | 0.66 |
| root-third | 1.91 / 26.2 / 2.41 | 1.21 / 13.0 / 0.46 | 1.25 / 13.2 / 0.43 | 1.21 / 13.0 / 0.46 (0.64) | 0.89 |
| elephant | 1.44 / 15.2 / 2.03 | 1.21 / 12.5 / 0.43 | 1.23 / 12.8 / 0.30 | 1.21 / 12.5 / 0.43 (0.96) | 0.87 |
| minibrot-p4 | 0.75 / 13.1 / 1.08 | 0.70 / 9.9 / 0.24 | 0.75 / 10.7 / 0.19 | 0.70 / 9.9 / 0.24 (0.87) | 0.49 |
| feigenbaum (6 frames) | 4.18 / 12.7 / 4.56 | 1.32 / 6.3 / 1.24 | 1.01 / 4.9 / 0.12 | 1.39 / 6.5 / 1.24 (0.94) | 0.93 |

## What each approach does in dense regions

1. **Pixel-footprint early termination: no.** A dense pixel's footprint image
   `|dz/dc| · pixel` stays below 1e−3 for ~97 % of its iterations and expands only just before
   escape; cutting at 1e−3 keeps 85–97 % of the iterations at the dense, deep, filament and
   minibrot views (shallower views reach the threshold earlier, where the colour is still
   open). There is no point at which the outcome is decided early. Derivative caps in the literature (DEM/M's
   overflow rule, Wolf Jung's `|dz| > 1e40`) treat the point as boundary; the Wikibooks DEM/M
   page calls that untrue, and Jung notes it is no good "if most points escape anyway"
   (literature notes §1.13, `kb/raw/research/dense-region-sampling/literature.md`).
2. **Texture fill / sparse sampling + reconstruction: no.** Replacing pixels with DE < 0.25 px by a
   local mean (Gaussian σ 3–12 px of computed texture samples) is wrong: the converged image of
   the "metallic grey" still has fibres and streaks at 1–2 px and tiny flowers inside it. Texture
   pixels end at mean ΔE 3.8–4.2 (FL AA: 1.96), blotchy at 200 %, and small flowers inside cells
   whose corners are all texture vanish (gallery `dense-ref-a9-quadAA-sparse.jpg`). It does damp
   flicker in motion (0.85 vs 1.43) by blurring. The one exception proves the point: at the
   Feigenbaum view the exterior is a structureless haze, and texture fill beats FL AA on every
   number (ΔE 1.28 vs 1.59 at 0.89 vs 2.27 of the cost; flicker 0.12) because it denoises a
   reference that is itself a 16-sample mean; but at 300 % it is blotchy and it erases the
   one-pixel minibrots on the spike (gallery `feigenbaum-ref-a9-quadAA-sparse.jpg`). ΔE against a
   finite-sample reference rewards smoothing; the images decide. Edge-aware upsampling needs a cheap guide image
   whose edges match the result (src: [Kopf et al. 2007: Joint Bilateral Upsampling](../sources/kopf-2007-joint-bilateral-upsampling.md) §3); a
   fractal has none, the iteration data is the expensive part. SVGF-style filtering assumes the
   noise is noise (src: [Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)](../sources/schied-2017-svgf.md) §4); here it is detail.
3. **Colour guessing (KF/XaoS style, in OKLab): no in dense regions.** τ = 5 saves 4 % at the
   dense view; τ = 10 saves 27 % and changes colours (ΔE p99 vs exact 16.7). It saves 70 % at
   the neck, but with a ΔE max of 83 (lost features). The literature agrees: guessing misses
   sub-pixel features (src: [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](../sources/muency-adjacency-optimization.md)), Ultra Fractal
   recommends linear drawing for final quality (src: [Ultra Fractal help: Anti-aliasing and drawing methods](../sources/ultrafractal-antialiasing.md)),
   and KF only guesses between pixels of equal glitch status (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
4. **Mariani–Silver, interior only: yes, but only where there is interior.** Zero wrongly
   filled pixels at every location (10 views) (the set is full: a closed curve of interior pixels encloses
   no exterior, and the derivative test is conservative near parabolic points, where it leaves
   pixels unresolved instead of interior). 1.6× at elephant, 1.4× at Feigenbaum, 1.3× at minibrot-p4,
   nothing in dense or parabolic views. See [Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md).
5. **Variance-driven AA: small.** Stopping a pixel's 3 × 3 samples once the standard error of its
   OKLab mean is below τ (Mitchell-style, src: [Mitchell 1987: Generating Antialiased Images at Low Sampling Densities](../sources/mitchell-1987-adaptive-sampling.md) §3)
   saves 5–28 % of the settled image's iterations at τ 1.5 (root-third 5 %, minibrot-p4 28 %;
   ΔE mean +0 to +23 %, e.g. 0.31 → 0.35 at minibrot-p4, 1.59 → 1.96 at Feigenbaum);
   at the dense view 9 % (τ 1.5) or 35 % (τ 3) for a visibly noisier result (ΔE mean
   1.96 → 2.16 / 2.59): every texture pixel's variance is high. Adaptivity by DE has the same
   problem: on a dense view it is no better than uniform supersampling
   (src: [mathr: Adaptive super-sampling using distance estimate (2014)](../sources/mathr-adaptive-supersampling-de.md)).
6. **Local series approximation: yes** (above and [Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md)).
   It does not change what is sampled, only how much of each orbit is iterated per sample, so
   minibrots, filaments and the texture's fibres are exactly as in the exact render.

## Recommendation

1. **Build local series approximation in the GPU kernel, in milestone M5** (it is the
   refinement/AA milestone and needs no M3b work; it applies to every depth). Expected
   iteration savings (CPU-measured): dense-5e-14 refined frame 5.2×, settled image 7.2×;
   deep-5e-14 4.1× / 3.8×; filament 5.6× / 4.0×; seahorse 1.85× / 1.7×; neck 1.9× / 1.7×;
   root-third 1.55× / 1.5×; minibrots 1.5× / 2.5×; interior- or unresolved-heavy views
   (elephant, minibrot-p4, Feigenbaum) 1.04–1.15× / 1.06–1.17×. GPU time gains will be smaller
   (seed passes, state copies; to measure on an idle GPU). Start with a kernel prototype that
   answers [Does local series approximation hold in the float32 perturbation kernel?](../questions/local-sa-float32.md) before touching the scheduler.
2. **Then interior-only Mariani–Silver tiles** (1.3–1.6× on interior-heavy views), as the
   2-pass tile design on [Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md). It complements local SA,
   which cannot shorten interior detection (a branched sample restarts its interior
   derivative).
3. **Do not build** exterior guessing, texture fill / sparse reconstruction, footprint
   early termination, or a periodicity check in the hot loop. Keep the FL temporal
   accumulation as is: local SA leaves its quality unchanged and makes each preview texel
   ~3–5× cheaper in dense views, so the same budget buys about 1.7–2.2× the preview resolution
   per axis (the filament cruise's ratio 0.25 previews would become ~0.5).
4. **Unresolved pixels** (near parabolic points and minibrot halos) are now the largest cost
   outside dense views: 50 % of iterations at neck-2e5, 65 % at root-third, 44 % at
   minibrots-5e-12. Local SA helps them only partly; a better interior/unresolved decision is
   a separate question ([Interior detection](../techniques/interior-detection.md)).

## Design: how it slots into the scheduler

Kernel (`iterate`, new function constant `kSeed`; our design, to be prototyped):

```text
NodeState { dz: float2, A: float2, B: float2, m: uint, n: uint }      // 32 B, A = d (DE derivative)
node list entries: (kind = node | pixel, node or pixel index, parent node index)

seed thread (kind = node):
    start from parent's NodeState shifted by δ (texels): dz += A δ + B δ²/2, A += B δ
    loop: exact perturbation step for dz, A (as now) and B <- 2 (A² + z B)  (z = ref[m] + dz)
          stop when |B| r_texels > 2 ε |A|, or escape / interior proof
    write NodeState; append the 4 children (2 x 2 node: its 4 pixels) to the admission list
pixel thread (kind = pixel): init from the parent node shifted by its δ, w = 1, wExp = 0;
    then the unchanged loop. With AA: run in seed mode with r = 0.57 px first, keep the branch
    state in the live state (+32 B) and write it to an AA seed buffer at escape if the pixel
    qualifies for the AA list (DE < 4 px).
AA sample (listMode 2): init from the pixel's AA seed shifted by the Halton offset.
```

- **Admission.** Levels admit quadtree roots (32 × 32 texels) instead of pixels; `prepareChunk`
  admits from the node list (the extension passes' `listMode 2` mechanism), so the live list
  mixes seeds and pixels. Children follow their parent within a chunk or two, so the tile order
  (focus first, exposed area first) is kept at root granularity.
- **Previews (gesture frames)**: same scheme at the preview's texel size; the jitter shifts
  every texel alike, so seeds are per level. The dynamic-resolution model sees cheaper texels
  and raises the ratio by itself.
- **Refinement and fade**: unchanged; pixels finish in quadtree order instead of raster order
  within a tile.
- **Progressive AA**: each jittered sample of a listed pixel starts from its pixel seed, so the
  8 extra samples cost about their last 10–20 % of iterations. FL's 1.5 GPU-s per-view AA budget
  then settles ~4–7× more views fully in dense regions.
- **Shown frame = fresh render.** Branch decisions depend only on the seed's orbit, never on
  chunk boundaries, and a seed's state is stored once and read by its children, so the shown
  frame and a fresh render stay identical (the M4 lesson).
- **Memory**: node states ≈ 1/3 node per pixel × 32 B ≈ 11 B/px while a level runs (46 MB at
  2560 × 1640; can be freed as subtrees finish); AA seeds 32 B per listed pixel.
- **Verification**: `verify` against the oracle at all 13 locations (escape-n, |Δμ| and class
  mismatch no worse than the plain kernel), `regress` sessions, a new `--sa 0|1` skip-off diff
  like M4's `--bla`, and the OKLab-of-the-look comparison from this prototype.

## Risks

- **float32 at the branch** ([Does local series approximation hold in the float32 perturbation kernel?](../questions/local-sa-float32.md)): if `|A δ|` falls below float32
  rounding of `dz`, neighbours start identical; `+dc` separates them again but could leave
  streaks. Must be checked on the GPU before anything else. **Checked (R5):** no collapse;
  8.7× fewer steps and 5× less GPU time at the dense view, but the interior derivative must
  be carried, not restarted, and interior-heavy views lose time
  ([Does local series approximation hold in the float32 perturbation kernel?](../questions/local-sa-float32.md)).
- **GPU efficiency**: the seed phase has 1/1024 of the threads at the 32 × 32 root level (the
  first few hundred iterations of a level run on few threads); children arrive in waves. The
  chunked live list hides some of it; measure wall and GPU time, not just iterations.
- **Statistical, not exact, equivalence** in chaotic regions: escape counts change on up to
  46 % of samples (the rounding control: 36 %); at the Feigenbaum view 44 % against 12 %, the
  largest gap, with 1 spp ΔE 4.25 vs 4.22 exact. Images match in every statistic measured; a
  user comparing a single pixel before/after will see differences, as between float32 and
  f64 today.
- **ε**: 1e−2 is measured; 1e−3 loses half the gain (children fail on their first step). Larger
  ε untested beyond the probe; keep it a tunable with a skip-off diff.
- **Interior and unresolved pixels gain little**; views dominated by them need the other levers
  above.
- **CPU f64 only so far**: the oracle's direct iteration, not the app's perturbation with
  rebasing; the GPU prototype must repeat the numbers.

## Related

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md) (the sampling approaches and literature),
[Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md), [Series approximation (legacy)](../techniques/series-approximation.md),
[Anti-aliasing and supersampling](../techniques/anti-aliasing.md), [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md),
[almondbread benchmarks and verification record](benchmarks.md) (R3 section), [Roadmap](../roadmap.md).

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
- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
