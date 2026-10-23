---
title: "F6: Burning Ship family"
type: plan
milestone: F6
status: in-progress
priority: 6
effort: L
appeal: high
audience: [deep-zoom-creators, artists, enthusiasts, casual]
depth: "unlimited; slow at deep needles"
realtime-ipad: likely
reuse: "medium: the non-conformal path from F5; diffabs at every step site; no SA seeds"
depends-on: ["plans/f2-pluggable-formulas", "plans/f5-mandelbar"]
risks:
  - "deep needle in float32: full floatexp steps when either part of Z is small; detection unresolved even in KF"
  - "local SA seeds do not work on folded regions"
  - "autopilot, target lock and journeys tuned on Mandelbrot"
  - "popularity claim is a forum anecdote (unverified)"
sources: ["synthesis/other-fractal-types", "formulas/burning-ship", "sources/mathr-deep-zoom-page", "sources/mathr-deep-zoom-theory", "sources/kf-manual", "sources/fractalshades-readme", "sources/hpdz-animations", "sources/fraktaler-3-site"]
tags: [formulas, burning-ship, non-conformal, bla, needle]
created: 2026-10-19
updated: 2026-10-23
---

# F6: Burning Ship family

Status: built, awaiting the user's review (in-progress); see "As built". After [F5: Mandelbar (Tricorn) and the non-conformal path](f5-mandelbar.md). The payoff of the formula track.

## Goal

Deep, real-time Burning Ship zooms (and its abs relatives: Celtic, Buffalo, Perpendicular, the
cubic ones) with the same engine. "The Burning Ship's ships and needles are the most popular
non-Mandelbrot deep-zoom subject" (unverified: forum anecdote; src:
[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a). No Apple-native app offers it deep and interactive
(§3 there).

## Scope

- Formulas `BurningShip`, then `Celtic`, `Buffalo`, `Perpendicular` (power 2), cubic variants
  if cheap.
- `diffabs` case analysis in the perturbed step at every kernel step site
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.2; [Burning Ship and abs variations](../formulas/burning-ship.md)).
- 2×2 BLA with fold caps `min(..., |X|, |Y|)`, halved for safety as Fraktaler 3 does
  (same sources).
- Local SA seeds off ("only good so long as the region is not folded", src:
  [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Series Approximation).
- Needle heuristic: when the share of steps with `min(|X|, |Y|)` tiny is high, run the floatexp
  kernel for the view (the deep-needle problem, src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Deep
  Needle; KF defaults "reference strict zero" on near the needle, src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Newton zooming with 2×2 Jacobians (works in Fraktaler 3, src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)),
  so seek carries over.
- A Burning Ship places catalogue (ships, needles, deep views to 1e-1000), journey and autopilot
  tuning, one tour.

## Non-goals

- Hybrids ([F7: Hybrid formulas via an opcode model](f7-hybrids.md)).
- Burning Ship Julia sets (later, after F4's pattern).
- LA for abs formulas.

## Design sketch

- `fStep` with `diffabs(X, x) = |X + x| − |X|` by sign cases, in float32 and floatexp.
- Fold-capped BLA radius per entry; the table is built per reference like today.
- Needle detection per view from the reference: count steps where `min(|X|,|Y|)` is below a
  threshold relative to the pixel spacing; above a share, pick the deep (floatexp) pipeline.
  Threshold found by experiment against the oracle.
- fractalshades demonstrates depth to 1e-2000 (src: [Fractalshades README](../sources/fractalshades-readme.md)); the
  first Burning Ship deep zoom (~2.35e-100) was brute force (src: [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)).

## Affected modules

- `almondbread-math`: `orbit.rs` (abs step on limbs), `parallel.rs` (`2|XY|` from the same
  products), `selection.rs` (2×2 Newton, period).
- `almondbread-core`: `reference.rs`, `oracle.rs`, `orbit.rs`, `bla.rs` (fold caps), `cycles.rs`
  and the interior probe, `seek.rs`, `places_data.rs` (new catalogue), `journey.rs` +
  `journey/regions.rs`, `autopilot/lock.rs` (thresholds), `tour.rs` + a tour file.
- `almondbread-render`: `mandelbrot.metal` (diffabs at the ~10 step sites, seeds compiled out),
  `scheduler.rs` (needle pipeline choice), `metal/mod.rs`.
- FFI capabilities; Swift picker, places category.

## Verification

- Oracle fixtures per formula; `verify` vs oracle at shallow, 1e-50, 1e-300, 1e-1000 and at
  two needle views; `precision` where they differ.
- BLA skip-off diff; needle heuristic on vs forced floatexp: same fields, speed table.
- `verify --history zoom|pan|jump`, new regress sessions for a ship and a needle.
- Mandelbrot fast path: bench A/B ≈ 1.0, bit-identical fields, `make regress`.
- `make interaction-test` (ship zoom, needle zoom), autopilot `fly` metrics on a Burning Ship
  view, `make selftest`, stills looked at.

## Review gate

Two gates. After the formula renders correctly to 1e-300: the user zooms ships and needles on
the Mac and the iPad (go for content work). After places, journeys and the tour: the user's
review of the content, as for Mandelbrot.

## Risks

- Deep needles (above): the hardest technical risk; may cap the "real time" claim to non-needle
  views.
- Autopilot and journeys may fly into folds or flat regions; needs tuning, measured with
  `autopilot` and `fly --log`.
- Popularity unverified; the review gate decides how much content to build.

## Open questions

- Needle detection heuristic in float32 (open in [Burning Ship and abs variations](../formulas/burning-ship.md) and
  [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)). Answered for
  almondbread by "As built": none is needed once the fold is decided at the delta's scale with
  an exact `X Y` (measured to 1e-1000).
- How should the chaotic bounded regions (unresolved, never provable) be coloured, and should the
  auto-iteration limit stop chasing them? Colouring answered after the first review (2026-10-23,
  "Review fixes" below): as the set. Chasing still open (numbers there).
- Which abs variants are worth a picker entry?

## As built (2026-10-23, awaiting review)

Built on branch `worktree-agent-a54eb563d94fc642b` from main 3e30f8c (F5 + BB). Measurements in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F6"; the maths in [Burning Ship and abs variations](../formulas/burning-ship.md) "As built in
almondbread (F6)". Differences from the sketch above:

- **Members.** The Burning Ship alone (`Formula::BurningShip`, kernel formula 4, picker family
  "Burning Ship", name `burningship`). Celtic, Buffalo and Perpendicular were not built: each
  needs its own fold cases (Celtic folds the real part `|x^2 - y^2|`, Buffalo both parts),
  Jacobian rule, places and checks; the code below makes them mostly mechanical, so they are a
  follow-up once the user has seen the ship.
- **Orientation.** `z <- (|x| - i|y|)^2 + c`: the literature's ship mirrored in the real axis, so
  it stands upright with the imaginary axis pointing up (the usual pictures flip the axis
  instead; we cannot flip a view). Coordinates are the literature's with `im` negated;
  documented on the formula page and in the picker's help.
- **Gate.** The plan named none; run first as one: the deep oracle (f64 deltas with the fold)
  against direct BigFloat iteration per pixel, on the needle at 1e-30, 1e-100 and 1e-320 per
  pixel and across the axis at 1e-100: 384 / 384 escape counts equal, max |Δμ| 1.2e-7. Passed,
  then the GPU path: `verify` on the needle at 1e-30, 1e-100, 1e-300 and 1e-1000 per pixel,
  class mismatch 0.000 %, escape-n differs ≤ 0.005 %.
- **No needle heuristic.** The fold is decided at the delta's scale (`diffabs(XY/s, d/s)`,
  `XY/s` saturated at `±2^100`), which float32 holds at any depth, and `XY` comes from a split
  floatexp reference (`FloatExpSplit`: each part its own exponent, the second in the GPU
  struct's padding word) where one part of `Z` is far below the other. So no step needs
  floatexp because `X` or `Y` is small, and no pipeline choice was needed. The needle is slow
  only because a reference on the axis (`Y = 0`) allows no BLA block.
- **Kernel** (`mandelbrot.metal`, formula 4; `kConformal = kFormula != 3u && kFormula != 4u`,
  so formulas 0–3 compile as before). `fStepFold` (float32, plain and rescaled), `fStepXFold`
  (floatexp, `XY` given), `foldScaled`, `foldXY` and `refAt` reading the split reference;
  `ncMulFold` / `ncMulFoldX` take `2z` where `x y < 0`, Mandelbar's reflection (`ncMul`)
  elsewhere, the sign from the step's own test (review fix below); everything else (matrix BLA
  walk, ncBuf state, DE, interior test) is F5's.
- **BLA.** `single_radius` and the deep table's single step gain the fold cap
  `min(|X|, |Y|) / 2`; a part flushed in the shared floatexp form caps to 0 (no block).
- **Newton and seek.** `JacobianNewton` takes the conformal step where `x y < 0`; the ball
  period methods hold, the box method does not (documented in the test). Seek finds chains of
  minibrots (from the ship: periods 31, 93, 124, 217, 403, 1209, 2015, 3224 down to 1e-24); it
  stops where Newton leaves the nucleus's smooth piece.
- **No probe.** The attracting-cycle probe is off (`FormulaCapabilities::BURNING_SHIP`): the step
  mixes holomorphic and antiholomorphic Jacobians, so neither Mandelbrot's nor Mandelbar's
  even-period rule holds. Large regions are bounded chaotic orbits anyway (no cycle to prove):
  they stay unresolved, and since the first review draw as the set (below).
- **Places.** Six, with thumbnails: the whole ship, the ship by the needle, a period-18 needle
  minibrot, little ships of periods 20 (1e-10) and 60 (1e-19) (they replaced 31 and 403 after the
  first review), the needle at 1e-100. Journeys,
  the tour and autopilot tuning were not built: they are the second review gate's content work
  (`content: false`); the autopilot already flies Burning Ship views (90 s simulation: target
  lock, 0 % uniform frames).
- **Home view** (`Viewport::fitting_for`): centre `-0.45 + 0.62i`, 3.6 × 2.6 (Reset, the
  autopilot's way out, the mini-map's whole level).

**What F7 hybrids would reuse.** `Formula::folds()` and the fold step (`diffabs` at the delta's
scale, `fStepFold` / `fStepXFold`), the split floatexp reference for any formula whose step reads
a product of the parts, the per-step Jacobian switch (`ncMulFold` by the step's sign), the fold cap in
`single_radius`, and the reference loops' `ABS` const generic (an abs is a sign drop on a product
register). A hybrid's per-phase step would pick among these per opcode.

**Review fixes (2026-10-23, after the user's first look).** Numbers in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F6 review fixes", maths on the formula page.

- **The ship place** takes the user's framing: `-1.7446738163860224 + 0.038088366410778681i`,
  span 0.11704 (0.000108068 per point across 1083 points).
- **Rectangles on the needle** (8e94x): a bug. The lighting / DE Jacobian took the fold sign from
  the float32 sum `Z + s w`, where `Y + y` flushes to 0 on the needle. Now the step's own sign
  test (`foldConformalAt`, `foldConformalX`); 28 % of the normals pointed the wrong way, 0 after.
- **Unresolved draws as the set** (capability `bounded_interior`, Burning Ship only): the hull's
  grey fog smear becomes the solid hull.
- **Little ships.** 31 and 403 sit in the hull's chaotic sea (noise at any sampling, the ship a
  skewed blob); 20 and 60, found by seek from the left mast, read as ships.
- **Chasing (proposal, not built).** Extension passes and compute further still chase the
  unresolved pixels. At the old period-403 view (1600 × 1000, `progress`) the view settles after
  10.4 s: the extension to 56 k resolves 6.3 % of the pixels in 2.2 s, compute further to 200 k
  another 2.4 % in 4.7 s (escape-time noise in the sea, from interior colour to colour); the
  period-60 place settles in 3.2 s, the whole ship in 1.9 s, the ship in 1.0 s. Skipping compute
  further for `bounded_interior` formulas would halve the worst case; the user decides.

**Unverified.** A foreground session on the Mac, the iPad and a device (the review gate); the
other abs variants; the review fixes in a foreground window (headless renders and tests only).

## Effort breakdown

L overall (3–6 weeks).

- Diffabs steps at all sites, fold-capped BLA: 1 week.
- Needle heuristic and deep pipeline choice: 1 week.
- Newton/seek with Jacobians, oracle, fixtures: 3–4 days.
- Places catalogue, autopilot and journey tuning, tour: 1–2 weeks.
- Verification and two review rounds: 3–4 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Burning Ship and abs variations](../formulas/burning-ship.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Fractalshades README](../sources/fractalshades-readme.md)
- [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
