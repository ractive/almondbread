# Dense-region sampling: literature notes

Date: 2026-10-14. Scope: ways to compute fewer pixel-iterations for the same
perceived image in dense, chaotic boundary regions of the Mandelbrot set, where
every pixel needs thousands of iterations but DE lighting, DE outline and
palette filtering turn most of the area into a near-uniform averaged texture.
Covers fractal-specific region skipping (solid guessing, Mariani-Silver,
boundary tracing, circle tiling, KF/UF/XaoS/Fractint options), DE-driven
adaptive supersampling, derivative-based early termination (DEM/M), periodicity
checking, and general graphics adaptive sampling and reconstruction (Mitchell,
Painter-Sloan, Bolin-Meyer, Hachisuka, JBU, guided filter, SVGF, checkerboard,
VRS, FSR2). Raw notes, paraphrased; short quotes marked. GPL/AGPL projects
(KF2+, Fraktaler 3, XaoS) are described in our own words; no code copied.
"Unverified" marks claims I could not check against the primary text.

---

## 1. Fractal-specific sources

### 1.1 mathr: "Adaptive super-sampling using distance estimate" (2014-11-22) — NEW page `mathr-adaptive-supersampling-de`

- URL: https://mathr.co.uk/blog/2014-11-22_adaptive_supersampling_using_distance_estimate.html
- Author: Claude Heiland-Allen. Terms: copyright the author; code linked from
  the post (`adaptive-dem.c`), not copied here.
- Rule: colour = tanh(DE in pixels). By Koebe 1/4, "every dark pixel is near
  the boundary and every pixel near the boundary is dark". So only dark pixels
  need subdividing: light pixels "are far from the boundary and would still be
  far from the boundary when subdivided".
- Algorithm: iterate z and dz/dc; escape radius 512; DE_px = 2|z|ln|z| /
  (|dz/dc| * pixel_spacing). If DE_px <= 4 and depth > 0: split into 4
  sub-pixels at quarter-pixel offsets, recurse with half spacing, average.
  Else return tanh(DE_px). Non-escaping points return 0 (interior not handled;
  extending to interior DE left as an exercise).
- Results, 960x540, maxiters 1e6 (samples per pixel relative to depth 0, time):
  - sparse view: d1 134% 0.66 s; d2 229% 1.30 s; d3 494% 3.18 s; d4 1233% 8.74 s (d0 0.44 s).
  - dense view: d0 17.2 s; d1 431%; d2 1669% 5m18; d3 6283% 20m53; d4 23466% 81m36.
  - Dense view "only beats plain supersampling at depth 3 or above"
    (uniform 2^d x 2^d = 4^d: 1600% at d2 vs 1669% adaptive; 6400% at d3 vs 6283%).
- Key implication for dense regions: when nearly every pixel is within 4 px of
  the boundary, DE-driven adaptivity saves nothing and the recursion overhead
  makes it worse. Efficiency depends on the apparent fractal dimension of the
  view.
- Already cited from deep-mandelbrot README (munrocket/deep-mandelbrot, ref 6),
  which lists "Adaptive super-sampling" as implemented (implementation details
  not documented; unverified that it uses this exact rule).

### 1.2 mathr: "Faking distance estimate colouring" (2014-12-13)

- URL: https://mathr.co.uk/blog/2014-12-13_faking_distance_estimate_colouring.html
- Author: Claude Heiland-Allen.
- SFT, KF and Mandel Machine (at the time) did not compute derivatives, so the
  author approximated DE-style colouring from the slope of the saved iteration
  data (neighbour differences) with a monochrome palette ("pseudo-de").
  Unsupersampled results look "pretty ugly"; supersampled and downscaled they
  come close to true DE. Exact formula not given in the post text (unverified).
- Relevance: an image-space DE proxy from neighbour iteration differences, i.e.
  reconstruction-side information that needs no derivative; quality depends on
  supersampling.

### 1.3 mathr: "Practical interior distance rendering" (2014-11-02) — existing page `mathr-interior-distance`

- URL: https://mathr.co.uk/blog/2014-11-02_practical_interior_distance_rendering.html
- New facts for this topic: three strategies. "Plain" (no interior checks);
  "unbiased" (interior check at every atom-domain partial; fast on views with
  interior, dramatically slower on zoomed views with no visible interior where
  the checks are "all useless"); "biased" (uses the previous pixel's result: after
  an exterior pixel, defers interior checks until maxiters, so escaping pixels
  pay nothing). Biased ≈ plain on exterior-dominated views. No absolute timings
  in text (graphs only).
- This is the cleanest published answer to "periodicity/interior checks cost
  time in exterior-dense regions": use neighbour coherence to decide whether to
  pay for them.

### 1.4 mathr: "Cardioid and bulb checking" (2022-11-19)

- URL: https://mathr.co.uk/blog/2022-11-19_cardioid_and_bulb_checking.html
- Implicit tests for main cardioid and period-2 disc; bounding-box test of the
  whole view first so per-pixel tests only run where the boundary crosses the
  view ("can save O(WH) work"). Perturbed form for deep zoom: 24 bits not
  enough, 53 "seemed ok", 64 used. No timings. Irrelevant at deep zoom in
  general (views are rarely near the cardioid) but the view-level test idea
  carries over (classify whole tiles before per-pixel work).

### 1.5 Mu-Ency (Robert Munafo) adjacency optimisation pages — NEW page `muency-adjacency-optimization`

- https://mrob.com/pub/muency/adjacencyoptimization.html — overview. Methods:
  successive refinement with solid guessing, boundary scanning, Mariani/Silver,
  circle tiling. Successive refinement "can miss narrow features such as
  cusps"; boundary scanning "will sometimes miss features if the maximum dwell
  is too low"; for fewest missed pixels "you should use Mariani/Silver";
  circle tiling "is guaranteed never to mis-identify points as exterior" (if
  DE is computed correctly) and "will not miss anything in the Set itself" but
  "does a horrible job of showing the Dwell Bands" and performs poorly where
  box-counting dimension is high.
- https://mrob.com/pub/muency/marianisilveralgorithm.html (Munafo, 2010-09-08,
  updated 2020-09-02): evaluate the rectangle border; if all equal, fill solid;
  else split into halves or quarters and recurse; splitting into halves reuses
  three known edges. Non-rectangular subdivisions (Ivan Freyman: pentagons,
  triangles) also work. "will occasionally miss features if the maximum Dwell is
  too low" and "often misses parts of cusps narrower than one pixel", a flaw
  shared by every adjacency optimisation except circle tiling. No proof of
  validity given on the page.
- https://mrob.com/pub/muency/circletiling.html (1996, "not finished yet"):
  DE / interior DE gives a disc wholly outside / inside M; points on the disc
  edge seed new discs recursively (after *The Beauty of Fractals*). Must ignore
  discs below a minimum size and avoid already-covered areas. Many areas that
  "ought" to be tiled are unreachable through narrow channels. No cost figures.
- https://mrob.com/pub/muency/solidguessing.html: glossary stub only;
  adjacency-based, used in raster scan (FRACTINT) and successive refinement
  (Super MANDELZOOM). No algorithm.
- https://www.mrob.com/pub/muency/speedimprovements.html: index page only.
- Terms: copyright the author (some pages state CC BY-NC 4.0).

### 1.6 Fractint / Xfractint help (passes) — NEW page `fractint-passes-help`

- URL: https://sources.debian.org/src/xfractint/3.04-1/help3.src (Xfractint
  3.04 help source). Also v13 notes via
  https://mrob.com/pub/muency/fractint.html (not opened). Terms: Fractint is
  freeware with source; help text copyright the Stone Soup Group (paraphrase).
- `passes=` option: single (1), dual (2), triple (3), solid guessing (g,
  default), solid guessing after pass n, boundary tracing (b), tesseral (t),
  diffusion/successive refinement (d).
- Solid guessing: 2–4 visible passes; first pass computes one pixel per 4x4,
  8x8 or 16x16 box and guesses the next level; later passes skip "blocks which
  are surrounded by the same color". Quote: "Solid-guessing can guess wrong, but
  it sure guesses quickly!"
- Boundary tracing: "only works accurately" for fractal types without islands of
  colour (fine for Mandelbrot, not Newton). Man page warns it sometimes locks up
  (secondary, via search snippet).
- Tesseral (Mariani/Silver, "super solid-guessing"): fill rectangles whose
  boundary is one colour; "subject to errors when islands of color appear inside
  the rectangles".
- Single and dual pass give identical images; single pass is the accuracy
  reference.

### 1.7 XaoS algorithms document — NEW page `xaos-algorithms`

- URL: https://raw.githubusercontent.com/xaos-project/XaoS/master/src/engine/algorithms.md
  (repo https://github.com/xaos-project/XaoS). Authors: Jan Hubička (text
  originally from the XaoS docs; repository maintained by the XaoS project).
  Terms: GPL-2.0 repository; paraphrased.
- §(intro): not every pixel is recomputed each frame; previous-frame pixels
  within less than one pixel step are accepted.
- §Calculating New Pixels: boundary detection (trace boundary, fill lakes) used
  only for new images; "Many claim that it does not introduce any errors, but
  this is not true": a connected part of a lake can be too thin to be visible at
  the sampling resolution, and then the whole area is missed. XaoS uses a
  modified solid guessing on added rows/columns: check the nearest three pixels
  on the rows above and below; if all equal, assume a 9x9 block is the same.
  Crossing new rows/columns: test only four pixels ("very rare", "hoped" not to
  add many errors). Interlacing: compute odd lines without guessing, then guess
  the rest; saves about 30% on the main Mandelbrot image. Same along x.
- §Calculation of the Mandelbrot Set: inner loop unrolled 8x with one bailout
  test per block, re-run last 8 slowly on escape. Periodicity checking loop is
  "significantly slower", so it is used only when solid guessing has already
  found some inside-set pixel.
- §Dynamic Resolution: compute only what fits in the frame time; uncomputed
  rows/columns copy the nearest; priority = size of the uncomputed block (binary
  subdivision so coverage is uniform), boosted for rows/columns near the zoom
  centre by `p / (|newpos - oldpos|/step + 1)`; frame time budget = 5x the time of
  the rest of the program, clamped to 15–35 fps while animating, 5–15 fps when
  still.
- Existing pages `wikipedia-xaos`, `xaos-readme` lack these details.

### 1.8 Kalles Fraktaler 2+ manual — existing page `kf-manual`

- URL: https://mathr.co.uk/kf/manual.html. AGPL/GPL program; manual paraphrased.
- Already on the page: "Use guessing" (§Advanced › Use guessing): "Enable
  interpolation of neighbouring pixel data when the iteration count is the
  same"; speeds interior and glitches; disable with OpenCL in exterior regions
  "for speed reasons". Fast preset enables it; Best preset disables it.
- New / emphasise for this topic:
  - §Status bar › Left: pixel states include "guessed good" and "guessed bad"
    (guessing interacts with glitch status: a guessed pixel can inherit a
    glitch).
  - Changelog kf-2.12.8: only interpolate when the glitch status of both
    neighbours is the same; ignoring this "caused lines when dragging the view".
    => with perturbation, equal iteration counts are not evidence of equal
    values when one neighbour is glitched.
  - kf-2.12.4: guessing disabled for "occasional randomly bright single pixels at
    low zoom levels" (race suspected); kf-2.12.7 re-enabled behind a menu
    option; same release added Adam7-style interlacing with circular sorting for
    low-res previews.
  - kf-2.13.2: "image corruption when enabling guessing with analytic DE" fixed
    (guessing must copy/interpolate all per-pixel channels, including DE, or
    none).
  - kf-2.14.10.3: "guessing exterior gives poor quality images" (reported by
    PrinceOfCreation) fixed (nature of fix not stated).
  - §Known Bugs: race between guessing and progressive interlacing passes
    (doesn't wait for the previous pass to finish); OpenCL guessing gives about
    1.5x instead of 4x on views with much high-iteration interior; needs even
    image dimensions with OpenCL.
  - §Interior checking (NanoMB only): "may or may not speed up" per-pixel work
    with large interior regions; experimental.
  - Jitter seed/scale (§Formula dialog): seeds give slightly different images
    for stacking (offline AA). Best preset enables jitter seed 1.
- Not documented: the exact neighbour pattern KF uses (it guesses a pixel of a
  later interlace pass from already-computed neighbours of earlier passes;
  unverified from source).

### 1.9 Fraktaler 3 documentation — existing page `fraktaler-3-site`

- URL: https://fraktaler.mathr.co.uk/ (mathr.co.uk/f3/ redirects there). AGPL.
- New facts: §Quality Window "Samples": quality by "computing many slightly
  different versions of the image and averaging them"; Samples = 0 runs
  indefinitely (progressive accumulation). "Sub" lowers resolution for
  interaction; "Super" raises it. §Image Parameters `image.subframes` = number
  of AA subframes. GUI renders one subframe at a time; batch all at once. No
  guessing, no adaptive sampling, no interlacing documented. Uniform stochastic
  supersampling only.

### 1.10 Ultra Fractal help — NEW page `ultrafractal-antialiasing`

- Render-to-disk AA: https://www.ultrafractal.com/help/exporting/antialiasing.html
  - Adaptive AA: a pixel is anti-aliased only if the difference to its
    neighbours, summed over red + green + blue, is >= threshold. Threshold 0
    disables adaptivity (slower); needed where adaptivity "performs poorly",
    e.g. images with many thin lines (Embossed formulas).
  - Depth >= 1 (default 1, usually enough); "+1 depth can easily double or
    triple the calculation time". Subdivisions 9 (3x3, default, recommended) or
    4 (2x2, for quick renders or depth > 1). "Non-adaptive" option: better for
    some images, much slower.
- Fractal-window AA: http://www.ultrafractal.com/help/fractalwindows/antialiasing.html
  - Simply renders at 2x width and height internally and downsamples; render to
    disk is "more advanced": 3x3 per pixel with a threshold to skip subsampling
    "when this is unlikely to improve the final image".
- Drawing methods: https://www.ultrafractal.com/help/writing/reference/settings/method.html
  - `method` = guessing (default), multipass (Multi-pass Linear), onepass.
- Rendering: https://www.ultrafractal.com/help/exporting/renderingimages.html
  - "Check the Force Linear drawing method option for the highest image
    quality": forces One-pass linear in all layers; guessing is for convenience
    while editing. => UF's own guidance: guessing is a preview tool, not for
    final images.
- Periodicity: https://www.ultrafractal.com/help/writing/reference/settings/periodicity.html
  - 0..3 (0 off, 3 "Rough"), default Normal (2). No error discussion.
- Not found: UF's description of how its guessing works.
- Existing UF pages (`ultrafractal-perturbation-*`) cover perturbation only.

### 1.11 Fractal eXtreme (Cygnus Software) guessing

- URL: https://cygnus-software.com/docs/html/Definitionguessing.htm
- Guessing exploits large iteration bands; "aggressive guessing will cause
  noticeable errors on some pictures", so every calculation ends with a pass
  that looks for "potentially incorrectly guessed pixels" and fixes them. Claims
  up to ~10x speedup "with no discernible loss of quality". Fixing rule not
  described (unverified). Only source found that pairs aggressive guessing with
  an explicit verify-and-repair pass.

### 1.12 Wikipedia: Plotting algorithms for the Mandelbrot set — existing page `wikipedia-plotting-algorithms`

- URL: https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set
- Nothing new beyond the existing page: border tracing 93.16% iteration
  saving on its example; rectangle checking slower than border tracing; both
  only for regions of "one discrete color"; corner-only Fractint variant
  "causes damaged pictures more often"; periodicity checking "is a trade-off"
  (memory/bookkeeping vs saved iterations). No section on exterior DE for
  skipping/filling.

### 1.13 Wikibooks: DEM/M — related to existing `wikibooks-*` pages (no demm page yet)

- URL: https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/demm
- §Analysis of code from BOF: *The Beauty of Fractals* DEM program says "If
  overflow occurs the point is regarded as belonging to the boundary"; the page
  (author "Gertbuschmann") calls that untrue and says the BOF code updates z
  before z' (wrong order: z'_{k+1} uses z_k).
- Wolf Jung's `mndlbrot::dist` (C++ quoted on the page) breaks out when
  |dz| > 1e40 or |z| > bailout; comment: "This would not be a good idea if most
  points escape anyway." Returns class "exterior but very close to the boundary"
  when the estimate < 0.04.
- A commented-out C line: treat as interior when |dZ|^2 > 1e60 ("interior when
  derivative explodes") — disabled in the page's code.
- Heiland-Allen pseudocode: `d := de / pixel_spacing` (DE in pixels).
- DEM/M wording from *The Science of Fractal Images* (Peitgen & Saupe eds.,
  Springer 1988, pp. 196–199; DEM/M on p. 198 per Mu-Ency
  https://mrob.com/pub/muency/distanceestimator.html): NOT VERIFIED. I could
  not access the book. The OVERFLOW constant and "if |dz| > OVERFLOW then
  boundary" rule recalled by the caller is consistent with the BOF passage
  quoted above, but its exact wording, value and section remain unverified. A
  fractalforums.com post (Duncan C, 2007-06-07,
  https://www.fractalforums.com/mandelbrot-and-julia-set/question-on-distance-estimator-algorithms-from-science-of-fractal-images)
  confirms the book has DEM/M pseudocode on pp. 196–199 but does not reproduce it.
- No source found that uses `|dz/dc| * pixel_size` (pixel-footprint
  expansion) explicitly as a stop criterion. Closest: mathr's DE-in-pixels
  threshold (1.1) and Wolf Jung's derivative cap. Treat a footprint-based
  stop as our own inference.

### 1.14 Periodicity checking in renderers (Brent etc.)

- XaoS (1.7): periodicity loop only after guessing found an interior pixel.
- mathr biased/unbiased (1.3): coherence-gated interior checks.
- Ultra Fractal (1.10): user setting 0–3, default 2.
- FracTest changelog 13.0.21 (https://moonblink.info/FracTest/downloads/FracTest):
  "Use Brent's algorithm (instead of Floyd's) for cycle detection." No details.
- Sean Esterkin (advisor Don Spickler), "Fractal Generation on GPU", Salisbury
  Univ. REU poster 2013 (https://faculty.salisbury.edu/~ealu/REU/Projects_File/2013/Fractal%20Generation.pdf):
  CUDA on GT540m, 1024x1024, 200 iterations: 30 fps no optimisation, 33 fps
  with cardioid/bulb check, 22 fps with periodicity checking (slower on GPU).
  CPU version: 3.8 / 12.8 / 4.5 fps. Toy setting (200 iterations), weak
  evidence, but consistent: naive per-pixel periodicity checking costs GPU
  throughput unless many pixels are interior.
- Wikibooks Mandelbrot_set page (https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set
  §interior detection): heuristic "pixel interior with high probability" if it
  and all neighbours are marked interior and all share the same period. Lists
  period detection methods (eps-closeness, Floyd, atom domains); no Brent.
- Not found: a primary fractalforums.org thread on Brent's method in a GPU
  renderer (fractalforums.com returned HTTP 429 during this session; .org not
  indexed by search). Existing page `fractalforums-interior-detection` may
  already cover the derivative-based interior test.

### 1.15 fractalforums.com: "To anti-alias or not to anti-alias" (2012)

- URL: https://www.fractalforums.com/sterlingware/to-anti-alias-or-not-to-anti-alias-hit-the-button-or-leave-it-alone/15
- Tglad (2012-12-19, reply #15): for zero-area detail (lines, points, Koch-type
  curves) "more supersampling just dilutes the pixels until you can't see the
  shape at all"; he scaled shade by n^-D (D = local box-counting dimension)
  instead of n^-2 to keep it visible. lycium (#16): gamma-correct after AA.
  Syntopia (#17): 16 spp, little gain beyond. chaos_crystal (#18): 4x render
  then 3:1 AA "didn't help much"; guesses 10–20x needed for many thin lines.
- Relevance: the "expected/average colour" of a chaotic sub-pixel region
  depends on the colouring's measure; with DE outline (dark near boundary) the
  limit average is dominated by the dark boundary fraction, so a dense region
  converges to a near-uniform tone. Only source found discussing averaged
  colour of sub-pixel fractal detail; no quantitative "expected colour" model
  found (unverified gap).

### 1.16 GitHub: mtraverso3/mandelbrot PR #25 "Supersample adaptively, only where neighboring pixels differ"

- URL: https://github.com/mtraverso3/mandelbrot/pull/25 (merged 2026-09-29).
- 1 spp first, then re-render only pixels that "differ sharply from a neighbor"
  on a sub-grid. Threshold and colour space not stated. CPU times (s): spirals
  off 1.1 / full 2x2 3.6 / adaptive 2x2 2.2 / full 3x3 6.1 / adaptive 3x3 3.6;
  mini-mandelbrot 4.5 / 13.5 / 6.3 / 30.7 / 9.0. Quality "within about 1%" of
  full supersampling against an 8x8 reference. Low-authority hobby project;
  useful only as a recent data point (not a new page).

### 1.17 Other renderers (Imagina, Mandel Machine, FractalShark)

- No documentation found of guessing, adaptive AA or periodicity-based early
  termination in Imagina, Mandel Machine or FractalShark beyond what the
  existing pages say (`imagina-readme`, `fractalforums-mandel-machine`,
  `fractalshark-readme`). Mandel Machine thread lists supersampling up to 32x32
  (search snippet; page returned 429). Unverified either way.

---

## 2. General graphics sources

### 2.1 Mitchell 1987, "Generating Antialiased Images at Low Sampling Densities" — NEW page `mitchell-1987-adaptive-sampling`

- Don P. Mitchell, SIGGRAPH '87, Computer Graphics 21(4), 65–72,
  doi 10.1145/37401.37410. PDF: http://mentallandscape.com/Papers_siggraph87.pdf
- §3 (adaptive sampling): base pass ~1 sample/pixel in a blue-noise
  (Poisson-disc-like) pattern; supersample at 4–9 spp where needed ("At 4
  samples per pixel, aliasing noise is barely visible, and at 9 ... the image
  looks perfect"). Image divided into square cells of 8–9 base samples; per
  cell compute contrast C = (Imax − Imin)/(Imax + Imin) separately for R, G, B;
  supersample if any exceeds its threshold: R 0.4, G 0.3, B 0.6 (green most
  sensitive, after eye noise sensitivity). Variance is "a poor measure of visual
  perception".
- Failure mode (same §): "coarse sampling can miss minute isolated features";
  tripod-leg shadow gaps in the example; "can be removed by doubling the density
  of the base sample, and thus doubling the cost". "Theoretically, it is not
  possible to infallibly detect the presence of high frequency regions in an
  image by sampling at such a low rate."
- §4 reconstruction: multi-stage box filters on a grid 4x finer than pixels
  (16 grid points per pixel area), widening each stage, so dense supersample
  clusters are normalised locally before mixing with base samples (plain
  weighted average lets clusters dominate). ~2 CPU-minutes for 640x480 on a
  VAX 8650, versus 30 min–hours of ray tracing.

### 2.2 Painter & Sloan 1989, "Antialiased ray tracing by adaptive progressive refinement"

- James Painter, Kenneth Sloan, SIGGRAPH '89 (Computer Graphics 23(3)),
  https://history.siggraph.org/?p=119379 (abstract only; full text not read).
- From abstract: adaptive stochastic sampling driven by three goals: coverage of
  the image, location of features, and confidence in values at "pixel level".
  Secondary source (arXiv cs/0511032 dissertation chapter): k-D tree over the
  image plane with a variance/error estimate per node, refined progressively
  until pixel-level accuracy. Exact confidence test: unverified.
- Relevance: progressive refinement ordered by estimated error, with a
  displayable image at every step (fits a per-refresh scheduler).

### 2.3 Bolin & Meyer 1998, "A perceptually based adaptive sampling algorithm"

- Mark R. Bolin, Gary W. Meyer, SIGGRAPH '98, pp. 299–309.
  TR: https://www.cs.uoregon.edu/Reports/TR-1998-002.pdf (abstract only);
  full paper copy: https://www.graphicon.ru/oldgr/library/siggraph/98/papers/bolin/bolin.pdf (not read).
- From abstract: a simplified vision model (colour, contrast sensitivity,
  masking) evaluated during rendering on a wavelet (Haar) representation
  built as samples arrive; uses natural-image statistics to estimate unsampled
  pixels; samples go where the perceptual error estimate is largest. Masking
  is the relevant idea: in busy textures, errors are less visible, so fewer
  samples are needed there. Costs of the vision model: unverified.

### 2.4 Hachisuka et al. 2008, "Multidimensional adaptive sampling and reconstruction for ray tracing"

- T. Hachisuka, W. Jarosz, R. Weistroffer, K. Dale, G. Humphreys, M. Zwicker,
  H. W. Jensen. ACM TOG 27(3) (SIGGRAPH 2008) 33:1–33:10.
  PDF: https://cs.uwaterloo.ca/~thachisu/mdas.pdf (not read in full).
- Two passes: place samples adaptively in the full multidimensional domain
  (image + lens + time + light) by local contrast/error, then reconstruct with
  anisotropic filters along the sample structure. Reported fewer samples and
  less noise than Mitchell-style image-space adaptive sampling. Relevance to us:
  low (our integrand is 2D); the lesson is that adaptivity pays where the signal
  is locally smooth in some direction, which chaotic boundary texture is not.

### 2.5 Kopf, Cohen, Lischinski, Uyttendaele 2007, "Joint Bilateral Upsampling" — NEW page `kopf-2007-joint-bilateral-upsampling`

- ACM TOG 26(3) (SIGGRAPH 2007), doi 10.1145/1276377.1276497.
  PDF: https://johanneskopf.de/publications/jbu/paper/FinalPaper_0185.pdf
- §3: upsample a low-res solution S using a high-res guide Ĩ:
  S̃_p = (1/k_p) Σ_{q↓∈Ω} S_{q↓} f(‖p↓ − q↓‖) g(‖Ĩ_p − Ĩ_q‖); f spatial Gaussian on
  the low-res grid, g range Gaussian on the guide.
- §5: cost O(N r²) in output size N, independent of upsampling factor; 5x5
  Gaussian, σ_d = 0.5; ~2 s per megapixel (2007 CPU). Works well when the
  solution is piecewise smooth with edges matching guide edges (exposure maps,
  depth, chroma).
- Failure mode (inference from §3/§4 assumptions): needs a cheap full-res guide
  whose edges coincide with the solution's edges. For dense Mandelbrot regions
  we have no cheap full-res guide (the expensive quantity IS the iteration
  data), and the solution is not piecewise smooth. A guide could be a cheap
  quantity such as a low-iteration or BLA-level proxy; untested.

### 2.6 He, Sun, Tang 2010, "Guided Image Filtering"

- ECCV 2010, pp. 1–14 (doi 10.1007/978-3-642-15549-9_1); extended TPAMI 2013.
  PDF (TPAMI): https://people.csail.mit.edu/kaiming/publications/pami12guidedfilter.pdf (not read).
- Local linear model q = a·I + b per window, solved by box filters: exact O(N)
  regardless of radius; edge-preserving; avoids bilateral gradient-reversal
  artefacts (claim from secondary source). Same limitation as JBU for us: needs
  a guide correlated with the target.

### 2.7 Schied et al. 2017, "Spatiotemporal Variance-Guided Filtering" (SVGF) — NEW page `schied-2017-svgf`

- C. Schied, A. Kaplanyan, C. Wyman, A. Patney, C. R. A. Chaitanya, J. Burgess,
  S. Liu, C. Dachsbacher, A. Lefohn, M. Salvi. HPG 2017 (best paper),
  doi 10.1145/3105762.3105770. Preprint: https://cg.ivd.kit.edu/publications/2017/svgf/svgf_preprint.pdf
- §4: input 1 spp path-traced colour plus noise-free G-buffer (depth, normals,
  motion). Temporal accumulation by exponential moving average, α = 0.2;
  per-pixel luminance variance from temporally accumulated first and second
  moments (spatial 7x7 estimate after disocclusion). §4.3: edge-avoiding
  à-trous wavelet filter, five levels, weights from depth (σ_z = 1), normals
  (σ_n = 128) and luminance scaled by local variance (σ_l = 4).
- §5.3: ~4.4 ms at 1280x720, ~10.2 ms at 1920x1080 (TITAN X Pascal).
- §6 limitations: over-blurred chrominance (only luminance variance tracked);
  detached shadows in motion; temporally unstable noise in very low light ("As
  the sample density approaches zero, the estimate of variance becomes
  increasingly unreliable"); over-blurred specular; over-blurred sharp features
  under motion (back-projection); incompatible with noisy primary rays (needs a
  clean G-buffer).
- Relevance: variance-guided spatial filtering of a noisy low-spp estimate
  works where a noise-free guide exists. In a fractal, the "noise" in dense
  regions is deterministic aliasing of real detail; filtering it is a choice of
  look (averaged texture), not denoising. Temporal accumulation with exact
  analytic motion (zoom/pan are affine maps) is the transferable part.

### 2.8 Checkerboard rendering

- Wikipedia: https://en.wikipedia.org/wiki/Checkerboard_rendering — render
  about half the samples (often per 2x2 quad), alternating each frame;
  reconstruction filter uses the previous frame; PS4 Pro had hardware support;
  Rainbow Six Siege used it (also as AA). No artefact section.
- El Mansouri, "Rendering Rainbow Six Siege", GDC 2016:
  https://archive.org/download/GDC2016Mansouri/GDC2016-Mansouri.pdf (not read);
  GDC Vault abstract claims up to 50% faster rendering "without great quality
  loss" (vendor claim).
- Wihlidal, "4K Checkerboard in Battlefield 1 and Mass Effect Andromeda", GDC
  2017: https://gdcvault.com/play/1024243/4K-Checkerboard-in-Battlefield-1
  (abstract: object id, alpha unrolling, gradient adjust, depth resolve; slides
  not read).
- Relevance: half-rate sampling per frame plus temporal reconstruction is
  exactly a 2-frame jittered accumulation; it relies on temporal coherence,
  which a still fractal view has perfectly and a zooming view has via the
  analytic warp.

### 2.9 Variable-rate shading / Metal rasterization rate maps

- Apple: https://developer.apple.com/documentation/metal/mtlrasterizationratemap
  and "Rendering with a rasterization rate map"
  (https://developer.apple.com/documentation/metal/rendering-with-a-rasterization-rate-map).
  Render target split into horizontal and vertical zones with per-zone rates;
  render into a smaller physical texture, then resolve. A Metal sample (Bistro
  scene) reportedly cut frame time ~31% (secondhand, gpuweb issue 450:
  https://github.com/gpuweb/gpuweb/issues/450).
- Relevance: rasterisation-only (fragment shading); almondbread's kernels are
  compute, so VRR does not apply directly. The idea (lower rate where
  perceptually cheap) maps to per-tile sample rates in our own scheduler.

### 2.10 AMD FidelityFX Super Resolution 2 (temporal upscaling)

- https://gpuopen.com/manuals/fidelityfx_sdk/fidelityfx_sdk-page_techniques_super-resolution-temporal/
- Inputs at render resolution: jittered colour, depth, motion vectors, optional
  reactive mask (for content without depth/motion footprint, e.g. particles),
  exposure. Per-dimension scale: Quality 1.5x, Balanced 1.7x, Performance 2.0x,
  Ultra Performance 3.0x. Cost at 3840x2160: 0.5–0.7 ms on RX 7900 XTX, 0.9–1.2 ms
  on RX 6800 XT, 3.7–5.4 ms on RX 590; ~448 MB working set at 4K Quality.
  Licence MIT (SDK). Limitations section not read (second half of page).
- Relevance: temporal upscalers assume the shaded signal is band-limited enough
  that jittered low-res samples accumulate into the high-res image over a few
  frames; with exact fractal motion vectors this is plausible for slow motion,
  but chaotic sub-pixel texture will ghost/shimmer under fast zoom (inference;
  history rejection in TAA-style methods clamps to the neighbourhood colour
  range, which in a dense region is wide, so rejection is weak).

### 2.11 Other related items seen but not used

- FractalNow / QFractalNow: `-p <AAMThreshold>` adaptive AA threshold option
  (man page, https://manpages.debian.org/buster/fractalnow/qfractalnow.1).
- Charles University thesis on real-time GPU fractal rendering with adaptive
  supersampling: https://dspace.cuni.cz/bitstream/handle/20.500.11956/109067/130264636.pdf (not read).

---

## 3. Things I could not verify

- Exact DEM/M pseudocode and OVERFLOW wording in *The Science of Fractal
  Images* (p. 198).
- How Ultra Fractal's and Kalles Fraktaler's guessing decide which pixels to
  guess (UF: undocumented; KF: manual only says "when the iteration count is
  the same").
- Any fractalforums.org thread on guessing artefacts with perturbation or
  rebasing; fractalforums.com fetches hit HTTP 429.
- Fractal eXtreme's repair-pass rule.
- Painter–Sloan confidence test; Bolin–Meyer cost figures; Hachisuka results.
- Whether deep-mandelbrot's adaptive supersampling follows the mathr rule.
