---
title: "Places: a catalogue of famous views to visit"
type: synthesis
date: 2026-10-14
sources: ["sources/beyer-mandelbrot-zoom-sequence", "sources/muency-named-places", "sources/wikipedia-misiurewicz-point", "sources/robotmoon-mandelbrot-set"]
tags: [places, locations, misiurewicz, product, decision]
---

# Places: a catalogue of famous views to visit

**What.** A curated list of well-known, beautiful views of the Mandelbrot set
("places") that the app shows as a browsable list with thumbnails and can jump or fly to,
plus a random pick. Asked for by the user ("a list of well known coordinates with
landscapes that you can visit"); curated destinations and autopilot dives are common in
other explorers.

**Where.** Data and API in `crates/almondbread-core/src/places.rs` (+ `places_data.rs`),
UniFFI in `crates/almondbread-ffi` (`places()`, `place(id)`, `random_place(seed,
category)`, `place_scale(id, w, h)`, `Explorer::go_to_place(id)`), thumbnails in
`Sources/AlmondbreadKit/Resources/Places/` (`PlaceThumbnails.url(id:retina:)`), CLI
`almondbread places`.

## Conventions

- **Centre** as decimal strings of any length (the deepest carry 3,050 digits).
- **Span**: complex extent across the view's **shorter** side, so a place frames the
  same in portrait (iPhone) and landscape windows. Sources give other extents: Beyer's
  Commons files the horizontal diameter of 4:3 images (span = 0.75 × it), Mu-Ency
  `@ r` the half-height (span = 2r), Robot Moon the vertical span (= span).
- **Look**: one of the existing looks (`palette::Look`), chosen per place by eye.
- **Iterations**: automatic for every place so far (`iterations: None`); the field exists
  for views that need a fixed limit. Interior detection stays on (no place needed it off).
- **Source**: every place cites where its coordinate comes from. `generated: true`
  marks centres we computed (Newton nuclei, Misiurewicz points) or framings we chose
  near a cited point. Descriptions are our own words.
- **Categories** (menu sections): landmark, valley, spiral, dendrite, minibrot,
  julia-island, deep (1e-30 and below), endless-dive (centred exactly on a Misiurewicz point). Within a category, shallow to deep.

## Generated coordinates

- Misiurewicz points were refined from Wikipedia's 8–15-digit approximations with a new
  Newton solver, `almondbread_math::find_misiurewicz` (`locate --center re,im
  --misiurewicz k,p --bits b`): Newton on `g = (z_{k+p} − z_k) / ∏_{i<k}(z_{i+p} − z_i)`,
  which divides out the lower-preperiod roots, with the derivative carried alongside.
  Wikipedia's seahorse spiral "M(23,2)" is M(24,2) with z_0 = 0 (orbit check; see
  [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md)); M(4,1), M(3,1) converge in 4–8 steps up to
  3,500 bits.
- Minibrots: `almondbread locate` (ball period + Newton nucleus) as for the deep benchmark
  set ([almondbread benchmarks and verification record](benchmarks.md) M3b). The starburst at 1e-112 is the period-477 nucleus
  5 radii from M(4,1) found with `--radius 1e-60`. Robot Moon's "pine compass" names a
  period-70 nucleus near -1.4303584327 but frames it at 1e-34 off-centre without giving
  the centre; `locate --radius 1e-10` finds the nucleus -1.430358432725985534067 (size
  6e-16), framed by us at 1.5e-12 (`pine-compass`) and 2e-14 (`crystal-compass`). Searching for a minibrot right at the
  M(24,2) spiral centre failed: Newton did not converge for periods 1,552–2,764 started at
  the Misiurewicz point itself.

## Checks (2026-10-14, M1 Pro, release)

- Every place renders headless at 320×200 and 640×400 (`--ss 3`) with its look; each
  thumbnail was looked at, and the command fails if one is flat (luma range < 40).
  Rejected or reframed while looking: a duplicate cusp view (same as elephant valley),
  the antenna tip at -2 (an empty grey line), the 1/3-limb tip M(3,1) (mostly empty),
  c = i framed exactly (moved the centre to -0.02 + 0.98i so the twigs fill the view),
  a deep 1/3-limb tip at 1e-1000 (wide grey bands), Beyer's images 11 and 14 at their own
  width (grey moiré at thumbnail size: zoomed 3× and 5×), two medallions zoomed 2×.
- Deep places through `verify --place id` against the CPU oracle at 160×100: class
  mismatch 0 % for all 15 places beyond f64 (1e-14 to 1e-1000); escape-n differs 0.08–18 %
  in the dense spiral and mandala views and 39 % at the seahorse minibrot, the known
  float32-vs-f64 chaos of dense regions ([almondbread benchmarks and verification record](benchmarks.md) M3b: sea-8007 23 % at
  400 wide), and below 0.2 % at the needle minibrots.
- Thumbnails: HEIC through `sips` (quality 70), 2 × 69 files, 4.6 MB; JPEG at the same
  size was 2× larger.

## Below 1e1000: four places to the depth limit (2026-10-16)

Four deep places beyond the two at 1e1000: `pinwheel-1e1500`, `star-1e2000`,
`swirl-1e3000` and `rock-bottom`. The depth limit is `MIN_SCALE_LOG2 = -10000`
(`viewport.rs`): 2^-10000 ≈ 5.0e-3011 per view point. A place's span is across the shorter
side, so in a 1000-point window the deepest reachable span is 5e-3008; Rock Bottom's span
1e-3007 (zoom 1e3007.5) is 2^-9999 per point there, one doubling above the limit, and
`go_to_place` clamps to the limit in windows over 2000 points. A new test keeps every place
reachable at 1000 points (it replaced a span floor of 1e-1100).

**Why not deeper Misiurewicz points.** Zooming further into M(24,2) or M(4,1) only repeats
`spiral-1e1000` and `junction-1e300`: a Misiurewicz point is asymptotically self-similar. The
new places are embedded Julia sets instead, at a set fraction of the log-depth between a
Misiurewicz point and a minibrot next to it. `locate --center M --radius r` (ball period +
Newton) finds a minibrot about r away of size about r² (1e-1000 → 1e-1998 to 1e-2000 at
c = i, M(4,1) and M(5,1)). Centred on that nucleus, the view at depth d shows the junction
of M doubled once per halving of the remaining log-distance: at the geometric mean of r and
the minibrot size a k-armed point shows 4k arms (c = i, a tip: 4; M(4,1): 12; M(5,1): 16),
at 3/4 of the way 8k, near the minibrot 2^n k arms and then the rings of
`starburst-1e112` and the needles (measured by eye, 2026-10-16). The point M decides the
symmetry, the fraction the arm count, so four views from three points at four fractions
look different (the community's version of this, Julia morphing and shape stacking, is in
[Finding good places at extreme depth (1e100 to 1e100000+)](finding-deep-places.md)):

| id | depth | from | minibrot (period, size) | view | look |
| --- | --- | --- | --- | --- | --- |
| `pinwheel-1e1500` | 1500.5 | c = i, `--radius 1e-1000` | 2659, 1e-2000 | 1/2: four curved arms | moss |
| `star-1e2000` | 2000.5 | M(5,1), `--radius 1e-1333` | 16857, 8e-2665 | 1/2: 16 straight spokes with four-armed crosses | ember |
| `swirl-1e3000` | 3000.5 | c = i, `--radius 1e-1714` | 4556, 1e-3428 | 3/4: eight curling arms | lagoon |
| `rock-bottom` | 3007.5 | M(4,1), `--radius 1e-2103` | 17046, 3e-4205 | 0.43: six-armed snowflake | classic |

M(5,1) = 0.36636298342276434 + 0.59153377326144523i is a four-way junction: Newton
(`locate --misiurewicz 5,1 --bits 11000`) from 0.29 + 0.58i, a guess; we did not check
whether it is the 1/4 limb's principal branch point. c = i, M(4,1) and M(5,1) were taken to
11,000 bits (3,300 digits) first; the centres are the nuclei cut to 40 digits below the span
(1,540 to 3,047 digits after the point).

**Rejected while looking** (640×400 renders, `build/deep/` in the worktree): the minibrots
themselves at 1e2000 (c = i, M(4,1)) and 1e3007 (c = i, period 3998): ring moiré like the
needle minibrots, and with the looks' calm dense colouring a flat disc around a small black
minibrot (mostly flat colour); M(4,1) at 3/4 to 7/8 of the way (dense starburst rings, a copy
of `starburst-1e112`); M(5,1) at 3/4 (the same); the 16-arm c = i swirl at 7/8 (too close to
the 16 spokes of `star-1e2000`); the 12-arm M(4,1) star (same genre as the 16-spoke star); an
`ice` snowflake at the bottom (a copy of the `snowflake` place's look). No minibrot was found
next to M(24,2): `locate` at radii 1e-30 to 1e-1001 picks periods 1,552 to 60,410 and Newton
gives up within 0.35 s every time (it leaves the 16-radius disc or stops converging; not
traced), also from starts 4 and 22 radii off the spiral centre; and a
second-order search on a branch point of the pinwheel at 1e-1503 (period 5,330) did not
converge either.

**Cost** (measured 2026-10-16, M1 Pro 10 cores, release, machine shared with other agents,
load average 40-95; the jumps repeated at load 5 came out within 10 %): reference as the app's worker computes it (`ALMONDBREAD_TRACE=1`: ball
period over the view, Newton, the nucleus orbit), `verify --history jump --size 1600x1000`
(a "Go to" with no history through the real per-refresh scheduler) and `verify --place` at
160×100 against the CPU oracle.

| id | ref bits | ref (period = points − 1) | ref time | first frame | resolved | class mismatch | escape-n differs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `pinwheel-1e1500` | 5120 | 2660 | 49 ms | 85 ms | 218 ms | 0 % | 0.03 % |
| `star-1e2000` | 6784 | 16858 | 400 ms | 460 ms | 593 ms | 0 % | 15 % |
| `swirl-1e3000` | 10112 | 4557 | 240 ms | 294 ms | 426 ms | 0 % | 0.04 % |
| `rock-bottom` | 10176 | 17047 | 939 ms | 1010 ms | 1143 ms | 0 % | 6.5 % |

For comparison `spiral-1e1000` needs a 62,957-point centre orbit at 3456 bits (333 ms). The
time is the reference: GPU iteration is 5-17 ms per 1600×1000 frame at every one of them
(BLA/LA over the nucleus), and `almondbread render` takes 0.23-1.12 s wall in all. The escape-n
differences at the star and Rock Bottom are float32 chaos, not error: `precision --place`
puts the GPU at a grid shift of ≤ 0.1 px (a 0.1 px shift of the oracle alone differs 22 % and
9 %). Finding the nuclei with `locate` took 0.5-12 s (Rock Bottom: 12.2 s at 14,080 bits);
that is catalogue work, not app work.

Thumbnails: `places --render-thumbs Sources/AlmondbreadKit/Resources/Places --inside bubbles
--extend 16 --only <ids>` (oversample 3, blur 0.5 by default), 1.0-2.6 s each, 2 × 4 HEIC
files, 192 KB; all four looked at, none flat.

## Needle minibrot at 1e3000: the needle of the needle (2026-10-16)

**Not in the catalogue** (status 2026-10-19). The place entry `needle-minibrot-1e3000` (its
3000-digit centre), its two thumbnails and `verify --history --no-verify 1` stayed on branch
`worktree-agent-a0bf2467567503583` (commit `d82efcf`), never merged: ~90 s to a settled frame is
far over the 10 s budget. `verify --history --timeout s` reached main separately. What follows
is the research as written on that branch; "the place" means that unmerged entry.

The user asked whether zooming further into `needle-minibrot-1e1000` finds the same minibrot
again near 1e3000. It does: `needle-minibrot-1e3000`, period 1,385,274 = 834 × 1661, size
1.7e-3000, at -2 + 1.0e-500 − 6.2e-1002 (on the real axis).

- **The copy.** The period-834 nucleus c₀ is 1.015e-500 from -2, size s = +3.09e-1002 (from
  `1/(β λ²)` with `λ = Π 2 z_i`, computed in Python `decimal`). In the copy's own
  coordinates `c = c₀ + s·u`; its antenna tip is at u ≈ -2.
- **The tip is M(835, 834), not M(1668, 834).** In the copy the critical orbit goes 0 → -β → β
  (local), but `f` is even, so `f(-β) = f(β)`: one step after the local -β, at iterate p + 1,
  the orbit is already on the β-cycle. Newton with preperiod 2p did not converge (35 s);
  `locate --misiurewicz 835,834 --bits 11200` from c₀ − 2s converged in 11 steps (1.1 s). On the
  period-3 airplane the same rule gives M(4,3) = -1.79033 from its estimate (Newton only, not
  rendered). The tip sits at u = -1.99999995: the
  copy is distorted by 5e-8.
- **The minibrot.** `locate --center tip + 1e-999·s --radius 1.5e-2001 --max-period 4000000`
  (centre computed in Python, so `--toward` was not needed): ball period 1,385,274, Newton 8
  steps, 206 s at 10,112 bits, local distance u + 2 = 1.41e-999. Needle minibrots near -2 at
  distance d have period ≈ log₄(1/d) + c; locally 1e-999 gives 1661, times 834.
- **It looks the same, but the rings are gone.** Rendered with the same span relative to
  its size as the 1e1000 view (2.1e-2999; `build/deep/keep/needle3000-640.png` next to
  `needle1000-640.png` in the worktree), the minibrot, its bulbs and satellites are the same
  picture at the same size. The ring field around it is not: dwell there is ~1661× larger and
  changes ~1661× faster per pixel, so the bands are far below a pixel and average to one flat
  colour (also with `--log 1`). The thumbnail at that framing failed the flat check (luma
  28..30), so the place is framed 2.1× tighter (span 1e-2999): the minibrot fills about a
  quarter of the view; the rest is flat.
- **Iterations.** The automatic limit stops at the 200,000 cap, 1/7 of one period: everything
  is unescaped. 20 M (14 periods) shows a black blob, 60 M a black minibrot silhouette, 200 M
  (144 periods; the 1e1000 view has 240) the coloured interior. The branch's entry was the first with
  `iterations: Some(200_000_000)`: Go and Fly switch to manual iterations and raise the cap
  (the cap picker in the control center has no 200 M entry).

**Cost** (measured 2026-10-16, M1 Pro, release, load 5-50):

| step | time |
| --- | --- |
| reference (ball period over the view, Newton, the 1,385,275-point nucleus orbit at 10,112 bits) | 67-78 s |
| GPU, 640×400 at 2.1e-2999 / 1e-2999 | 10.7 s / 29 s (4-5e12 iterations, LA 416× fewer steps) |
| `verify --history jump`, 1600×1000, span 1e-2999 | first frame 70 s, resolved 90 s |
| `verify --history jump`, 1600×1000, span 2.1e-2999 | first frame 82 s, resolved 97 s (load 53) |
| thumbnails (oversample 3 × ss 3, two sizes) | 18 min at 2.1e-2999 (rejected, flat); at 1e-2999 the 1× took ~8 min, the @2x (5760×3600 samples) hit the GPU watchdog (`ImpactingInteractivity`) repeatedly and was killed after 26 min |
| @2x thumbnail instead: `render --size 1280x800`, no supersampling, `sips` to 640×400, HEIC 70 | 161 s |

This is ~100× the other deep places and far over the 10 s budget; the branch put it in the
catalogue because the user asked for it, with a description saying it is slow. The time is the period:
any minibrot at 1e-3000 inside the 834 copy has period ~834 × 1661. A first frame would need
the reference worker to deliver something before Newton finishes (e.g. a centre-orbit prefix
while Newton runs), not built.

**Checks.** `verify --place` at the place's 200 M did not finish: the CPU oracle computes its
reference to the limit on one thread (killed after 32 min). At 80×50 and `--iterations 20000000`
(GPU and oracle alike): class mismatch 0 %, escape-n differs 98 % with median |Δμ| 157;
`precision` with the same settings puts that between a 0.001 px grid shift of the oracle
(97.7 %, median 48) and a 0.01 px one (median 470): dense-band chaos, not error. CLI additions
for this: `verify --history ... --no-verify 1` (timings only; branch only) and `--timeout s`
(default 30 s; on main).

## Endless dives (2026-10-17)

How such points are found: [Finding good places at extreme depth (1e100 to 1e100000+)](finding-deep-places.md); the autopilot and journey that fly into them: [Controls: keyboard, pointer and touch](../decisions/controls.md).

A category (`endless-dive`, "Endless dives") of places centred **exactly** on a Misiurewicz
point, so a straight zoom with no steering keeps the boundary in the middle of the view down
to the depth limit (2^-10000 per point) and never ends in a featureless area. The deep places
above use embedded Julia sets because a Misiurewicz point "only repeats itself"; these four
make the repetition the point.

**Self-similarity.** At a Misiurewicz point c₀ of preperiod k and period p the critical
orbit lands on a repelling p-cycle with multiplier `λ = ∏_{i=k}^{k+p-1} 2 z_i`. Near c₀ the
set is asymptotically self-similar: the view at magnification m and at m·|λ| agree, the
second turned by arg λ (Tan Lei, "Similarity between the Mandelbrot set and Julia sets",
1990; the parameter-space similarity is J_{c₀} near z_k scaled by dz_k/dc) (unverified
against the paper here; the factors below are computed). Above about 1/|dz_k/dc| the view
still shows the preperiodic part (other structure); below it, only the repetition.

| id | point | digits | λ | \|λ\| | arg λ | repeats |
| --- | --- | --- | --- | --- | --- | --- |
| `endless-forking` | c = i, M(2,2) | exact | 4 + 4i | 4√2 = 5.657 | 45° | full turn every ×2^20 ≈ 1e6 |
| `endless-spiral` | M(24,2) | 3,307 | 1.0428 − 0.2770i | 1.0789 | −14.87° | full turn every ×6.3; 30.3 repeats (1.25 turns) per ×10 |
| `endless-junction` | M(4,1) | 3,307 | −0.6552 + 1.1555i | 1.3283 | 119.55° | ×2.34 for three repeats (358.7°) |
| `endless-wander` | M(1001,3) | 3,310 | 1.146 + 4.461i | 4.605 | 75.59° | changes down to ~1e-295, then repeats |

λ from 3,400-digit Python `decimal` orbits of the stored centres; the M(24,2) and M(4,1)
residuals |z_{k+p} − z_k| are 1e-3304 and 1e-3305. Extending the two centres to 11,000 bits
(`locate --misiurewicz 24,2|4,1 --bits 11000`, 9 Newton steps, 0.05 s and 0.01 s) agreed with
all stored digits (1,049 and 327; the old M(24,2) re differed in its last, rounded digit), so
`two-arm-spiral`, `spiral-1e1000`, `three-way-junction`, `junction-*` and the rest now share
3,307-digit centres. The older `dendrite-i` stays framed at −0.02 + 0.98i.

**Never-repeating point: `locate --endless-seed n`.** The external angle is `--preperiod`
(default 1,000) pseudo-random bits from the seed (SplitMix64), then a `--period` (default 3)
bit tail; the last preperiodic bit differs from the last tail bit so the preperiod is exact.
The ray is traced inward (`almondbread_math::rays::trace_ray_inward`, Heiland-Allen's method
as in mathr's `m_d_exray_in`: per angle doubling n, 4 points at radii R^(2^(-j/4)), R = 65536,
each solved by Newton on `z_n(c) = r e^{2πi 2^{n-1}θ}` from the previous point, precision
2n + 128 bits) for preperiod + 4 periods doublings, then `find_misiurewicz(k = preperiod + 1,
p)` refines at 11,000 bits. The landing point of a ray with preperiod k′ is a Misiurewicz
point of preperiod k′ + 1 with z₀ = 0 (1/6 = .0(01) → c = i = M(2,2), a unit test).

Seed 2026, preperiod 1,000, period 3, tail (110) (measured 2026-10-17, M1 Pro, release, shared
machine): ray 39.6 s (1,012 doublings; 1.2 s to 250, 6.0 s to 500, 16.8 s to 750: cost grows
as n³, n iterations per solve at 2n bits), Newton 2.6 s (12 steps), 42 s in all. Checks:
|z_1004 − z_1001| = 1e-3019 (the 1e295 derivative eats ~295 of the 3,310 digits),
|z_1003 − z_1000| = 10^0.4 (preperiod exact), ray end to point 1e-296. The scenery changes
down to about 1/|dz_k/dc| = 1e-295 (≈ 0.295 decades per angle bit, close to log10 2: the
preperiodic part lasts about preperiod × log10 2 decades), then repeats ×4.605 at 75.6° to the
depth limit. Reaching the depth limit with change all the way would take a 10,000-bit
preperiod: by the n³ scaling ~1,000 × 40 s ≈ 11 h, not done. Seed 40 bits: 0.04 s, changes to
1e-11.

**Checks.** Contact sheet (start framing, 1e10, 1e100, 1e250, 1e1000, 1e3000; 320×200 renders):
the boundary stays at the centre at every depth for all four; the seahorse spiral and the
junction look the same at every depth (turned), c = i and the wander point are tips (one ray),
so the twigs fill one side. `endless-wander` at 1e10, 1e100, 1e200, 1e290, 1e300, 1e400 with
a crosshair: the centre is a twig tip each time and the shapes change until ~1e300. `verify
--place` at 160×100 at the framing, 1e1000 and 1e3000: class mismatch 0 % everywhere; escape-n
differs ≤ 0.013 % (c = i), ≤ 5.4 % (junction), ≤ 0.7 % (wander), and 17 % / 28 % for the
spiral at 1e1000 / 1e3000 (dense spiral float32 chaos, as `spiral-1e1000` above). Render time
0.05–6 s per 320×200 frame (6 s: spiral at 1e3000). Thumbnails looked at, none flat.

## Using a place

```text
almondbread places                          # list: id, category, depth, look
almondbread render out.png --place spiral-1e1000 --size 1600x1000
almondbread places --render-thumbs Sources/AlmondbreadKit/Resources/Places --inside bubbles --extend 16 [--only a,b] [--oversample 3 --blur 0.5]
almondbread places --format landscape       # landscape-rt start commands (f64 places)
```

Thumbnails (re-rendered 2026-10-15 after the merge into the control-center branch) use the
calm dense regions and the *bubbles* interior colouring with extension passes
([Colouring methods](../techniques/coloring-methods.md) "Interior colouring as built"): the flat black interiors
hid most landmarks, and the dense bands of deep minibrots were grey. The three needle
minibrots (1e91, 1e302, 1e1000) showed moiré: their radial spokes and ring bands come
within a pixel or two of the thumbnail grid, and the calm dense colouring does not touch
them (it keys on the radial band density, the spokes are angular). More samples alone did
not fix it: 9 → 81 samples per pixel (3× oversampling with a box filter) still left beats
near the Nyquist rate, because a box filter passes them. Since 2026-10-15 `--render-thumbs`
renders 3× larger (`--oversample 3`, times the `--ss 3` grid) and resamples with a Gaussian
of σ 0.5 output pixels in linear light (`--blur 0.5`; response 0.29 at Nyquist instead of
the box's 0.64); the needle thumbnails were re-rendered with it and the beats are gone
(before/after looked at, `build/thumbs/`; the remaining fine rings are real bands). The
other 66 thumbnails were not re-rendered (no visible aliasing). In the app,
Go and Fly to a place switch a flat interior to bubbles, so the view matches its thumbnail;
the control center's Places section shows them as a grid by category with Go, Fly
(`Explorer.flyToPlace`) and Random.

Swift: `places()` for the list (sections by `category`), `PlaceThumbnails.url(id:)`
for the image, `explorer.goToPlace(id:)` to jump (centre, span fitted to the view's
shorter side, look, automatic iterations), or `placeScale(id:width:height:)` with
`setViewportText` for a fly-to target. `randomPlace(seed:category:)` for "surprise me".

**Landscape prototype** (`prototypes/landscape-rt`, f64 only): its `--loc` takes a name or
`re,im`. Its names map to places: `seahorse` ≈ `julia-island` centre (Beyer's seahorse
point), `elephant` = `elephants`, `minibrot3` = `airplane`, `spiral` ≈ the mirror image of
`two-arm-spiral`. Any place within f64 starts with
`--loc=re,im --scale S`, S = span / 30 (it starts 2 S off the boundary, fog at 60 S);
`almondbread places --format landscape` prints these lines.

## Spiral Galaxy: a place found by hand (2026-10-21)

The first place the author found by exploring rather than from a source: `spiral-galaxy`,
re -0.832130215420311320156874621447, im -0.207171646604986446426901492118, just outside the
period-2 disc (0.267 from its centre -1, radius 1/4). A wheel of arms made of seahorse-tail
spirals around a dense core; `locate --radius 3.7e-23` finds a period-461 nucleus about 1.5e-24 (0.04 search radii)
from the centre (size 1e-25.3, below a point at this framing). The look is `spectrum`,
the one the app had when the view was found (the persisted settings in the
`ch.ractive.almondbread` defaults matched the Spectrum look exactly).

**Span from the app's scale.** The HUD's scale is complex units per view point and the canvas
fills the window (it ignores the safe area), so span = scale × the window's shorter side:
6.79538e-26 × 1083 pt = 7.36e-23. In a window of the same size the place opens on exactly that
view; elsewhere it is fitted to the shorter side as every place is. The HUD's zoom counts from
3/800 per point, a place's depth from span 3, so the HUD shows 10^22.7 where the catalogue
lists 22.6 (the `place_zoom_offset` of the window). Render cost is low (about 0.2 s GPU at
1600x1000 with 3x3 supersampling, maxIter 18,000); it is also an App Store screenshot draft
(`--ui-demo=place-spiral-galaxy`, `AppStoreShotsUITests.test10SpiralGalaxy`).

## The catalogue

<!-- places-table -->
76 places (landmark 11, valley 14, spiral 8, dendrite 4, minibrot 9, julia-island 17, deep 13; the four endless dives are listed above). By magnification: < 1e5: 36, 1e5-1e13: 20, 1e13-1e30: 7, 1e30-1e300: 5, >= 1e300: 8. Depth is log10 of the magnification relative to the whole set (span 3); `almondbread places --format tsv` prints centres and spans.

| id | name | category | depth (log10 zoom) | look | source |
| --- | --- | --- | --- | --- | --- |
| `whole-set` | The Whole Set | landmark | 0.1 | aurora | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 00 |
| `head-and-shoulders` | Head and Shoulders | landmark | 0.9 | lagoon | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 01 |
| `pi-at-the-neck` | π at the Neck | landmark | 1.6 | aurora | [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md) (Boll 1991); ε = 0.02 on -3/4 + iε, [π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md) |
| `pi-at-the-cusp` | π at the Cusp | landmark | 3.7 | aurora | [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md); ε = 2e-4 on 1/4 + ε, [π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md) |
| `the-spike` | The Spike | landmark | 0.3 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2F(1/2B) (the spike / main antenna) |
| `third-bulb` | The 1/3 Bulb | landmark | 0.9 | dusk | Nucleus of period 3 (root of c³ + 2c² + c + 1), computed with almondbread locate (generated) |
| `quarter-bulb` | The 1/4 Bulb | landmark | 0.8 | moss | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.1/4 |
| `fifth-bulb` | The 1/5 Bulb | landmark | 1.1 | spectrum | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.1/5 |
| `two-fifths-bulb` | The 2/5 Bulb | landmark | 0.9 | classic | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.2/5 |
| `five-eighths-limb` | The 5/8 Limb | landmark | 1.3 | aurora | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2F(5/8B) |
| `feigenbaum-point` | Feigenbaum Point | landmark | 3.2 | spectrum | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Feigenbaum point |
| `seahorse-valley` | Seahorse Valley | valley | 1.9 | aurora | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 02 |
| `seahorse` | A Seahorse | valley | 2.4 | lagoon | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 03 |
| `elephant-valley` | Elephant Valley | valley | 1.2 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Elephant Valley (R2.C(0)) |
| `elephant-herd` | Elephant Herd | valley | 2.3 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.C(0) |
| `elephants` | Among the Elephants | valley | 3.5 | moss | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.C(0) |
| `triple-spiral-valley` | Triple Spiral Valley | valley | 2.3 | nebula | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.C(1/3) |
| `quad-spiral-valley` | Quad Spiral Valley | valley | 2.4 | ice | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Quad Spiral Valley and Rational Coordinates (1/4 + 1/2 i) (generated) |
| `scepter-valley` | Scepter Valley | valley | 2.6 | classic | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.1/2.C(1/2) (Scepter Valley) |
| `scepter` | A Scepter | valley | 3.5 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.1/2.C(1/2) |
| `scepter-crown` | Scepter Crown | valley | 3.8 | ember | [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md) |
| `double-scepter-valley` | Double Scepter Valley | valley | 2.8 | lagoon | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.1/3.C(1/2) (Double Scepter Valley) |
| `peacock-eye` | Peacock Eye | valley | 2.6 | spectrum | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Peacock eye |
| `shepherds-crook` | Shepherd's Crook | valley | 5.1 | graphite | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Shepherds crook |
| `double-hook` | Double Hook | valley | 4.5 | aurora | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 06 |
| `spiral-sanctuary` | Spiral Sanctuary | spiral | 2.7 | lagoon | [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md) |
| `triple-spiral` | Triple Spiral | spiral | 3.2 | dusk | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2.C(1/3), figure 2 |
| `seahorse-tail` | Seahorse Tail | spiral | 3.2 | nebula | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 04 |
| `tail-part` | Inside the Tail | spiral | 3.7 | moss | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 05 |
| `two-arm-spiral` | Two-Arm Spiral | spiral | 4.5 | lagoon | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `satellite-double-spiral` | Satellite Double Spirals | spiral | 8.4 | aurora | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 11 |
| `spiral-wheel` | Spiral Wheel | spiral | 9.0 | ember | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 12 |
| `spiral-galaxy` | Spiral Galaxy | spiral | 22.6 | spectrum | found by the author in the app (2026-10-20), see "Spiral Galaxy" below |
| `dendrite-i` | The Tip at i | dendrite | 1.7 | ice | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `three-way-junction` | Three-Way Junction | dendrite | 2.8 | dusk | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `antenna-junction` | Antenna Crossroads | dendrite | 1.8 | ember | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `snowflake` | Snowflakes | dendrite | 3.5 | ice | [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md) |
| `airplane` | The Airplane | minibrot | 1.9 | nebula | Nucleus of period 3 on the real axis, computed with almondbread locate (generated) |
| `period-4-minibrot` | Period-4 Minibrot | minibrot | 2.9 | classic | benchmark set ([almondbread benchmarks and verification record](benchmarks.md)), `locate` (generated) |
| `third-limb-island` | Island on the 1/3 Limb | minibrot | 1.7 | moss | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): R2F(1/3B1)S |
| `satellite` | Satellite | minibrot | 5.4 | aurora | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 07 |
| `satellite-antenna` | Satellite Antenna | minibrot | 6.1 | lagoon | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 08 |
| `satellite-head` | Satellite Head and Shoulders | minibrot | 6.8 | nebula | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 09 |
| `satellite-seahorse-valley` | Satellite Seahorse Valley | minibrot | 7.5 | dusk | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 10 |
| `pine-compass` | Pine Compass | minibrot | 12.3 | moss | [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md) (generated) |
| `crystal-compass` | Crystal Compass | minibrot | 14.2 | ice | [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md) (generated) |
| `embedded-julia` | Embedded Julia Set | julia-island | 5.2 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Embedded Julia Set |
| `cantor-island` | Cantor Island | julia-island | 6.7 | moss | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Favorite Views 1992 |
| `paramecium` | Paramecium | julia-island | 6.3 | ice | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Paramecia |
| `cauliflower-medallion` | Cauliflower Medallion | julia-island | 6.0 | moss | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Cauliflower Medallion |
| `triple-spiral-medallion` | Triple Spiral Medallion | julia-island | 6.6 | nebula | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Triple Spiral Medallion |
| `elephant-medallion` | Elephant Medallion | julia-island | 7.0 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Elephant Medallion |
| `seahorse-medallion` | Seahorse Medallion | julia-island | 8.4 | spectrum | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Seahorse Medallion |
| `second-order-julia` | Julia Within Julia | julia-island | 8.3 | classic | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Second-Order Embedded Julia Set |
| `two-fold-julia` | Two-Fold Julia | julia-island | 9.9 | dusk | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): 2-fold Embedded Julia Set |
| `julia-island-tail` | Island in the Tail | julia-island | 9.8 | nebula | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 13 |
| `julia-island` | Julia Island | julia-island | 11.6 | lagoon | [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md) 14 |
| `spiral-ring-island` | Ring of Spirals | julia-island | 11.2 | aurora | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Favorite Views 1994 |
| `four-fold-julia` | Four-Fold Julia | julia-island | 14.4 | spectrum | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): 4-fold Embedded Julia Set |
| `sunflower-island` | Sunflower Island | julia-island | 15.1 | dusk | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Favorite Views 1993 |
| `third-order-satellite` | Third-Order Satellite | julia-island | 19.0 | nebula | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Favorite Views 1994 |
| `copper-mandala` | Copper Mandala | julia-island | 21.7 | ember | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Second-Order Embedded Julia Set |
| `frost-mandala` | Frost Mandala | julia-island | 27.0 | ice | [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md): Favorite Views 1994 |
| `seahorse-minibrot-1e31` | Minibrot of the Seahorse | deep | 31.2 | aurora | benchmark set ([almondbread benchmarks and verification record](benchmarks.md)), `locate` (generated) |
| `spiral-1e50` | Spiral at 1e50 | deep | 50.5 | lagoon | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e91` | Needle Minibrot | deep | 91.5 | aurora | benchmark set ([almondbread benchmarks and verification record](benchmarks.md)), `locate` (generated) |
| `junction-1e100` | Junction at 1e100 | deep | 100.5 | nebula | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `starburst-1e112` | Starburst | deep | 112.5 | dusk | Generated: almondbread locate --radius 1e-60 at M(4,1) (generated) |
| `junction-1e300` | Junction at 1e300 | deep | 300.5 | dusk | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e302` | Needle Minibrot at 1e302 | deep | 302.5 | aurora | benchmark set ([almondbread benchmarks and verification record](benchmarks.md)), `locate` (generated) |
| `spiral-1e1000` | Spiral at 1e1000 | deep | 1000.5 | ice | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e1000` | Needle Minibrot at 1e1000 | deep | 1000.9 | spectrum | benchmark set ([almondbread benchmarks and verification record](benchmarks.md)), `locate` (generated) |
| `pinwheel-1e1500` | Pinwheel at 1e1500 | deep | 1500.5 | moss | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) (c = i) + `locate --radius 1e-1000` (generated) |
| `star-1e2000` | Sixteen-Point Star | deep | 2000.5 | ember | `locate --misiurewicz 5,1` + `locate --radius 1e-1333` (generated) |
| `swirl-1e3000` | Swirl at 1e3000 | deep | 3000.5 | lagoon | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) (c = i) + `locate --radius 1e-1714` (generated) |
| `rock-bottom` | Rock Bottom | deep | 3007.5 | classic | [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md) (M(4,1)) + `locate --radius 1e-2103` (generated) |

# Citations

- [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](../sources/beyer-mandelbrot-zoom-sequence.md)
- [Mu-Ency: named features and favourite views (coordinates)](../sources/muency-named-places.md)
- [Wikipedia: Misiurewicz point](../sources/wikipedia-misiurewicz-point.md)
- [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](../sources/robotmoon-mandelbrot-set.md)
- [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md)
