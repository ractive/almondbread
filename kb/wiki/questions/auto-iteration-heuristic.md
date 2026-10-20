---
title: "What auto-maxIter rule should almondbread use?"
type: question
status: open
sources: ["[[sources/kf-manual]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/wikibooks-fraktaler-3]]", "[[sources/fractalforums-interior-detection]]"]
tags: [max-iterations, heuristic, interior-detection]
---

# What auto-maxIter rule should almondbread use?

## Question

How should the maximum iteration count adapt to the view so that zooms never show
false black regions yet do not waste time?

## Why it matters

Too low a cap shows "blobby false interior"; too high costs time and reference
length. See [Automatic maximum-iteration control](../techniques/auto-iterations.md).

## What sources say

- KF2+ "auto iterations" is on by default but undocumented; it analyses rendered
  images (it does not work in "Save KFR" mode) and drops maxIter too far on
  zoom-out sequences, producing black regions (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- With interior detection a very large N is cheap; Chéritat suggests avoiding only
  absurd values like 1e10 (src: [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md)).
- Fraktaler 3 has separate caps for total iterations (262144 in an example), reference
  iterations and perturbation steps (4096) (src: [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)).
- mathr's renderer statistics show limits that are too small give "blobby" false
  interior (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)).

## Candidate rule (inference, untested)

After each frame: p999 = 99.9th percentile of escaped n; unresolved = fraction of
pixels that hit maxIter without escaping and without passing the interior test.
If unresolved > 0.1% and p999 > 0.5 maxIter, double maxIter (or set 4 p999).
Never lower during a zoom-out without hysteresis. Cap at what the reference
supports (with rebasing, the reference can be shorter than maxIter).

## How to resolve

Run zoom-in and zoom-out sequences on the benchmark locations with the rule, count
frames with unresolved pixels and compare time against a fixed high cap with
derivative interior detection ([Interior detection](../techniques/interior-detection.md)).

# Citations

- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](../sources/cheritat-mandelbrot-rendering.md)
- [Wikibooks: Fractals/fraktaler-3](../sources/wikibooks-fraktaler-3.md)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
