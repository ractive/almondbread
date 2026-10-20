---
title: "Guessing, border tracing and rectangle subdivision"
type: technique
status: draft
sources: ["[[sources/wikipedia-plotting-algorithms]]", "[[sources/kf-manual]]", "[[sources/xaos-readme]]", "[[sources/fractint-passes-help]]", "[[sources/muency-adjacency-optimization]]", "[[sources/xaos-algorithms]]"]
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
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Border tracing / edge checking).

**Rectangle checking (Mariani–Silver).** If every border pixel of a rectangle
has the same value, fill it; else subdivide and recurse, reusing computed
border pixels (e.g. 25×25 → 13×13 → 7×7 → 4×4). A halving variant with 1.4:1
boxes is faster. The corner-only Fractint method "causes damaged pictures more
often" and works only with ~6×6 boxes and no recursion. All variants are exact
only for regions of one discrete colour; smooth-coloured exteriors only benefit
if the inside uses a smooth method such as interior DE
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Rectangle checking).

**KF2+ "Use guessing".** Interpolates neighbouring pixel data when the iteration
count is the same. History: disabled in 2.12.4 for random speckles (possibly a
race), re-enabled behind a menu option in 2.12.7; a race with progressive
interlacing remains; guessing exterior gives poor quality; only interpolates
when both neighbours share glitch status. With OpenCL on GPU it gave about 1.5×
instead of 4× on interior-heavy views, and needs even image dimensions
(src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

**XaoS** is reported to use an adjacency optimisation that skips the interior of
large solid regions (research notes citing Mu-Ency's XaoS page, not saved)
(unverified). Its README only describes realtime zooming
(src: [XaoS README](../sources/xaos-readme.md)).

## Safety analysis (inference)

- With smooth colouring, exterior iteration counts rarely match exactly, so
  guessing mainly fires on interior pixels (maxIter or derivative-interior).
- Thin exterior filaments that cross a tile without touching its border are
  lost. Mitigations: small tiles (8×8, 16×16), test every border pixel, and a DE
  check: if any border pixel has `de_px` below the tile size, refine. By Koebe
  1/4 no set point lies closer than `DE/4`, so this bound is rigorous for
  exterior pixels ([Distance estimation](distance-estimation.md)).
- With derivative interior detection ([Interior detection](interior-detection.md))
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

## Measured (R3, CPU f64, 2026-10-14, M1 Pro)

From `prototypes/dense-sampling` ([Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md)): 800 × 500 crops,
default look, cost = iterations relative to exact 1 spp, error = OKLab ΔE × 100 against a
converged reference.

- **Interior-only Mariani–Silver** (32 px tiles, split to 4 px, border pixels computed, filled
  only when the whole border is interior): elephant 0.63 (8.9 % of pixels filled), minibrot-p4
  0.76 (37 %), seahorse 0.994; nothing at the dense, deep, filament, neck and root-third views.
  **Zero** filled pixels were exterior anywhere. The derivative test leaves parabolic regions
  (neck, root-third) unresolved rather than interior, so the fill never fires there.
- **Colour guessing in OKLab** (grid 8 → 1, guess a pixel from its cell corners or edge ends
  when they share a class and differ by ≤ τ): at the dense view τ 5 saves 4 %, τ 10 saves 27 %
  and changes colours (p99 vs exact 16.7); elsewhere τ 10 costs 0.24–0.61 with growing p99 and
  max errors (neck: max ΔE 83). Exterior guessing confirms the KF note "guessing exterior gives
  poor quality" (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Guessing failure modes in the literature: islands inside uniform borders
  (src: [Fractint / Xfractint help: drawing passes](../sources/fractint-passes-help.md)), sub-pixel cusps and thin channels
  (src: [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](../sources/muency-adjacency-optimization.md)), connections thinner than the sampling
  (src: [XaoS: Algorithms document](../sources/xaos-algorithms.md) §Calculating New Pixels).

## For almondbread

Interior-only tile fill shipped in M5 (below), gated to the views where it pays; never
exterior guessing. Local SA ([Local series approximation (tile and pixel seeds)](local-series-approximation.md)) cannot shorten
interior detection, so the two complement each other, but they do not combine well on one
level (the fill's border-first order breaks the seeds' admission coherence): a level uses one
or the other. Related: [Frame reuse and progressive refinement](frame-reuse-and-progressive.md).

## As built (M5, 2026-10-15): interior-only tile fill

Interior-only Mariani–Silver on the 16 × 16 admission tiles of a full level, one level of
tiles (no subdivision). Always auto (the switch `ALMONDBREAD_FILL` and `--fill` were removed 2026-10-17). Code: `tileTexel` /
`tileIndex` (fill order), `prepareChunk` (border-first gate), `iterate` (fill and the tile
stamps), scheduler `fill_wanted` / `decide_fill`.

```text
admission order of a filled level: all tile borders (60 texels each, ring order), then the
    14 x 14 insides of the tiles (rank order as before)
border pixel: good  = proven interior by the derivative test within fillProof (8192)
                      iterations (a function of its own orbit)
              spoils its tile (tileBad[tile] = epoch) when it finishes otherwise, or is still
              running at fillProof (it can only finish later)
prepareChunk: insides only once no border pixel is still below fillProof (per-chunk count),
              so a tile's decision never depends on chunk timing; the wait is <= fillProof
inside pixel of a whole tile with tileBad != epoch: written interior (.y = 1, DE 0) without
              iterating; otherwise iterated as before
```

- **Exact in every measured case**: the filled field equals the unfilled one bit for bit
  (elephant, home-100k, minibrot-p4; test `interior_fill_exact`). Only proven interior, never
  "unresolved": the derivative test leaves parabolic regions (neck, root-third, cusp)
  unresolved, so the fill never fires next to the narrow exterior channels where a tile border
  could step over a sub-pixel gap (the residual risk of any Mariani–Silver: a channel thinner
  than the border's sampling).
- **Flat interior colouring only**: period, angle, bubbles and distance need per-pixel cycle
  analysis (multiplier, interior distance); a fill would paint the tile without it, so the
  fill is off with those modes (decided, not computed).
- **Gate (auto)**: the border-first order costs chunk occupancy (the borders, 23 % of the
  texels, are admitted alone), the short wait, and the local-SA seeds' coherence. Measured
  (1600 × 1000, M1 Pro): elephant 32 → 23 ms (1.39×, 33 % fewer steps), cusp-1e5 1.03×
  (unseeded), home-100k and minibrot-p4 5–17 % slower (their interior proves in tens of
  iterations), cusp-1e5 seeded 16 % slower with no tile filled. So it fills where the last
  level had ≥ 15 % *proven* interior pixels (unresolved ones excluded: at sea-8007, 98 %
  unresolved, it filled nothing and cost 9 %) and ≥ 300 iterations per pixel, and only where no tile
  starts from local SA seeds (a fill candidate waits for its seed round before its first
  chunk). R3's CPU study found 0.76 at minibrot-p4 with 32 → 4 px subdivision; on the GPU the
  single-level fill loses there.

# Citations

- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [XaoS README](../sources/xaos-readme.md)
- [Fractint / Xfractint help: drawing passes](../sources/fractint-passes-help.md)
- [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](../sources/muency-adjacency-optimization.md)
- [XaoS: Algorithms document](../sources/xaos-algorithms.md)
