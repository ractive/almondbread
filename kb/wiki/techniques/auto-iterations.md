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
  stored as *unresolved* (iteration code −3). It is drawn as the interior colour
  lifted 3 % (linear) towards neutral grey: the same in every view, distinguishable
  from interior, and still reading as "inside" at a glance. Most such pixels are
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
41 536, 2 257 ms instead of two (10 384 + 41 536). `ALMONDBREAD_ITER_FLOOR=0` turns it off.

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
unresolved colour should differ from interior (open; proposal in the log, 2026-10-17).

## For almondbread

High default maxIter, derivative interior detection always available,
statistics-driven growth with hysteresis (never decrease on zoom-out), a
separate per-pixel step cap, and a statistics overlay like mathr's for
diagnosis. Related: [[techniques/frame-reuse-and-progressive]],
[[techniques/rebasing]].
