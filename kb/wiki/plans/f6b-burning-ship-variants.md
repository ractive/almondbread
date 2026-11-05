---
title: "F6b: the Burning Ship's variants (Celtic, Buffalo, Perpendicular)"
type: plan
milestone: F6b
status: in-progress
priority: 6.5
effort: M
appeal: medium
audience: [deep-zoom-creators, artists, enthusiasts]
depth: "unlimited (verified to 1e-1000 on the needle)"
realtime-ipad: likely
reuse: "high: F6's fold path (diffabs at the delta's scale, split reference, matrix BLA, Jacobian Newton); new: their fold cases, sign-pattern Jacobians, exact R = X^2 - Y^2 per reference point"
depends-on: ["plans/f6-burning-ship"]
risks:
  - "Celtic's and Buffalo's real fold needs R = X^2 - Y^2 from the high-precision orbit: rounded X and Y lose it near |X| = |Y|"
  - "kernel formula codes 5-7 may collide with codes other branches add"
  - "the variants' references run single-threaded (no three-thread squaring)"
sources: ["formulas/burning-ship", "plans/f6-burning-ship", "sources/kf-manual", "sources/mathr-deep-zoom-theory", "sources/mathr-deep-zoom-page", "synthesis/benchmarks"]
tags: [formulas, burning-ship, celtic, buffalo, perpendicular, non-conformal, bla]
created: 2026-10-31
updated: 2026-10-31
---

# F6b: the Burning Ship's variants (Celtic, Buffalo, Perpendicular)

Status: built 2026-10-31, awaiting the user's review; see "As built". After
[F6: Burning Ship family](f6-burning-ship.md), which built the ship alone and called the other
members "mostly mechanical" follow-ups.

## Goal

The three best-known members of the Burning Ship family beside the ship, deep and real-time
with the ship's engine: Celtic (folds the real part), Buffalo (folds both parts) and the
perpendicular Burning Ship (folds at the real axis), each picked in a second menu of the
Burning Ship family, with a handful of places and the ship's seek and autopilot.

## Scope

- Formulas `Celtic`, `Buffalo`, `Perpendicular` (power 2), kernel formulas 5, 6, 7, defined as
  Kalles Fraktaler's formula list defines them (src:
  [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §Formulas), in the ship's orientation.
- Their fold cases in every perturbation step (f64 oracle, floatexp, float32 plain and
  rescaled), their Jacobians (DE, interior derivative, BLA, Newton), BLA fold caps.
- Places, the picker's second menu, capabilities as the ship's (what is not built is hidden).
- The gate before the GPU: the deep oracle against direct BigFloat iteration at 1e-30, 1e-100
  and 1e-300, across a fold.

## Non-goals

- The cubic and higher-power abs formulas, Mandelbar Celtic, perpendicular Celtic / Buffalo /
  Mandelbrot, hybrids ([F7: Hybrid formulas via an opcode model](f7-hybrids.md)).
- Journeys, tours and autopilot tuning (the ship has none either: `content: false`).

## Design sketch

In our coordinates (the literature's imaginary part negated, as for the ship, so every member
is drawn the way KF draws it):

| Formula | KF definition | Ours | Folds |
|---|---|---|---|
| Celtic | `w = z^2; z = abs(u) + i v + c` | `x' = abs(x^2 - y^2) + cx`, `y' = 2xy + cy` | real part at `abs(x) = abs(y)` |
| Buffalo | `w = z^2; z = abs(u) + i abs(v) + c` | `x' = abs(x^2 - y^2) + cx`, `y' = cy - 2 abs(xy)` | both |
| Perpendicular | `z = (x - i abs(y))^2 + c` | `x' = x^2 - y^2 + cx`, `y' = 2x abs(y) + cy` | at `y = 0` |

Celtic commutes with conjugation (symmetric in the real axis), so the flip changes nothing for
it. The perturbed steps use `diffabs(a, d) = |a + d| - |a|` decided at the delta's scale as
F6 does: Celtic and Buffalo fold `Re((2Z + z) z)` against `R = X^2 - Y^2`, Buffalo the imaginary
part against `X Y` as the ship, Perpendicular rewrites `x|Y + y| - X|Y| = x|Y| + (X + x)
diffabs(Y, y)` and folds against `Y`. Each step's Jacobian is `diag(±1, ±1)` times
multiplication by `2z`: `2z`, Mandelbar's reflection, `-2z` or the negated reflection by the
signs the step decides.

## Affected modules

- `almondbread-math`: `formula.rs` (variants, `folds_re`, `fold_signs`, `fold_values`, hooks),
  `orbit.rs` (`orbit_loop_fold`, the `fold_re` output, `compute_with_fold`,
  `compute_double_exp_fold`), `selection.rs` (register step, Jacobian Newton).
- `almondbread-core`: `formula.rs` (capabilities, picker, `formula_text`), `reference.rs` (`fold`
  per point), `oracle.rs` (exact fold values), `bla.rs` (fold caps), `viewport.rs` (home views,
  set boxes), `places_data.rs` (17 places).
- `almondbread-render`: `mandelbrot.metal` (formulas 5-7), `scheduler/reference_gpu.rs` (two
  slots per point for Celtic and Buffalo), `scheduler.rs` (their slots bound in the plain kernel).
- FFI tests; Swift `FractalRow` (the variant menu), `ControlSearch` (search words).

## Verification

- Gate: oracle against direct BigFloat per pixel (on the needle, beside it, across Buffalo's
  diagonal fold, at a Celtic minibrot); a negative control with R rounded from X and Y.
- `verify` at 1e-30, 1e-100, 1e-300, 1e-1000; `precision` where escape counts differ.
- GPU test at every place with BLA on and off; normals against the smooth count.
- Mandelbrot unchanged: its pipelines compile to the same code (formula 0 never reaches a
  variant branch); bench A/B.

## Review gate

The user zooms each variant on the Mac (and the iPad): the shapes, the variant menu, the places,
seek and the autopilot.

## Risks

- R's precision (handled: exact per point, see "As built").
- Chaos: away from the needle most deep views are chaotic at their sampling (as the ship's).

## Open questions

- Do the variants want journeys and tours (the ship's content work is open too)?
- Celtic's fold lines are diagonals; does the user want a softened crease (the ship's proposal)?

## Effort breakdown

M (about a week): maths and gate 2 days, kernel and GPU checks 2 days, places, UI and kb 1-2 days.

## As built (2026-10-31, awaiting review)

Built on branch `worktree-agent-a832e2234f2efa2b4` from main 4d846fa. Measurements in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F6b"; the maths in
[Burning Ship and abs variations](../formulas/burning-ship.md) "Variants". Our own design and
measurements unless cited.

- **Members.** `Formula::Celtic`, `Buffalo`, `Perpendicular` (names `celtic`, `buffalo`,
  `perpendicular`; kernel formulas 5, 6, 7; `is_ship_variant`). One picker family "Burning
  Ship" with a second menu: Burning Ship, Celtic, Buffalo, Perpendicular (the ship's title is
  now its name; `formula_text` gives each formula as maths, shown in the menu's help).
- **Exact R.** The new problem the ship did not have: the real fold compares `R = X^2 - Y^2`
  with a perturbation of the delta's size, and `R` rounded from rounded `X`, `Y` carries an
  absolute error of `2^-53 |Z|^2` (float32: `2^-24`) wherever `|X|` and `|Y|` are close. A view
  1e-30 wide whose centre lies 3.7 pixels off a fold line reads `R_1 ~ 1e-30` as rounding noise
  and folds half its pixels the wrong way (negative control: 327 of 384 escape counts right).
  So the variants' reference loop (`orbit_loop_fold`, the multiplication form on one thread)
  writes each point's `R` from the high-precision `(X + Y)(X - Y)`, rounded once; the CPU
  oracle reads it (`compute_double_exp_fold`), the references keep it (`Reference::fold`, f64
  references from f64), and the GPU gets it in the floatexp slots: two per point for Celtic
  and Buffalo (the split point, then `{R mantissa, R exponent}`), bound in the plain kernel
  too. Metal has no free buffer index (0-30 in use), so no new binding.
- **Kernel.** `fStepFoldV` (float32, plain and rescaled, fold values `(R / s, Q / s)` from
  `foldValuesAt`), `fStepXFoldV` (floatexp), `foldSigns` / `foldSignsX` and `ncMulSigns` /
  `ncMulSignsX` (the Jacobian by the step's own sign tests, F6's lesson), `refSlot`, `foldR`,
  `foldY`. The ship's code (formula 4) and every other pipeline are unchanged (`kFold` replaces
  `kFormula == 4u` only where it reads the same).
- **BLA.** Fold caps per variant: `min(|X|, |Y|)` (Buffalo), `|Y|` (Perpendicular), and for the
  real fold `(||X| - |Y|| - 2^-20 (|X| + |Y|)) / sqrt 2` (Celtic, Buffalo: `|x| + |y| <= sqrt 2
  |z|`; the margin covers the table's float32 rounding), each halved. On Celtic's needle the
  BLA pays (the real fold is far there); on Buffalo's and the perpendicular needle (`Y = 0`) no
  block is valid, as for the ship.
- **Newton and seek.** The register Jacobian takes the signs from the registers exactly. Seek
  finds chains in each (Celtic periods 5, 11, 29, 87, 116, 261 to 1e-17; Buffalo 9, 19, 57,
  95, 228, 684; Perpendicular 8, 20, 60, 100, 220, 620 to 1e-19). Newton can run to the fixed
  point `c = 0` or to a lower period; seek tries other candidates, as for the ship.
- **Capabilities.** The ship's (`FormulaCapabilities::SHIP_VARIANT`): no attracting-cycle probe
  (mixed Jacobians), LA, SA seeds, atom domains, closed-form components, π meter, rays, Julia
  panel, Buddhabrot or content; DE, interior derivative, seek, orbit overlay, unresolved drawn
  as the set (measured: of the pixels unresolved at the whole sets' auto limit, 94-98 % never
  escape within 1e5 iterations). The orbit overlay reads `R` and each part's own exponent from
  the reference, as the oracle does (2026-11-05: `R` rebuilt from the probe's 24-bit parts put 26
  of 64 escape counts wrong across Buffalo's diagonal at 1e-12, the shared exponent 29 of 64 on
  the ship beside the needle at 1e-101; measured against direct BigFloat, release).
- **Gate, then GPU.** Oracle against direct BigFloat: 384 of 384 escape counts on the needle at
  1e-30, 1e-100 and 1e-300, beside it at 1e-100 and across Buffalo's diagonal at all three
  depths, max |Δμ| ≤ 1.1e-6; the Celtic minibrot 383 of 384 (the other a sensitive orbit:
  direct f64 is off by 0.44 there, the oracle by 2.8e-4). GPU `verify`: class mismatch 0.000 %
  on every needle view to 1e-1000; escape counts differ on 0.4-1.0 % of Celtic's and Buffalo's
  (a 0.001 px grid shift changes 2.3-2.6 %: chaos), 0.000-0.006 % of the perpendicular ship's.
- **Places.** Seventeen, ours, with thumbnails: Celtic (whole set, filament tree, lattice,
  antenna copy, spined copy, folded minibrot 29), Buffalo (whole set, forest, tree trunk,
  needle shelf, calf 10), Perpendicular (whole set, ship on the needle, spined copy, island 20,
  comet 37, rainbow needle 48).
- **Autopilot.** Flies all three from their whole sets (90 s simulations: target lock, 0 %
  uniform frames, 1e34.9).
- **Home views and limits.** Celtic `-0.84`, span 2.8 x 3.7; Buffalo `-0.82 + 0.63i`, 2.7 x 2.5;
  Perpendicular `-0.47 - 0.095i`, 3.4 x 2.5. Set boxes (the test holds every set tightly): Celtic
  re -2 .. 0.3295, im ±1.7075; Buffalo re -2 .. 0.3715, im -0.4555 .. 1.708; Perpendicular re
  -2 .. 1.0565, im -1.1915 .. 1.0045 (extreme points of 0.0005 grids, 3000 iterations).
- **Buffalo's shelf.** Just above the real axis a Buffalo orbit stays near it (`y' = cy - 2|x|y`
  keeps a small positive `y` small) and follows Mandelbrot's real line, so the set has a thick
  bounded shelf above the needle wherever the real line is bounded (the dark rectangles at its
  needle places); below the axis everything escapes. Oracle and GPU agree on it.

- **Mandelbrot unchanged.** Its pipelines never reach a variant branch; `bench` A/B against
  main (release both): sea-8007 0.99, dendrite-1e300 equal; fields byte-identical.
- **Merge note.** Kernel formula codes 5-7 and the `Formula` arms are new; a branch that adds
  its own formula codes has to renumber one side.

**Unverified.** A foreground session on the Mac and the iPad (the review gate); device GPUs.

# Citations

- [Burning Ship and abs variations](../formulas/burning-ship.md)
- [F6: Burning Ship family](f6-burning-ship.md)
- [F7: Hybrid formulas via an opcode model](f7-hybrids.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
