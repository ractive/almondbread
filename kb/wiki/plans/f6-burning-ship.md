---
title: "F6: Burning Ship family"
type: plan
milestone: F6
status: planned
priority: 6
effort: L
appeal: high
audience: [deep-zoom-creators, artists, enthusiasts, casual]
depth: "unlimited; slow at deep needles"
realtime-ipad: likely
reuse: "medium: the non-conformal path from F5; diffabs at every step site; no SA seeds"
depends-on: ["[[plans/f2-pluggable-formulas]]", "[[plans/f5-mandelbar]]"]
risks:
  - "deep needle in float32: full floatexp steps when either part of Z is small; detection unresolved even in KF"
  - "local SA seeds do not work on folded regions"
  - "autopilot, target lock and journeys tuned on Mandelbrot"
  - "popularity claim is a forum anecdote (unverified)"
sources: ["[[synthesis/other-fractal-types]]", "[[formulas/burning-ship]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/kf-manual]]", "[[sources/fractalshades-readme]]", "[[sources/hpdz-animations]]", "[[sources/fraktaler-3-site]]"]
tags: [formulas, burning-ship, non-conformal, bla, needle]
created: 2026-10-19
updated: 2026-10-19
---

# F6: Burning Ship family

Status: planned, after [F5: Mandelbar (Tricorn) and the non-conformal path](f5-mandelbar.md). The payoff of the formula track.

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
  [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)).
- Which abs variants are worth a picker entry?

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
