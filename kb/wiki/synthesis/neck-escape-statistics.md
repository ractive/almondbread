---
title: The −3/4 neck gap at 1e8–1e9 (escape statistics, what is true)
type: synthesis
date: 2026-10-17
sources: ["[[sources/klebanoff-pi-in-mandelbrot]]"]
tags: [neck, escape-time, ground-truth, max-iterations, precision, known-hard-case]
---

# The −3/4 neck gap at 1e8–1e9

A known hard case: the gap between the main cardioid and the period-2 disc just above the
−3/4 neck, around `c = −0.74999996 + 0.000369183 i`, zoom 1.9e8–1.3e9 (HUD), turned −3.2°.
The user saw an almost black view with thin filaments that "extend" and areas that "appear out
of nowhere" while zooming, also with a 10 M limit (2026-10-17). This page records what the set
really looks like there and why the app showed what it showed.

## Geometry

Near the neck the cardioid is `re ≈ −3/4 + (3/4) im²` and the period-2 disc
`re ≈ −3/4 − 2 im²` (our expansion of `c(t) = e^{it}/2 − e^{2it}/4` at `t = π` and of
`|c + 1| = 1/4`). At `im = 3.69e−4` the gap is 3.7e−7 wide; the user's views are 1e−8 to 4e−8
wide, well inside it. No interior of either component is in view. The neck escape count is
`N ≈ π/ε` for `c = −3/4 + iε` (src: [[sources/klebanoff-pi-in-mandelbrot]];
[[techniques/pi-in-the-mandelbrot-set]]): about 8 500 here, and that is the *minimum* escape
count in these views (measured 8 516–9 463), not a typical one: an orbit passes the slow
parabolic gate near `z = −1/2` many times before it escapes.

## Ground truth (measured 2026-10-17, M1 Pro, release)

Tool: `cargo run --release -p almondbread-core --example neck_truth -- --center re,im --scale s
--size 160x100 --angle -3.2 --limit 1e7 [--dd k] [--shift e]`: direct f64 iteration, multithreaded,
an interior proof (back within 1e−10 of a saved point with a contracting map since), escape
histogram by powers of two, `log10 |dz/dc|` at escape, an `.iter` dump in the oracle encoding,
and a double-double re-run of a sample. `--paint a.iter [--cycle p]` paints any dump
(also `verify --dump`) in the same colouring, for side-by-side looks.

The four views, 160 × 100 framing the 2000-point window, limit 10^7:

| zoom (HUD) | escaped | proven interior | not escaped by 1e7 | min n | p10 | median | p90 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1.9e8 | 99.81 % | 0.02 % | 0.17 % | 8 516 | 17 024 | 42 550 | 263 805 |
| 3.4e8 | 99.45 % | 0 | 0.55 % | 8 554 | 25 536 | 102 001 | 629 855 |
| 6.7e8 | 97.94 % | 0 | 2.06 % | 8 662 | 68 044 | 297 797 | 1 889 212 |
| 1.3e9 | 94.19 % | 0 | 5.81 % | 9 463 | 59 495 | 433 570 | 3 712 003 |

- **Everything escapes** (but for a few tiny minibrot pixels). The picture is a saddle: two
  crossing smooth gradients of the escape count, a near-straight **seam** across which the
  count jumps (one more passage through the gate), and a disc whose pixels escape after
  2e7–7e7 iterations (a 16 × 16 probe of the disc: all escape by 4.8e7, median 3.6e6 at
  1.2e−10/px; 8 × 8 near its centre at 4e−11/px: 2.0e7–7.4e7). The user zooms towards that
  disc, so typical escape counts grow 10× between 1.9e8 and 1.3e9.
- **Escape counts are chaotic below the pixel.** `|dz/dc|` at escape is 10^13–10^50
  (median 10^15–10^17.5), against 10^10 per pixel: each pixel contains many escape bands, and
  the count at a pixel centre is a sample of the local distribution. f64 itself is not exact
  per pixel: double-double from the same `c` gives a different count for 9 % of escaped pixels
  at 1e6 (some by a whole passage, 323 945 vs 545 227), and double-double at `c` vs `c + 1e−20`
  differs for 2 %, at `c + 1e−26` for 0.4 %, at `c + 1e−30` for none. Only statistics and the
  smooth large-scale structure are meaningful; per-pixel counts are not reproducible by any
  renderer at affordable precision.
- **Cost of the truth.** Mean iterations per pixel are 10^5–10^6, so a resolved frame at the
  window's 4000 × 2500 pixels needs 10^12–10^13 iterations: minutes to tens of minutes of GPU at
  the plain kernel's 6–12e9 it/s. An auto limit of 4 000–18 000 leaves 90–99 % of the pixels
  unresolved, and that is correct behaviour for the limit, not an error.

## GPU (float32 perturbation) against the truth

`verify --iterations 1e6` at `−0.74999996120361811, 0.00036918279150454967`, 400 × 250, 1e−11/px,
and `--iterations 1e7` at 200 × 125, 2e−11/px, both against the f64 oracle (same grid):

| limit | escape histogram, GPU vs f64 (per 2^k bin) | class mismatch | escape n differs | median |Δmu| |
| --- | --- | --- | --- | --- |
| 1e6 | 2^16 8.98/8.96 %, 2^17 14.69/14.75 %, 2^18 15.27/15.33 %, 2^19 13.32/13.25 %, unresolved 42.40/42.39 % | 1.57 % | 27 % | 0.31 |
| 1e7 | 2^20 11.16/11.16 %, 2^21 8.91/8.93 %, 2^22 10.01/9.88 %, 2^23 2.22/2.22 %, unresolved 9.02/9.17 % | 0.74 % | 60 % | 17 |

The histograms agree to sampling noise, and the pictures (painted the same way, cyclic period
50 and 2 000 iterations) are indistinguishable, including the seam and the noise above it. The
per-pixel differences are the chaos above, not float32 error: f64 differs from double-double
in the same way. **The GPU is right at 1e6 and 1e7 here.** No float-float path is needed for
this region. The **straight seam** of the user's `render --iterations 1e6/1e7` (stripes below,
noise above) is in the f64 oracle too: real structure. Stripes are where the escape count varies
smoothly (one passage count across the region, the palette cycles through it); noise is where
neighbouring pixels differ by whole passages.

## Why the app showed a black view

**Auto iterations (cap 10 M).** The depth heuristic gives 3 800–4 600 at these zooms, below the
minimum escape count, so the first level is 100 % unresolved. The learner's retry renders
again at 4×, 15 000–18 000; the learned limit is capped at 4× the depth heuristic, so the
user's 10 M cap never binds. The extension passes need the unresolved list: at 4000 × 2500,
9.7 M unresolved pixels overflow the 4 M-entry list (all-or-nothing), so there are no passes
and no `escapedPast` signal. Traced (`ALMONDBREAD_TRACE=1 verify --history jump ... --size
4000x2500 --cap 10000000`): `learn: soft 3831 ... crossed 10000000 -> 15324 (render again)`,
then `soft 15324 ... crossed 9709680 escapedPast [0, 0, 0, 0] -> 15324`: 96.6 % unresolved,
drawn in the interior colour. At 1600 × 1000 the passes run (to 57 008 in the 2 s budget,
`escapedPast [949617, 949617, 902845, 329572]`), and the learner still stops at 4× depth
(14 252). The shown frames match the f64 oracle at their hard limit (`verify --history zoom`
ending at 1.3e9: class mismatch 0.002 %, median |Δmu| 9e−4; 98 % unresolved at 17 676).

**Manual 10 M.** The limit reaches the kernel (`maxIter 10000000`), and the f64 reference runs
to the centre's escape at 7 283 202 (escaped: exact reach unbounded, no extension with a manual
limit), the same with and without `f77cf7b`. Two things kept the view black:

1. *Local SA seeds stalled the level* (bug, fixed 2026-10-17). A seeded level admits no pixel
   until every seed node is final, and nodes run until their expansion breaks or the limit. In
   the striped regions the expansion stays valid for 10^5–10^6 iterations, at ~256 steps per
   node per round: after 20 s at 4000 × 2500, 210 000 of 830 000 nodes were still running and
   not one pixel chunk had run (snapshot: all black). Seeds now stop at
   `SEED_HORIZON` = 65 536 iterations ([[techniques/local-series-approximation]] "Seed
   horizon"); the same run seeds in 224 rounds (~3.5 s) and pixels iterate from then on.
2. *The frame is a long render in progress.* 10^13 iterations at 4000 × 2500 take an estimated
   10–20 minutes. Pixels are admitted tile by tile into 1 M live slots, and each slot is held
   by a pixel for up to millions of iterations, so the shown frame has a few early escapes (the
   n ≈ 8 600 filament) ending where admission has reached so far: the "filament ending
   mid-screen with a rounded tip". Zooming out lowers the counts, so the rest appears.

**"Areas appear out of nowhere."** Expected with a limit far below the escape counts: each
limit step (depth heuristic, 4× retry, extension passes) reveals a band of the heavy tail, and
the zoom path heads into slower and slower escapes. No reference change or cut-off extension
was involved (shown frames match fresh renders and the oracle at their hard limit).

## Open: what the app should do here

- Escalating the auto limit towards the user's cap costs 10 s–minutes per ×4 step at full
  resolution: only as an explicit or idle background refinement with progress, cancelled by
  input. The extension list's all-or-nothing rule blocks it at Retina sizes; passes over the
  list in segments or per tile would lift that.
- Unresolved pixels are drawn in the interior colour mixed 0.5 % towards grey
  (`mix(edge, 0.5, 0.005)`), indistinguishable from interior; pending pixels of a frame in
  progress look the same. See [[techniques/auto-iterations]] and the UI proposal in the log.
