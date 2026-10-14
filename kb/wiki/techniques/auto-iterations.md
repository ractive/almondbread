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

## For almondbread

High default maxIter, derivative interior detection always available,
statistics-driven growth with hysteresis (never decrease on zoom-out), a
separate per-pixel step cap, and a statistics overlay like mathr's for
diagnosis. Related: [[techniques/frame-reuse-and-progressive]],
[[techniques/rebasing]].
