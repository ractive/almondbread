---
title: "Local series approximation (tile and pixel seeds)"
type: technique
status: reviewed
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
`w = dz_n/dz_1` is a function of `c` like `z`: the seed carries it and its derivative
`W = dw/dc` and a sample starts at `w + W δ` (R5; the R3 design restarted `w` at the branch,
which paints exterior interior at deep minibrot views, see Pitfalls):

```text
w_{n+1} = 2 z_n w_n          w_1 = 1   (skip the step from z_0 = 0)
W_{n+1} = 2 (A_n w_n + z_n W_n)   W_1 = 0
```

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

## GPU prototype (R5, measured 2026-10-14, M1 Pro, release)

Metal, float32 deltas, the production perturbation/rebasing loop (`iterate` with function
constant `kSeeded`), floatexp seeds in the deep kernel; offscreen renders only (`almondbread
render|verify --sa 1 [--sa-eps e] [--sa-top t] [--sa-leaf l]`, `ALMONDBREAD_SA=1`). Answer and
per-view numbers: [[questions/local-sa-float32]], [[synthesis/benchmarks]] R5.

```text
seedLevel (one dispatch per quadtree level, 32 -> 16 -> 8 -> 4 -> 2 texels; one thread per node)
    root: dz = 0, a = 0, b = 0, w = 1, W = 0, m = n = 0
    child: parent's state shifted by delta (texels): dz += a delta + b delta^2/2, a += b delta,
           w += W delta; re-test the rebase |Z_m + dz| < |dz|
    loop (n < limit):
        deep + BLA table: at aligned m take the longest block valid for the whole node
            (|dz| + |a| r (1+eps), |c| + texel r): dz' = A dz + B c, a' = A a + B texel,
            b' = A b, w' = A w, W' = A W  (exact under the block's affine map)
        propose one exact step (z = Z_m + dz): a', b' = 2 (a^2/texel + z b), dz', w', W'
        stop before it if the step escapes or |b' texel| r > 2 eps |a'|
        accept, rebase test; stop after it if |w| n^2 < 1e-3 (interior seed)
    write SeedNode (72 B: dz, a, b, w, W with exponents, m, n)
iterate (kSeeded): a fresh pixel reads its leaf node and starts from the shifted state;
    everything else (chunks, live list, BLA, extension lists) is unchanged.
```

Units: `a = dz/dc · texel` (the kernel's DE derivative), plain `b = d²z/dc² · texel` (so
`a²/texel` stays finite), deep `b = d²z/dc² · texel²` in floatexp.

What the GPU adds to the CPU picture:

- **Float32 is not the problem.** A child's offset `a δ` is relative to the same reference
  delta as the seed, with the same resolution a plain pixel's delta has; no collapsing
  neighbours, streaks or blocks in any crop.
- **Steps fall further than on the CPU at the 1× limit**: 8.7× at the dense view (R3, at 4×
  the limit: 5.2×), 12× at neck-2e5, 17× at sea-1e20 (after BLA).
- **GPU time falls less than steps**: dense 5.0×, deep-5e-14 4.6×, root-third 3.2×, filament
  1.9× (5.6× fewer steps). Three costs: the seed passes (1 600 root threads; long seed runs in
  unresolved views: neck-7e6 33 of 44 ms), reference reads that lose coherence (a SIMD group
  starts 8 leaves at unrelated `m`; Feigenbaum 0.75×, cusp 0.9× at 1.56× fewer steps), and
  per-pixel fixed costs when only ~100 iterations remain.
- **Tunables** (step counts at the dense view; times too noisy on the shared GPU to rank):
  leaf 2 / 4 / 8: 5.3e8 / 6.2e8 / 7.6e8 steps; root 16 / 32 / 64: 5.4e8 / 5.3e8 / 5.3e8;
  ε 3e−3 / 1e−2 / 3e−2: 7.7e8 / 5.3e8 / 3.6e8 (3e−2 not verified for quality).

## Pitfalls

- Flat tiles stop at the first fold; nest them.
- `ε = 1e−3` makes child seeds fail on their first step (no gain past the parent); 1e−2 works
  and its error is invisible in every measured view.
- **Do not restart the interior test's `w` at the branch** (the R3 design): the next passage
  near 0 multiplies the restarted `w` by `|2z|` (1e−300 next to a deep minibrot's nucleus)
  with nothing to balance it. Measured at needle-1e305: 96 % of exterior pixels painted
  interior. Carry `w` and `dw/dc` instead (Formulas). Copying the seed's `w` unchanged would
  be the opposite error near component boundaries.
- The seed's own `w` must start at 1 after the first step: the generic step from `z_0 = 0`
  multiplies it by 0, and every root seed then "proves" interior at n = 8.
- Branch decisions must not depend on chunk boundaries (as for BLA lookups in M4): the branch
  rule depends only on the seed's orbit, so it is chunk-independent if the seed state is
  stored, not recomputed.
- Deep views: seeds without BLA iterate in floatexp what the pixels' BLA would skip (needle
  1.08× fewer steps, sea-8007 0.34×); take BLA blocks in the seeds.
- Interior- and unresolved-heavy views gain nothing (1.03–1.6× fewer steps) and lose time to
  incoherent reference reads; switch SA off there (below).

## For almondbread: recommendation and M5 integration

**Go, with a gate.** Build it in M5 for full-resolution levels where the forecast says it
pays; leave it off for interior-heavy views. Expected: 3–5× less GPU time for the refined
frame in dense and filament views (dense-5e-14 78 → 16 ms), 4–8× at deep seahorse-type
views, neutral to −25 % where it does not pay (hence the gate).

Design (ours; from the prototype's measurements):

1. **Seed phase per level, on the GPU, chunked.** A level that is seeded starts with the
   quadtree passes (one dispatch per node level, root 32, leaf 2) in the same command buffers
   as its first chunks. Seeds need the watchdog treatment too: give each seed pass an
   iteration budget and resume, like `iterate`, because unresolved seeds run to the limit
   (Feigenbaum root seeds: ~200 000 iterations on 1 600 threads). Node buffer: 72 B × ~1/3
   node per pixel (2560 × 1640: ~100 MB at leaf 2; 25 MB at leaf 4), allocated once at the
   largest level size like the preview textures.
2. **Admission from leaves, in leaf-sorted order.** `prepareChunk` keeps tile admission; the
   iterate kernel's fresh path reads the leaf (as in the prototype). To win back reference
   coherence, admit pixels so a SIMD group shares a branch index: order the tile's 256 texels
   by leaf `m` (a per-tile sort in a small kernel after the seed phase, writing the tile's
   admission permutation), or use leaf 4 so 16 pixels share one start (costs 17 % more steps
   at the dense view). Measure both; the prototype admits in plain raster-in-tile order.
3. **Gate per view** from cheap data the scheduler already has: the previous level's (or
   preview's) iteration statistics. Seed when the mean escape iteration is high and the share
   of interior + unresolved texels is low (dense, filament, seahorse, neck-type views); skip
   for interior-heavy views (home-100k, elephant, minibrot-p4, Feigenbaum, cusp). A first cut:
   seed if interior + unresolved < 30 % and mean iterations > 300 (unverified thresholds;
   calibrate on the benchmark set). Also skip when the root seeds branch early (mean root
   n < 10 % of the limit): the leaf depth then gains little and the seed cost remains.
4. **Previews (gesture frames)**: the same scheme at the preview's texel size (jitter shifts
   all nodes alike: node centres include the level jitter). The dynamic-resolution model sees
   the cheaper texels and raises the ratio by itself. Seeds are per level and per jitter, so
   nothing is shared between gesture frames.
5. **Extension passes** (unresolved pixels at 4× / 16× the limit) start from the same leaves:
   seeds stop at the nominal limit, so an extension pixel restarts at the leaf's `n`, not at 1
   (the prototype already does this).
6. **AA samples**: pixel seeds as in R3 (radius 0.57 px, the pixel's own orbit as the seed);
   not prototyped on the GPU. The leaf state is a cheap start for them already: an AA sample
   can branch from its pixel's leaf node with its Halton offset.
7. **Deep kernel**: keep BLA blocks in the seeds; floatexp seed steps are ~5× a plain step,
   affordable only because seed steps are 2–20 % of the total.
8. **Shown frame = fresh render**: seeds are deterministic per level (no dependence on chunk
   boundaries), so the M4 rule holds; `regress` sessions and a `--sa 0|1` skip-off diff
   (like `--bla`) belong in the tests.
9. **Verification**: `verify` at all locations (class mismatch and GPU-int/CPU-ext no worse
   than plain; escape-n/median |Δμ| within the measured R5 margins), and a deep minibrot view
   with the interior test on (needle-1e305: the case that caught the `w` restart).

Complements [[techniques/bla]]: BLA skips the start of orbits at M3b depths, local SA the
coherent middle at any depth. Interior-heavy views need interior-only Mariani–Silver
([[techniques/guessing-and-subdivision]]) instead.

## As built (M5, 2026-10-15)

Code: `crates/almondbread-render/src/shaders/mandelbrot.metal` (`seedBegin`, `seedLevel`,
`seedGate`; `iterate` with function constant `kSeeded`, `prepareChunk`'s seed gate) and the
scheduler (`start_seeding`, `encode_seed_round`, `harvest_seed_round`, `advance_seeding`).
On by default: `ALMONDBREAD_SA=0|1|auto` (auto), CLI `--sa 0|1|auto` with the tunables
`--sa-leaf`, `--sa-top`, `--sa-eps`, `--sa-gate rel,abs`, `--sa-budget`. Numbers:
[[synthesis/benchmarks]] M5.

```text
seeding of a level (its own texel size and jitter):
    rounds until no node is unfinished, one command buffer each:
        seedBegin                                   (status := 0)
        seedLevel per quadtree level, root first    (budget steps per node; a node keeps its whole
                                                     loop state + stamp = epoch | state, so any cut
                                                     of the work into rounds gives the same nodes)
        seedGate                                    (per 16 x 16 tile: start from the seeds?)
    prepareChunk admits nothing while the last round left nodes unfinished
iterate (kSeeded): a fresh pixel of a seeded tile starts from its leaf, offset by its sample
    position (view rotation applied like the pixels'); every other pixel as before
```

- **Quadtree 32 → 4 texels, ε 1e−2.** Leaf 4, not R5's 2: ~15 % more steps at the dense view
  but faster on the GPU (4× fewer leaves; with Morton order inside the 16 × 16 admission tiles
  a SIMD group of 32 starts from exactly two leaves). Cusp-1e5: leaf 2 126 ms, leaf 4 102 ms;
  spiral 687 → 600 ms (measured 2026-10-15, M1 Pro).
- **Every level seeds**: refine previews and the full level at their own texel size and jitter;
  extension passes and AA samples branch from the full level's leaves (AA at their Halton
  offset; the offset stays inside the leaf's radius). Not built: R3's per-pixel seeds for AA.
- **Rounds within the frame budget.** A level's budget per node is a share of a chunk of GPU
  time divided by its "waves" (nodes / 16 384) and a learnt cost per budget step (up fast, down
  slowly; ~1.5 µs per dependent deep step on few threads); later rounds size by the nodes left,
  so a tail of a few slow nodes takes long passes. Chunks may be encoded right after the first
  round (they admit nothing until the seeds are final), so a view whose seeds finish in one
  round starts its pixels in the same refresh. The spiral at 1600 × 1000: 5 rounds, 42 ms.
- **Gesture frames and previews of refined views**: one bounded round (about a third of the frame budget for the whole chain
  of levels); a node out of budget *branches where it is* (`truncate`: an earlier branch is
  always valid, only less useful). Not deterministic in the budget, which gesture frames need
  not be. Where the bounded chain cannot reach the gate's bar (deep spiral: ~23 000 seed
  steps), no seeds at all.
- **Per-tile gate** (`seedGate`, auto): a tile starts from its seeds when its leaves' mean
  *path* (accepted seed steps from the root, i.e. what a pixel skips) is at least 1/6 of the
  last level's steps per texel (unseeded-equivalent: its pixel steps plus what its seeds
  skipped) and at least 8; without that estimate (a fresh renderer) when the leaves' mean
  branch iteration is 1/6 of the limit. Per tile, so a SIMD group never mixes. Where no tile is
  seeded the level switches to the plain pipeline (bit-identical fields, checked at elephant;
  the seeded variant costs ~4 % even unseeded). Iterations (not steps) as the measure fail in
  BLA views (sea-8007: leaves at n 20 845 of 40 000 but 26 seed steps, 2 % of a pixel's
  steps: seeded 16 % slower); a share of the limit fails where pixels escape early (dendrite:
  leaf n 793 of 20 000 saves 24 of 29 steps per pixel: 2.3× faster seeded).
- **Deep seeds in float32** down to texels of 2^−80: scaled units (`a = dz/dc · texel`,
  `b = d²z/dc² · texel`; `a²` never formed), max-norm tests (squared norms of 1e−19 deltas
  underflow), BLA blocks from the floatexp table converted per lookup; floatexp below 2^−80.
  The spiral's seeds: 150 → 40 ms (the root level's serial chain dominates).
- **Interior colourings** (period/angle/bubbles/distance need the orbit's closest approach):
  seeds track the smallest |z_n| and its n, pixels inherit it (z_0 = 0 excluded).
- **Node buffer** 88 B per node, grown to the largest level and kept (1600 × 1000: 133 k
  nodes, 11.7 MB; 2560 × 1640: 350 k, 31 MB), tile flags 4 B per tile.
- **Determinism**: a level's seeds depend on the view only (round cuts: bit-identical fields,
  plain, deep float32 and deep floatexp, test `local_sa_rounds_resumable`). The auto gate's bar
  depends on the last level's statistics, which can change which tiles are seeded between the
  shown frame and a fresh render; seeded and plain fields differ in class by 0.000 % at all
  18 locations, so the shown-frame check (class) holds.

### Pitfalls (M5)

- A seed's `b` must round-trip exactly between rounds: storing `b · texel` in floatexp and
  dividing on resume changed fields with the round budget (deep float32 seeds keep
  `d²z/dc² · texel` and the pixel start multiplies by the texel).
- The closest-approach tracking must skip `z_0 = 0`, or every inherited minimum is −∞ at n = 0
  and the interior analysis finds no period.
- Only the first round may be followed by chunks right away; letting a level's first chunk
  wait for the round harvest changed the chunk-size schedule (minibrot-p4 +20 %).
- A cheap seeded refined frame misleads the gesture frames' dynamic resolution (cost per
  texel): previews of the spiral ran 166 ms each. Gesture chunks now report their cost also
  when their job was superseded.
- Bounded gesture seeds are useless where the chain is long; skip them rather than pay ~3 ms
  per frame for leaves no tile uses.
