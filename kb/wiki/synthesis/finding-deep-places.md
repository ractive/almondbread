---
title: "Finding good places at extreme depth (1e100 to 1e100000+)"
type: synthesis
date: 2026-10-17
sources: ["[[sources/mathr-julia-morphing-posts]]", "[[sources/mathr-embedded-julia-navigation]]", "[[sources/mathr-domains-and-periodicity-scan]]", "[[sources/heiland-allen-inflected-julia-sets-2025]]", "[[sources/dinkydau-evolution-journals]]", "[[sources/fractalforums-pathfinding-revisited]]", "[[sources/fractalforums-deepest-zoom-poll]]", "[[sources/deep-zoom-videos]]", "[[sources/wikibooks-emndl]]", "[[sources/kf-manual]]", "[[sources/fraktaler-3-site]]", "[[sources/fractalshark-readme]]", "[[sources/imagina-readme]]", "[[sources/muency-newton-raphson-zooming]]", "[[sources/muency-atom-domain]]"]
tags: [places, julia-morphing, shape-stacking, minibrot, misiurewicz, newton, coloring, records, automation, research]
---

# Finding good places at extreme depth

**Question.** How does the deep-zoom community find places worth looking at past 1e100,
which deep places are known, what makes them look good, and what could almondbread
automate? Our own data point: a minibrot nested in the needle at 1e3000 had period 1.4 M and
looked flat, while minibrots next to low-preperiod Misiurewicz points looked good
(reported by the driver session 2026-10-17; the Misiurewicz-based places are in
[[synthesis/places]] "Below 1e1000").

**Short answer.** Nobody finds good deep places by search. They are *built*: the view at
any depth shows a record of the path taken to get there, and artists steer that record by
choosing which minibrot to approach and stopping at fixed fractions of the log-distance to it
(1/2 for a morph, 3/4 for a doubling). Newton's method does the expensive part (finding the
minibrot and its depth); the human picks the targets. Automation exists (Heiland-Allen's
`m-*` tools, emndl_autotune, KF/F3 Newton zoom, FractalShark's Feature Finder) but chooses
geometry, not beauty. Density at depth is a colouring problem, solved by scaling the
iteration count (KF "iteration division", log, histogram) or by colouring from the distance
estimate instead.

## 1. How places are found

### The path builds the shape (shape stacking, Julia morphing, evolution)

- Every minibrot sits in concentric "shells" that record the path to it from the main
  cardioid: cusp, then seahorse valley, then the other side, then a minibrot gives a
  minibrot ringed by exactly those patterns. "The path you choose to follow BUILDS the shape
  you will find." (src: [[sources/fractalforums-pathfinding-revisited]]). The forum user
  "claude" (presumably Heiland-Allen, unverified): "history repeats, only twice as fast" (same thread). This is called **shape
  stacking**: each choice made on the way down is stacked as a layer of the final picture.
- **Julia morphing**: zooming towards a minibrot, a view at log-depth `n` that contains a
  minibrot at log-depth `m` looks, at `(n+m)/2`, like the old view mapped through
  `c → c² + c_mini`, i.e. doubled around the minibrot (src:
  [[sources/dinkydau-evolution-journals]] "Simulating Julia morphings"). Mu-Ency calls it
  "the art of locating novel shapes through repeated zooming in to ever higher orders of
  embedding" (src: [[sources/muency-newton-raphson-zooming]]). Each step costs half the
  total zoom again (src: [[sources/heiland-allen-inflected-julia-sets-2025]]).
- **Depth rules of thumb.** Doubling at about 3/4 of the log-depth to a minibrot outside the
  shape, morph with one inside it (src: [[sources/dinkydau-evolution-journals]]); 2 arms at
  3/4, 4 at 7/8, 8 at 15/16 (src: [[sources/fractalforums-pathfinding-revisited]]); the
  2-fold embedded Julia set at about `s^(3/4)` for a minibrot of size `s` (src:
  [[sources/muency-newton-raphson-zooming]]). KF's Newton-zoom presets encode the same:
  "Minibrot relative" fold factor 0.5 doubles the pattern, 0.75 quadruples, power `P` uses
  `1 − (1/P)^M`; "Minibrot absolute" 0.75 is "typically near an embedded Julia set";
  "Atom domain absolute" 1.125 is "sometimes near a Julia morphing" (src: [[sources/kf-manual]]
  Newton-Raphson zooming). F3 has the same window with power presets 2, 4, 8 … and a Capture
  button that sets the start depth (src: [[sources/fraktaler-3-site]]).
- **Evolution** (Dinkydau): double the shape, morph one copy, repeat with the morph as the
  new shape; the stages stay visible side by side and "the more steps taken, the better"
  (src: [[sources/dinkydau-evolution-journals]]). Building blocks named by Heiland-Allen:
  disk, tree, line, chosen by where in the shortest spiral arm the next target sits
  (src: [[sources/heiland-allen-inflected-julia-sets-2025]]). "Mandel-trees" / "Evolution of
  trees" are this: repeated morphs of a branch point produce tree-like embedded Julia sets
  (Dinkydau's series reached 1.1e449 with 70 M iterations; src: [[sources/deep-zoom-videos]]).
- **Off-centre next to a minibrot to double symmetry**: the measured version of the rules
  above. In our own places, centred on a minibrot found about `r` from a Misiurewicz point
  (size about `r²`), a k-armed point shows 4k arms at the geometric mean of `r` and the
  minibrot size and 8k at 3/4 of the way (measured by eye 2026-10-16,
  [[synthesis/places]]). The Misiurewicz point fixes the symmetry, the fraction the arm count.

### Tools

- **Newton-Raphson zooming** (Heiland-Allen, in KF2 and F3): from "an 'accurate enough'
  guess" it finds the nearest minibrot's period, nucleus, size and skew, and zooms to a chosen
  fraction of its depth without rendering; usually faster than zooming by hand, slower at high
  iteration counts, and it "does not choose a location for you" (src:
  [[sources/dinkydau-evolution-journals]], [[sources/kf-manual]]). KF's ball method for the
  period is the default, box otherwise; it fails fast if the iterate leaves the target
  (src: [[sources/kf-manual]]).
- **Heiland-Allen's automated Julia morphing.** 2016: extrapolate external angles block-wise
  (periods 5, 69, 143, 291 = `2p + 5`), trace each ray, Newton the nucleus; `O(p²)` per level,
  over 24 h for the last level, "not practical". 2017 (`m-ejs-tree-morph`): alternate Newton
  for Misiurewicz points (choose preperiod to go to a spiral or a tip) and Newton for nuclei
  along a spiral spine; guess the next tip from atom-domain coordinates scaled by a power of
  1.5; `O(p)`, period "more than doubles" per level; fails in a **symmetry trap** when Newton
  starts exactly on a basin boundary (src: [[sources/mathr-julia-morphing-posts]]). 2021: a
  circuit of morphs generated from angle prefixes, framed with `m-domain-size`, rendered in KF
  (same source).
- **Embedded Julia sets by angle**: islands in hairs have angles made of blocks of the parent
  and influencing island, "counting in binary"; embedded Julia sets split into halves with
  periods `p, p+3, p+6 …` doubling in count (src: [[sources/mathr-embedded-julia-navigation]]).
- **Domain maps**: atom domains (index of the minimum `|z_n|`) mark nearby nuclei and their
  period (src: [[sources/muency-atom-domain]]); atom-domain size
  `R_a = |F^q(0,c_0)| / |∂F^p/∂c|`; Misiurewicz domains (minimum `|z_{p+q} − z_q|`) mark
  preperiodic points; a periodicity scan over finer grids lists every nucleus in a view with
  period and size (src: [[sources/mathr-domains-and-periodicity-scan]]).
- **emndl_autotune** "attempts to find interesting points ... that might look nice": its
  output is a chain of nuclei of growing period (1, 4, 31; or 1, 5, 21, 115, 596); the scoring
  is not documented (src: [[sources/wikibooks-emndl]]).
- **Feature finders**: FractalShark right-click Feature Finder identifies the period and
  refines by Newton, optionally scanning a grid around the click for the best candidate, on the
  GPU; it found its built-in minibrot at 1e650452; its "autozoom" is experimental (src:
  [[sources/fractalshark-readme]]). Imagina: double click "Find nearby feature" (src:
  [[sources/imagina-readme]]).
- **Previews before the deep render**: ExploreFractals and Inflector Gadget simulate morphs
  by transforming the plane ("inflections") so the artist sees the shape before paying for
  the zoom; KF2's "Show Inflection" does the same (src: [[sources/dinkydau-evolution-journals]],
  [[sources/heiland-allen-inflected-julia-sets-2025]]).

No source describes a search that scores *beauty*. Full automation of "continue the
artist's pattern" is called open (src: [[sources/heiland-allen-inflected-julia-sets-2025]]).

## 2. Known deep places and how they are shared

| Place | Depth | Period / iterations | Shared as |
| --- | --- | --- | --- |
| Orson Wang record (2010) | 2.1e275 | | video only |
| Dinkydau "Evolution of trees" (2014) | 1.1e449 | 70 M iterations | video, magnification only |
| Maths Town "Eye of the Universe" (2017) | 3.4e1091, ends on a minibrot | ~17 M iterations | full Re/Im in the YouTube description |
| Dinkydau record (2013) | e1500 | | video |
| Spångberg 10 T iterations (2022) | ~e2107, minibrots at e520, e1041, e2095 | reference 435 M | video |
| Maths Town "Deepish" (2022) | 4.8e2565 (title says 4e2656) | 591 M | Re/Im on the maths.town page (too long for YouTube) |
| Maths Town "Infinite Relaxation" (2021) | 3e3284 | | video |
| Kalles "e10000" (2016; remastered 2021) | e10002 | 5 M | `.kfr` |
| Kalles "Final" (2016) | 1.28e20004 | 5 M | `.kfr` at chillheimer.de |
| Spångberg (2022) | e122768 | 100 G at the end | c is ~246,000 decimal characters, on request |
| FractalShark View #34 (readme, 2026) | 1e650452 | | built-in view / Imagina-compatible save file |

(src: [[sources/deep-zoom-videos]], [[sources/fractalshark-readme]],
[[sources/fractalforums-deepest-zoom-poll]]). A "Deepest Mandelbrot Zoom Ever E100000" video
(2015) is an April Fools' joke. FractalShark's author says "The Internet is full of nonsense
about 'deepest ever'", and Maths Town sets record conditions: finish on a minibrot, at least
4K, depth doubling no faster than once per second, coordinates published (src:
[[sources/deep-zoom-videos]]). Named deep *shapes* are series names given by their makers
("Evolution of trees", "Shape-Stack Overflow", "Eye of the Universe"), not a shared catalogue.
"Neutrino" was not found in any source (unverified).

**Sharing formats.** KF `.kfr` text files (`Re:`, `Im:`, `Zoom:`, `Iterations:`, `IterDiv:`,
palette, slopes; the e20004 file is 51 KB); F3 TOML-like parameter files with
`reference.period` ("Setting an inaccurate period is a good way to get corrupt images",
src: [[sources/fraktaler-3-site]]); FractalShark and Imagina save files that include the
compressed reference orbit (src: [[sources/fractalshark-readme]]). At depth 1e100000+ the
coordinate alone is 100k+ digits, so sharing moves to files, not descriptions.

**Where the deep places are.** Several famous deep zooms sit near the real axis, away from the
main body: Kalles e20004 at Re −1.9855 (antenna side), Deepish at −1.9416, the 2016 "Hardest
zoom" just right of the period-3 cusp at −1.75 (src: [[sources/deep-zoom-videos]]). Iteration
counts are modest for their depth when the path is cheap (5 M at e20004) and huge when it is
not (591 M at e2565, 750 M at e99).

## 3. What makes a deep place look good

- **Structure comes from the path, not the depth.** A deep view is interesting when the
  path to it made choices (morphs, doublings, evolution stages); a path that keeps doing the
  same thing (heading straight into a needle, or the self-similar centre of a Misiurewicz
  point) repeats the same picture (src: [[sources/fractalforums-pathfinding-revisited]];
  our M(24,2) / M(4,1) finding in [[synthesis/places]]). simon.snake's E623 "quite easy
  but boring location on the far left" is the same complaint
  (src: [[sources/fractalforums-deepest-zoom-poll]]).
- **Density is iteration-count gradient against palette period.** Near a minibrot of period
  `p`, each ring adds about `p` iterations (inference from the period-`p` return of the orbit;
  not stated by a source). The colour per pixel changes by `|∇n| / palette_period`; once that
  passes about a cycle per pixel the image is noise or, with almondbread's calm-dense colouring,
  a flat fill ([[techniques/coloring-methods]] "Dense regions"). Our 1.4 M-period needle
  minibrot is that case (inference: with `n` jumping ~1.4 M per ring and palettes cycling
  every tens to hundreds of iterations, every ring is thousands of cycles). Low-period
  minibrots near low-preperiod Misiurewicz points (periods 2,659-17,046 at 1e1500-1e3007 in
  our places) keep the gradient small.
- **What experts do about density.**
  - KF "Divide iteration": "for dense images this value can be greater than 1"; transfer
    functions log, square root, cube root, LogLog, "Stretched" (min to max in the image),
    ATan (src: [[sources/kf-manual]] Colors dialog). Kalles raised the iteration division three
    times near the end of the e20004 zoom "to avoid that all get into a noisy blurr" (src:
    [[sources/deep-zoom-videos]]).
  - Histogram colouring (F3 3.1) and F3's iteration and DE histograms for auto-unstretch
    (src: [[sources/fraktaler-3-site]]).
  - Colouring that does not depend on the count: distance estimate (linear, sqrt, log) and
    slopes; KF's slope implementation was rewritten to be "independent of zoom level and
    iteration count", and "Deep views requires a couple of magnitudes higher value" of slope
    power (src: [[sources/kf-manual]]).
  - Two layers: Maths Town's "Eye of the Universe" blends a broad colour pass with "a more
    dense brightness layer" from KF's wave function (src: [[sources/deep-zoom-videos]]).
  - The density itself is sometimes the attraction: "the extreme density of concentric
    circles around the minibrot" (Fractal universe e10000), "Shape-Stack Overflow | High
    density Mandelbrot zoom" (src: [[sources/deep-zoom-videos]]). Dense is not wrong if
    resolved; a flat fill is.
- **Iterations and blur.** Too few perturbation iterations give "blobby spiral centers" and
  soft minis; F3 recommends raising the limits then (src: [[sources/fraktaler-3-site]]).
  Interior blobs are cheap with interior checks ([[techniques/interior-detection]]).

## 4. Algorithms almondbread could use

Ordered by value per effort. Costs from our runs: reference for periods 2,660-17,047 at
5,120-10,176 bits took 49-939 ms; GPU frames 5-17 ms at those places (measured 2026-10-16,
M1 Pro, release, [[synthesis/places]]). Reference cost grows about linearly with period and
with bits squared-ish (inference), so a 1.4 M-period target is minutes, not ms.

1. **Frame by fraction** (cheap, high value). Given a start (Misiurewicz point or nucleus)
   and a target minibrot of size `s` at distance `r`, put the view at
   `exp((1−f)·ln r + f·ln s)` for `f ∈ {1/2, 3/4, 7/8}` and compare the arm counts; what we did
   by hand for the four deepest places. Same as KF/F3 "Minibrot relative".
2. **Score a thumbnail before accepting** (cheap). Render 160×100 and compute: interior
   fraction, luma range (we already reject < 40), the share of pixels where the palette phase
   changes more than 1/4 cycle per pixel (density; the calm-dense mask), edge entropy, and
   rotational symmetry (autocorrelation over k-fold rotations about the centre). Reject flat
   and fully dense views; prefer high structure with density below a threshold. None of the
   sources does this (no published scorer); it is our design.
3. **Period budget.** Prefer targets whose period keeps the reference under a time budget
   (e.g. 1e5 for interactive "go to"); measure, do not guess. Pick low-preperiod Misiurewicz
   starts (M(k,p) with small k+p) and minibrots about `r` from them (size ~`r²`), which keep
   periods in the thousands at 1e3000.
4. **Auto colour scaling by period.** When the view is centred on a known period `p`, set the
   iteration divisor (palette period in iterations) from `p` or from the measured `|∇n|`, or
   switch to log / histogram / DE colouring, instead of fading to a flat fill. KF does this by
   hand ("iteration division"); doing it per view is our design.
5. **Julia-morph walker** (medium). Extend `locate --walk`: after a minibrot, choose the next
   target *inside the current shape* (morph) or *outside* it (doubling), with Newton for
   Misiurewicz points between nuclei as in `m-ejs-tree-morph`. Start Newton off the symmetry
   centre to avoid the symmetry trap (src: [[sources/mathr-julia-morphing-posts]]); this is a
   likely reason our Newton failed at the M(24,2) centre ([[synthesis/places]]), unverified.
6. **Periodicity / domain scan of a view** (medium). Atom domains and Misiurewicz domains on
   the GPU over the perturbed orbit give a candidate list (period, preperiod, approximate
   position) per view; Newton refines the few best; atom-domain size frames them (src:
   [[sources/mathr-domains-and-periodicity-scan]]). Feeds the minibrot atlas
   ([[decisions/minibrot-atlas]]).
7. **External-angle navigation** (expensive). Angle extrapolation is `O(p²)` per level by ray
   tracing (src: [[sources/mathr-julia-morphing-posts]]); only worth it for exact,
   reproducible sequences.

## What almondbread should build

We have: `almondbread locate` (ball period + Newton nucleus, minibrot size and angle,
precision raised to the size), `--misiurewicz k,p` (Newton with lower preperiods divided out),
`--walk k --toward --shrink` (nested minibrots, period strictly increasing). We lack: any
notion of *where on the way* to stop, any check that the result looks good, a Misiurewicz step
between nuclei, and colouring that adapts to period.

1. **`locate --frame f`** and a `--frames 1/2,3/4,7/8` contact sheet: print and render the view
   at fraction `f` of the log-distance from the start to the found minibrot. Smallest change,
   makes every place we already have reproducible by command.
2. **Thumbnail scorer** in core (`place_score`: structure, density, interior share, k-fold
   symmetry) used by `locate`, the places checks and later the autopilot. Reject flat output
   like the 1.4 M needle minibrot automatically.
3. **Period-aware colouring**: an automatic iteration divisor from the reference period or the
   measured gradient, so dense rings render as rings instead of a calm fill; compare against the
   current calm-dense look on the needle minibrot and `star-1e2000`.
4. **Morph walker**: `locate --morph inside|outside` alternating nucleus and Misiurewicz Newton
   steps, starting off-centre; a period cap (default 1e5) and a time cap per step.
5. **View scan**: atom-domain and Misiurewicz-domain periods from the GPU pass as candidates;
   needed for the atlas and for "find something interesting near here".

Not worth building now: external-angle tracing (quadratic in period), a "deepest ever" target
(our depth limit is about 1e3011, [[synthesis/places]]; records are 1e122768 and a minibrot at
1e650452).

## Contradictions

- Maths Town's "Deepish" title says 4e2656, its own location field 4.771479e2565 (src:
  [[sources/deep-zoom-videos]]). Use the field.
- Mu-Ency reads `s^(3/4)` as both the 2-fold and the 4-fold embedded set
  ([[sources/muency-newton-raphson-zooming]]); Dinkydau and the forum put a doubling at 3/4 and
  a morph at 1/2 of the log-depth. Our measurement (4k arms at 1/2, 8k at 3/4 around a k-armed
  point) is consistent with "doubling per halving of the remaining distance" if the first
  doubling happens before 1/2; not resolved, see [[questions/julia-morph-depth-fractions]].
