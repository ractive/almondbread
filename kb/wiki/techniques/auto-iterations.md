---
title: "Automatic maximum-iteration control"
type: technique
status: draft
sources: ["[[sources/kf-manual]]", "[[sources/fraktaler-3-site]]", "[[sources/wikibooks-fraktaler-3]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/fractalforums-interior-detection]]"]
tags: [max-iterations, heuristics, interaction]
---

# Automatic maximum-iteration control

Too low a maxIter turns escaping pixels into false interior (solid blobs, flat
spiral centres, blurry minibrots); too high wastes time on true interior
pixels. Depth alone does not set the right value: near a minibrot of period
`p`, escaping pixels need several hundred times `p` iterations (GBy: 1e7
iterations for a period-14292 minibrot)
(src: [[sources/fractalforums-interior-detection]] §Interior detection for deep Mandelbrot set zooms).

## What renderers do

- **KF2+** "Use auto iterations": on by default, algorithm undocumented. It
  analyses rendered images (it does not work in "Save KFR" mode, where nothing
  is rendered). Known bug: zoom-out sequences get black regions because maxIter
  is reduced too much before spirals appear in the next frame; workaround:
  disable it. It was refactored with an issue left unresolved; Newton-Raphson
  zooming raises maxiters "more suitably" (src: [[sources/kf-manual]] §Use auto iterations).
- **Fraktaler 3** has no auto mode; it splits the limit into three caps:
  total iterations (can be "arbitrarily high without too much slowdown"),
  maximum reference iterations (normally equal to iterations; too low corrupts
  images; can be locked to the period after Newton zooming for a speedup), and
  maximum perturb iterations (a few thousand; raise if spiral centres are blobby
  or minis not sharp), plus maximum BLA steps (a few thousand; raise for glitchy
  areas). Limits exist partly to avoid GPU-driver timeouts. Iteration-count
  histograms help inspect limits; keypad +/− doubles/halves the count
  (src: [[sources/fraktaler-3-site]]). A saved example uses 262144 iterations and
  4096 perturb iterations (src: [[sources/wikibooks-fraktaler-3]]).
- **Chéritat**: with interior detection, a huge N barely costs anything
  ("N = 10⁶ is OK, but avoid N = 10¹⁰")
  (src: [[sources/cheritat-mandelbrot-rendering]] §Images and speed).
- mathr's own renderer reports per-image statistics (mean/min/max/stddev of
  steps, BLA steps, perturbation steps, iterations, max reference iteration
  accessed, rebases by cause) to diagnose limits that are too small ("blobby"
  false interior) or too large
  (src: [[sources/fractalforums-interior-detection]] §Reply #1).

The useful idea in Fraktaler: once each skip covers many iterations, the cost
is in **steps**, not iterations, so a step cap (perturb steps + BLA steps)
bounds work while the iteration cap can be very high (inference from
[[sources/fraktaler-3-site]]).

## Heuristic for almondbread (inference, not a published method)

Needs derivative interior detection ([[techniques/interior-detection]]), so that
"reached maxIter" means "undecided", not "interior".

```text
after each completed frame (or coarse refinement level):
    esc  = iteration counts of escaped pixels
    n99  = percentile(esc, 99.9)
    unres = fraction of pixels that hit maxIter and were NOT marked interior
    if unres > 0.001 and n99 > 0.5 * maxIter:
        maxIter = max(2 * maxIter, 4 * n99)          // grow fast
    else if unres == 0 and n99 < maxIter / 16 and not zoomingOut:
        maxIter = max(maxIter / 2, 8 * n99)          // shrink slowly, with margin
    never shrink during zoom-out (hysteresis)        // fixes KF's black-region bug
    if reference is a nucleus of period p: allow maxIter >> refLen (rebasing wraps)
    else: cap maxIter at the reference length
```

- Zoom-based formulas (`k·log(zoom)`) underestimate near minibrots, because the
  minimum escape count scales with nearby periods, not depth.
- Use the coarse refinement pass as the sample, so maxIter adapts before the
  full-resolution pass.
- Expose a step cap per dispatch for the watchdog
  ([[platform/gpu-watchdog-and-chunking]]), separate from maxIter.

Open: thresholds and whether to use per-tile statistics; see
[[questions/auto-iteration-heuristic]].

## As built in almondbread (2026-10-14)

From the −3/4 neck wedge report (measured 2026-10-14, M1 Pro, release; numbers in
[[synthesis/benchmarks]]).

- **Unresolved is not interior.** A pixel that reaches the limit without being
  proven interior by the derivative test ([[techniques/interior-detection]]) is
  stored as *unresolved* (iteration code −3). It was drawn as the interior colour
  lifted 3 % (linear) towards neutral grey, then (2026-10-17) as a flat dark grey veil;
  since 2026-10-17 it is fog, lit by the resolved pixels around and almost black further
  in ([[techniques/unresolved-fog]]). Most such pixels are
  slow-converging interior near parabolic points. Without the test (the adaptive
  switch turned it off) nothing can be told apart, and limit pixels are drawn as
  interior as before.
- **Extension passes.** After the full level completes, the unresolved pixels
  (scanline indices collected by the kernel, one atomic per threadgroup) are
  re-iterated *from the start* in background passes to 4× (or 2×) the previous
  limit, up to 16× the nominal limit, the iteration cap and an estimated 2 GPU
  seconds per view. Each pass collects what is still unresolved for the next.
  A pass is never cut short, because the list is in scanline order and stopping
  inside it left a straight horizontal edge. An extension is all-or-nothing for
  the same reason (a view with more unresolved pixels than the list holds gets
  none). See [[platform/gpu-watchdog-and-chunking]] for why the passes restart
  instead of keeping state. The passes need a reference orbit that reaches as far
  as they go, or one that escaped: an orbit cut at the nominal limit made them
  draw noise near the −3/4 neck (fixed 2026-10-16; [[techniques/rebasing]]
  "Truncated references").
- **Learned limit.** When the passes found escapes past the nominal limit in at
  least 1 % of the pixels, later views start from the smallest of 2× or 4× the
  limit that leaves fewer than 1 % escaping beyond it. The learned limit is capped
  at 4× the depth heuristic and at the iteration cap. It decays by 0.7 per completed
  view that has no unresolved pixels and no escape above a quarter of the limit.
  It applies from the *next* view on: the view it was learned at is already
  resolved by the passes, and re-rendering it would repeat the work.
- **Why not "cover the maximum"**: near parabolic points escape counts have a heavy
  tail (`n ≈ π/|c − c₀|` at the neck). At the neck (zoom 9 000, limit 1 703) the
  fraction of pixels escaping beyond 1×, 2×, 4× and 8× the limit was 3.4, 2.3, 1.4
  and 0.56 %, roughly halving per doubling. "Cover every escape" ran to the 200 000
  cap within three views, and "cover all but 0.05 %" did too. The neck at 7e6
  (auto limit 2 907) still has 29 % of pixels escaping beyond 33 808, so no
  affordable limit resolves it. The passes plus the unresolved colour are the
  answer there.
- **Depth-only limits make the picture jump.** In the user's report, one wheel
  notch (7.06e6× → 9.34e6×, auto 2 907 → 2 981) made a large region appear. Those
  were pixels escaping between the two limits. With the passes, both views show
  the same structure (`make regress` runs exactly that step).
- **Nothing escaped** (bug found 2026-10-14 after M3b). At `sea-8007` (period-8007
  minibrot, 2e-34/px) the depth heuristic gives 29 575, and every escape around the
  minibrot comes later. All 576 000 pixels were *unresolved*, none escaped and none
  was proven interior, so the view was flat unresolved grey (RGB ≈ 0x0a). It was not false
  interior: the kernel classified the pixels correctly. Two gaps let it stay that way.
  (1) The learner only grew on escapes *past* the limit, and the passes never ran:
  `render` has no passes (offscreen extension ×1), and at full Retina size the
  7.5 M unresolved pixels overflow the 4 M-entry list (all-or-nothing) and the 2 s budget.
  So there was no signal. (2) The "applies from the next view" pin kept the view at its
  limit. Fix: when at least **25 %** of the pixels are unresolved (with the derivative test
  on, not a gesture frame) and no pass looked as far as 4× the limit, the limit becomes
  4× the nominal and the **same view renders again**, with no pin. The scheduler keeps
  refreshing until it has, and offscreen renders loop up to 3 times. The 4×-depth cap
  bounds it: a view whose unresolved pixels are interior settles after at most one extra
  render. Results (measured 2026-10-14, M1 Pro, release, 960×600, `render`): sea-8007
  29 575 → 118 300, 0.26 % unresolved, rings of seahorses visible (0.75 % at 59 150,
  0.21 % at 200 000). A full-screen "Go to coordinates" jump (3456×2160) renders the
  structure at 119 844. Inside the minibrot (1e-36/px) the first render is also all
  unresolved: the test needs a few periods. The ×4 render proves every pixel interior and
  the limit decays afterwards. Inside the main cardioid nothing changes (all proven at
  the depth limit). Side effect, CLI `render` only: necks, the cusp and Feigenbaum are
  56–100 % unresolved at the depth limit and now render once more at 4× (neck-7e6
  2 831 → 11 324, 60 → 190 ms GPU). The app's refined views have the passes and do not
  hit the rule there. Regressions: `make regress` "jump" sessions
  (`verify --history jump --location sea-8007 --max-unresolved 1`, with and without
  `--extension-budget 0`), Metal tests `auto_iterations_nothing_escaped` and
  `auto_iterations_interior_view`.

## Floor from the reference (M5, 2026-10-15)

The user's slowest view (`spiral-32412-1e19`: a dense spiral at 4.2e−19/px around a disc of
pixels that need more than 41 536 iterations) spent 37 % of its 4.43e10 iterations on a
first pass that left every pixel unresolved at the depth limit 10 384 before the 4× retry
(3 350 ms GPU in all, driver's measurement on main). The view's big reference knows better
before any pixel runs: its centre orbit escaped only at 32 411 (the "ref 32412 pts" of the
report: not a nucleus period; ball period detection finds period 2 235 and Newton does not
converge, so there is no nucleus in view). Rule (`period_floor` in the scheduler, a function
of the reference alone, so the job's key is stable):

- reference = nucleus of period P found in the view: limit ≥ 1.25 P;
- centre orbit (no nucleus) that escaped after the depth limit, or did not escape within its
  length (2× the limit it was made for): limit ≥ 4× the depth limit (the retry level);
- capped at 4× the depth limit and at the user's cap; auto iterations only; nuclei are now
  searched up to that cap (`period_limit`), not only to the current limit.

A high limit costs only the pixels that reach it, so the floor is cheap where most pixels
escape earlier. Spiral, 1600 × 1000, plain kernel (measured 2026-10-15, M1 Pro): one pass at
41 536, 2 257 ms instead of two (10 384 + 41 536). (The switch `ALMONDBREAD_ITER_FLOOR=0` was removed 2026-10-17.)

**The adaptive interior-test switch must not drop the test while pixels end unresolved**
(M5): with the test off, limit pixels are drawn interior and the extension passes (which need
the test) never continue them. After a pan at the spiral the switch turned the test off (few
interior pixels, many iterations each) and 2 % of the pixels, the disc of slow escapes, were
painted interior (`verify --history pan`). The test now stays on while ≥ 0.1 % of the last
level's pixels ended unresolved (`UNRESOLVED_KEEPS_TEST`).

**Learned state does not survive a jump** (M5, found by the flight benchmark): after a dive at
c = i (1e−280) and a jump to the spiral, the dendrite's statistics (no interior, so the
interior test off) drew the spiral's unresolved disc interior, and the learner could not
retry without the test: an all-black settled frame in 2 of 3 runs, 0.6 % class difference
from a fresh render in the third. Now a view change that is not continuous (scale ratio ≥ 8
or the centre two view widths away, between consecutive refreshes) resets the learned limit,
the pin, the interior-test statistics and the local-SA gate's cost estimate; and a view where
a quarter reached the limit without the test renders again with it (a retry epoch in the job
key, also when the limit cannot grow). `verify --history` checks the settled frame against a
from-scratch render (own auto iterations, no learned state), and `make regress` runs the
dive-then-jump path (`--prior 0,1,1e-280 --prior-speed 40 --prior-seconds 0.8`).

**The learned limit survives anti-aliasing** (checked 2026-10-16). A report said the learned
limit reset after the AA pass. Not reproduced: AA sample levels are complete when created and
`finish_level` returns before the learner for them, so they never change the learned state;
neither does anything after the view settles. Checked headless with the app's defaults (9-sample
AA): the neck regression session with `--ss 3` (learned 8 216 → 8 472 across the AA and the
`--then` step), the dive-then-jump to the spiral, and Metal test `learned_limit_survives_aa`
(Host at the neck: limit learned above the depth limit, unchanged by AA, 100 idle refreshes and
a from-scratch renderer at the view; the next view after a pan renders at it). The trace line
behind the report (flight benchmark, below) is the *jump check's* fresh renderer, which has
its own learner and starts at 0, printing `learn: soft 41536 ... -> 0` right after the live
renderer's AA; the live renderer's limit was unchanged.

## Where the learner stops: the neck gap at 1e8–1e9 (2026-10-17)

The learned limit is capped at 4× the depth heuristic whatever the user's cap: with a cap of
10 M the −3/4 neck gap at 1.9e8–1.3e9 settles at 15 000–18 000, while its escape counts have
a minimum of ~8 500 and medians of 42 000–434 000 ([[synthesis/neck-escape-statistics]]).
At Retina window sizes (4000 × 2500) the ~9.7 M unresolved pixels overflow the 4 M-entry list,
so no extension pass runs and `escapedPast` stays zero; at 1600 × 1000 the passes reach 57 008
in the 2 s budget and the learner still stops at 4× depth. Lifting the cap would not be cheap:
a resolved frame there needs 10^12–10^13 iterations (minutes of GPU). Escalation towards the
user's cap belongs in an explicit or idle background refinement with progress, and the
unresolved colour should differ from interior (done 2026-10-17: [[techniques/unresolved-fog]]).

## Compute further (2026-10-17)

The idle background refinement the section above asked for. When a refined view's levels and
extension passes are done and at least `further_min_share` (0.1 %) of its texels are still
unresolved, the scheduler keeps iterating exactly those texels (`scheduler.rs`, `Further`):

```text
reached = limit of the last pass, u = unresolved texels
loop:
    stop "finished" if u < 0.1 % of the texels
    ceiling = max(view limit, Upper limit)          # as extend_to; a manual limit is its own
    stop "upper limit" if reached >= ceiling
    next = min(4 reached, ceiling)
    if next > exact_reach of the reference:
        f64 orbit cut at its limit -> recompute it to next (<= 16 M points) on a thread, retry
        else next = exact_reach, or stop "orbit length"
    next = 2 reached if 4x does not fit the GPU budget; stop "time limit" if 2x does not
    for each segment of <= list capacity admission slots (tiles in priority order):
        GPU scan lists the segment's ITER_UNRESOLVED texels (`unresolvedList` kernel)
        an extension level (list mode 2) iterates them from the start to next
    reached = next, u = what the segments left
```

- **Segments lift the all-or-nothing list.** The full level's own unresolved list is
  all-or-nothing (4 M entries): at 4000 × 2500 the neck's 10 M unresolved texels got no
  extension pass at all. A scan of the iteration texture per segment of at most the list
  capacity always fits; the segments follow the tile priority order, so the centre fills first.
- **Budget**: all extension passes of a view at most `further_budget` = 60 GPU s, estimated
  as `u × next / rate` per pass (pessimistic: every pixel to the limit), with the passes' own
  measured iterations per GPU second. The throughput model's rate read 10–30x too high after
  levels whose pixels are proven early (1e7 neck pixels to 65 k looked like 0.3 s).
- **Smoothness**: compute further's chunks are as small as AA samples' (8 ms, two in flight),
  so input waits at most ~16 ms of GPU; any input starts a new job and drops all of it. Gesture
  frames that start while it runs (4000 × 2500 neck, cap 10 M): 27–36 fps against 28 fps with
  it off (the gesture itself is that heavy; run-to-run noise 28–60 fps both ways).
- **Longer f64 orbits**: an f64 reference cut at its limit (centre not escaped) is recomputed
  longer on a thread and swapped into the job; it is bit-identical up to the old end (same
  arithmetic), so levels done stay exact. Later segments iterate without BLA/LA (their tables
  belong to the old orbit; f64 views rarely use them). A `verify` of the result: identical to a
  fresh offscreen render to the same limit (0 of 100 000 pixels differ). Big (deep) references
  are not extended: compute further stops at their `exact_reach` (they are usually escaped or
  periodic, so unbounded).
- **Not learned from**: the learner sees only the base level and the first extension passes,
  so the next view (a pan, a wheel step) starts at the fast base limit again.
- **HUD**: "computing further · 1.2 M of 4 M steps · 12 % not finished · ~40 s" while it runs;
  once settled with 0.1 % or more left, "3.8 % not finished at 200 k steps (upper limit)" (or
  "time limit", "orbit length"); nothing once finished.
- **AA first.** Compute further waits for the view's AA samples (they take a second or two, it
  can take a minute), then a GPU pass marks the texels still unresolved (one bit each) and its
  passes compose in a mode of their own: a marked texel fades to its colour once resolved,
  every other pixel keeps its anti-aliased history. Pixels it resolves carry their centre
  sample only, as extension-resolved pixels always did (AA samples iterate to the nominal
  limit and skip unresolved samples). AA done (M1 Pro, 1600 × 1000): neck 2.3 s on main,
  1.9–2.3 s with compute further; seahorse 0.30 / 0.30 s; home 0.33 / 0.35 s. The `verify
  --history` check "resolved pixels in the unresolved colour" was 0 on home, seahorse and the
  neck with `--ss 3`, and 14 % when the mode is disabled on purpose. With the fog it became
  "pixels resolved by compute further not showing their colour" (by the field data against a
  fresh render; [[techniques/unresolved-fog]] "Checks"), which found the last pass's texels
  left part faded; the compose now runs on for 4 τ after the last pass.
- Switches: always on in the app (the env switch `ALMONDBREAD_FURTHER` and `--further 0|1` were removed
  2026-10-17); `--further-budget s` on
  `verify --history` (which runs it only when given, so `make regress` stays as fast: 302.8 s against
  300.2 s on main, the 2.4 s being its one compute further session, the neck at 400 × 250 with
  AA and a 4 s budget) and `progress`; `ALMONDBREAD_UNRESOLVED_AT=1,5,20,60` prints the shown
  frame's unresolved texels at those seconds.

**The 4x depth cap stays.** With compute further it is not what limits a still view any more;
lifting it would make the *base* limit of every neck view, and every frame of a pan, slower:
the pass from 16 k to 65 k alone took 1.4–1.8 GPU s at 1600 × 1000 and ~3 s per 4 M-texel
segment at 4000 × 2500. Keeping it, the first complete frame stays at 61 ms (1600 × 1000) and
the deeper work is incremental and interruptible.

Measured 2026-10-17, M1 Pro, release, `verify --history jump --extension-budget 2` (the app's
extension budget), view `−0.74999994719705121 + 0.00036918485351065104 i`, 4.48e−12 per pixel
(zoom 4.2e8), unresolved share of the shown frame:

| size, Upper limit | further | 1 s | 5 s | 20 s | 60 s | settles |
| --- | --- | --- | --- | --- | --- | --- |
| 1600 × 1000, 200 k | off | 100 %* | 15.2 % | 15.2 % | 15.2 % | 3.0 s at 65 k |
| 1600 × 1000, 200 k | on | 100 %* | 3.78 % | 3.78 % | 3.78 % | 3.6 s at 200 k |
| 1600 × 1000, 10 M | on | 100 %* | 6.57 % | 0.06 % | 0.06 % | 14 s at 8.3 M |
| 4000 × 2500, 200 k | off | 100 % | 100 % | 100 % | 100 % | 1.4 s at 16 k (no pass) |
| 4000 × 2500, 200 k | on | 100 % | 60.2 % | 12–22 % | 6.94 % | 24 s at 200 k |
| 4000 × 2500, 10 M | on | | | | | 75 s at 10 M, 0.064 % |

(* the preview or first extension pass still running.) Against the ground truth (`neck_truth`,
160 × 100 of the same window): unresolved by 65 536 15.1 % (GPU 15.2 %), by 200 000 ~3.8 %
(GPU 3.78 %), by 8.33 M 0.11 % of 16 000 samples (GPU 0.059 %). Against the CPU oracle at the
reached limit: class mismatch 0.020 % at 8.3 M, all of it chaos (GPU escapes where f64 does not
and the reverse in equal shares, see [[synthesis/neck-escape-statistics]]); no wrong escapes.
Ordinary views are unaffected: home (0.015 % unresolved) and needle-minibrot-1e91 (0.007 %)
stop at once ("finished"), junction-1e100 has none, seahorse goes 0.35 → 0.10 % in +45 ms.
A view whose f64 orbit is cut (`−0.74999996120361811 + 0.00036918279150454967 i`, 1e−11/px,
cap 10 M): five orbit extensions to 3.97 M (escape), 98.7 → 1.2 % unresolved at 60 s.

## For almondbread

High default maxIter, derivative interior detection always available,
statistics-driven growth with hysteresis (never decrease on zoom-out), a
separate per-pixel step cap, and a statistics overlay like mathr's for
diagnosis. Related: [[techniques/frame-reuse-and-progressive]],
[[techniques/rebasing]].
