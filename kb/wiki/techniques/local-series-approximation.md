---
title: "Local series approximation (tile and pixel seeds)"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/mathr-deep-zoom-theory-again]]"]
tags: [series-approximation, iteration-skipping, anti-aliasing, dense-regions, measured]
---

# Local series approximation (tile and pixel seeds)

Neighbouring samples share most of their orbit. Expand `z_n(c)` to second order around a
*local* seed point `c₀` (a tile centre, or a pixel centre for its anti-aliasing samples)
instead of around the view's reference, iterate only the seed while the expansion holds,
then start every sample of the region from the expansion. This is classic series
approximation ([[techniques/series-approximation]]) with two changes: the expansion point is
local, so it stays valid much longer where the global one fails, and regions are nested
(quadtree), so a fold that stops a big region early only costs that region's children a
restart, not every pixel. Our design (R3, 2026-10-14); the recurrences are the standard SA
ones (src: [[sources/wikipedia-plotting-algorithms]] §Perturbation theory and series approximation).

## Why it works in dense regions where BLA does not

[[techniques/bla]] (M4) linearises each step around the *reference* orbit: valid while the
pixel's delta `|dz| ≪ |Z|`. At the dense benchmark view deltas restart at `≥ 1e−5` after every
rebase, so linear runs are 2–8 steps. But the pixels of a small region stay close to *each
other*: the image of one pixel's footprint, `|dz/dc| · pixel`, stays below 1e−3 (z-plane
units) for about 97 % of the orbit and expands only in the last ~3 % before escape (measured
2026-10-14, CPU f64, 4e5 pixels at dense-5e-14; [[synthesis/dense-region-sampling]]). So
`c ↦ z_n(c)` is nearly linear across a pixel, and across a small tile, for most of the orbit,
even though the step map is not linear around the reference.

## Formulas

Seed recurrences (derivatives with respect to `c` at the seed):

```text
A_{n+1} = 2 z_n A_n + 1            A_1 = 1      (dz/dc, the DE derivative)
B_{n+1} = 2 (A_n² + z_n B_n)       B_1 = 0      (d²z/dc²)
z_{n+1} = z_n² + c₀
```

Validity over a region of radius `r` (complex units): the quadratic term is at most `ε` of the
linear one,

```text
|B_n| r / (2 |A_n|) <= ε          (ε = 1e-2 measured best; 1e-3 branches earlier)
```

Branch at the last iteration `m` where this holds. A sample at `c = c₀ + δ` starts at

```text
z_m ≈ Z_m + A_m δ + B_m δ²/2,   dz/dc ≈ A_m + B_m δ,   n = m
```

and iterates exactly from there (perturbation in the app). The interior test's derivative
`w = dz_n/dz_m` restarts at the branch with the absolute `n` in its `n²` guard; the seed's own
`w` does not carry over to other `c`.

Quadtree: a `T × T` tile seeds from `n = 1`; each child of a node re-seeds from its parent's
branch state with half the radius (`z + Aδ + Bδ²/2`, `A + Bδ`, `B`) and iterates on in seed
mode until its own branch; 2 × 2 nodes hand their state to their pixels. A pixel that will
get AA samples seeds once more with radius 0.4·√2 px; its 3 × 3 samples start there.

```text
seed(node, parent_state):
    s = parent_state shifted by δ = node.centre - parent.centre
    while |B| r / (2|A|) <= ε and not escaped: step z, A, B          # seed mode
    if node is a pixel: run the pixel (and its AA samples) from s     # exact mode
    else: for child in 4 children: seed(child, s)
```

## Error

The dropped cubic term at the branch is about `ε²` of the linear term, i.e. a sample starts
where an exact orbit from a point about `ε² |δ|` away would be; later iterations scale both
alike. In chaotic regions individual escape counts change anyway: a rounding-level change of
`c` (branching at iteration 28 with ε = 1e−9) already changes 36–48 % of escape counts in f64
at the dense view, and float32 perturbation differs from the f64 oracle on 30 %
([[synthesis/benchmarks]] M4). What matters is that branched samples are statistically the
same draw: measured, every error statistic against an independent 16-sample reference equals
the exact render's to two decimals (dense view: mean ΔE 5.41 at 1 spp, 1.96 with 3 × 3 AA,
both ways), and the images look the same at 200 % (crops in the study output).

## Measured (CPU f64, 2026-10-14, M1 Pro; full table in [[synthesis/dense-region-sampling]])

Dense view (−1.40803213403045 + 0.13680236432900i, 4.6e−14/px, 800 × 500 of the 1600-wide
framing, limit 4 × 5791):

| variant | iterations / exact 1 spp | error vs reference |
|---|---:|---|
| exact, 1 spp | 1.000 | ΔE mean 5.41, p99 18.1 |
| flat tiles 16 × 16, ε 1e−2 | 0.279 | same |
| quadtree from 32, ε 1e−2 | 0.194 | same |
| quadtree from 32, ε 1e−3 | 0.283 | same |
| exact FL 3 × 3 AA (settled image) | 8.936 | ΔE mean 1.96, p99 7.1 |
| AA samples off their pixel's orbit only | 2.050 | same |
| quadtree 32 + AA off pixel seeds | 1.244 | same |

Seed-mode iterations are 5 % of the total. The minibrot-rich view 100× shallower showed why
flat tiles are not enough: every tile size from 4 to 32 branched at ~10 % of the orbit
(a fold: the orbit passes close to 0 inside the tile), saving 8 %; the quadtree recovered
2× there.

## Costs on a GPU (inference)

- A seed step adds `B` (one complex multiply-add plus `A²`) to the DE step the kernel already
  does; seed-mode iterations are ~5 % of the work, so the extra ALU is ~2–3 % overall.
- A branch costs one state copy per sample (`dz, A, B, m, n`: 32 B per seed).
- Samples of a region start together at the same `m`: better SIMD coherence than raster
  admission (reference reads at the same index).
- float32: the per-pixel offset `A_m δ` must stay above float32 rounding of `dz`; it is the
  same condition under which neighbouring pixels are distinguishable at all in the plain
  kernel, and `+dc` is added again every step. To verify in the GPU prototype
  ([[questions/local-sa-float32]]).

## Pitfalls

- Flat tiles stop at the first fold; nest them.
- `ε = 1e−3` makes child seeds fail on their first step (no gain past the parent); 1e−2 works
  and its error is invisible in every measured view.
- Do not copy the seed's interior-test derivative to other samples: an exterior pixel would
  inherit a contracting `w` and be painted interior.
- Branch decisions must not depend on chunk boundaries (as for BLA lookups in M4): the branch
  rule depends only on the seed's orbit, so it is chunk-independent if the seed state is
  stored, not recomputed.

## For almondbread

Recommended for the M5 (refinement/AA) milestone, see [[synthesis/dense-region-sampling]]:
quadtree seeds for the refined frame and the gesture previews, pixel seeds for the
progressive AA samples. Complements [[techniques/bla]]: BLA skips the start of orbits at
M3b depths, local SA the coherent middle at any depth.
