---
title: "Guessing, border tracing and rectangle subdivision"
type: technique
status: draft
sources: ["[[sources/wikipedia-plotting-algorithms]]", "[[sources/kf-manual]]", "[[sources/xaos-readme]]"]
tags: [guessing, mariani-silver, border-tracing, performance, gpu]
---

# Guessing, border tracing and rectangle subdivision

Image-level tricks that skip whole regions. They rely on the Mandelbrot set
being full (no holes): a closed curve of same-valued pixels encloses only that
value.

## Methods

**Border tracing.** Follow the boundary of a region of one iteration level and
fill its inside. Wikipedia reports a 93.16% reduction in iterations on a
1000-iteration example, growing with maxIter
(src: [[sources/wikipedia-plotting-algorithms]] §Border tracing / edge checking).

**Rectangle checking (Mariani–Silver).** If every border pixel of a rectangle
has the same value, fill it; else subdivide and recurse, reusing computed
border pixels (e.g. 25×25 → 13×13 → 7×7 → 4×4). A halving variant with 1.4:1
boxes is faster. The corner-only Fractint method "causes damaged pictures more
often" and works only with ~6×6 boxes and no recursion. All variants are exact
only for regions of one discrete colour; smooth-coloured exteriors only benefit
if the inside uses a smooth method such as interior DE
(src: [[sources/wikipedia-plotting-algorithms]] §Rectangle checking).

**KF2+ "Use guessing".** Interpolates neighbouring pixel data when the iteration
count is the same. History: disabled in 2.12.4 for random speckles (possibly a
race), re-enabled behind a menu option in 2.12.7; a race with progressive
interlacing remains; guessing exterior gives poor quality; only interpolates
when both neighbours share glitch status. With OpenCL on GPU it gave about 1.5×
instead of 4× on interior-heavy views, and needs even image dimensions
(src: [[sources/kf-manual]]).

**XaoS** is reported to use an adjacency optimisation that skips the interior of
large solid regions (research notes citing Mu-Ency's XaoS page, not saved)
(unverified). Its README only describes realtime zooming
(src: [[sources/xaos-readme]]).

## Safety analysis (inference)

- With smooth colouring, exterior iteration counts rarely match exactly, so
  guessing mainly fires on interior pixels (maxIter or derivative-interior).
- Thin exterior filaments that cross a tile without touching its border are
  lost. Mitigations: small tiles (8×8, 16×16), test every border pixel, and a DE
  check: if any border pixel has `de_px` below the tile size, refine. By Koebe
  1/4 no set point lies closer than `DE/4`, so this bound is rigorous for
  exterior pixels ([[techniques/distance-estimation]]).
- With derivative interior detection ([[techniques/interior-detection]])
  interior pixels already stop early, so the payoff shrinks. Measure first.

## GPU mapping (inference)

Recursion and per-thread divergence do not suit GPUs. A two-pass tile scheme:

```text
pass 1: compute all tile-border pixels (tiles 16x16) -> per-pixel (n, flags, de)
pass 2: per tile (one threadgroup): uniform = all border pixels interior
        if uniform: fill tile as interior
        else: append tile id to work list (atomic counter)
pass 3: dispatchThreadgroups(indirectBuffer:) over the work list,
        compute remaining pixels
```

The KF OpenCL result (1.5×) suggests the gain is capped by divergence and the
extra passes. No GPU benchmark for perturbation deep zoom was found.

## For almondbread

Not in v1. If profiling shows interior-heavy frames still dominate after
derivative interior detection, add tile guessing restricted to tiles whose
entire border is interior (never exterior guessing), via indirect dispatch.
Related: [[techniques/frame-reuse-and-progressive]].
