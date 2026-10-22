---
title: "F5: Mandelbar (Tricorn) and the non-conformal path"
type: plan
milestone: F5
status: in-progress
priority: 5
effort: M
appeal: medium
audience: [enthusiasts, education]
depth: "unlimited"
realtime-ipad: likely
reuse: "high: the three-product reference split carries over; new 2×2 BLA, dual-number derivatives, Jacobian Newton"
depends-on: ["plans/f2-pluggable-formulas"]
risks:
  - "2×2 derivatives double the derivative state per pixel (registers, speed)"
  - "medium appeal on its own; its value is the path it builds for F6"
sources: ["synthesis/other-fractal-types", "formulas/mandelbar", "sources/mathr-deep-zoom-page", "techniques/bla", "techniques/interior-detection", "synthesis/benchmarks"]
tags: [formulas, mandelbar, tricorn, non-conformal, bla]
created: 2026-10-19
updated: 2026-10-22
---

# F5: Mandelbar (Tricorn) and the non-conformal path

Status: built, awaiting the user's review (in-progress); see "As built". After [F4: Deep Julia sets](f4-deep-julia.md).

## Goal

Build the non-conformal machinery (2×2 real BLA matrices, 2×2 derivatives, Newton with a
Jacobian) on the simplest formula that needs it, `z ← conj(z)² + c`. The Tricorn is "the
cheapest way to build the non-conformal path; its own appeal is medium"
(src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).

## Scope

- Formula `Mandelbar`; conjugated perturbed step; `kConformal = false` pipelines.
- BLA with a 2×2 matrix that is a scaled reflection, so the validity radius equals the
  conformal one ([Mandelbar (Tricorn)](../formulas/mandelbar.md)).
- DE and interior with dual numbers (2 or 4 dual parts) and a matrix norm
  (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §5, §6).
- Period detection and Newton nucleus with a 2×2 Jacobian in `selection.rs` (seek works).
- Oracle, f64 reference, overlay; a small places set.

## Non-goals

- Burning Ship's `diffabs` and fold caps ([F6: Burning Ship family](f6-burning-ship.md)).
- LA, local SA seeds unless measured to pay (seeds are compiled out for non-conformal formulas
  at first).

## Design sketch

- `PixelState` carries a 2×2 derivative (four floats instead of two) when `kConformal = false`
  ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §6); Mandelbrot pipelines keep the two-float state.
- The reference reuses `parallel.rs`'s three products (X², Y², (X+Y)²): Mandelbar needs the
  same terms with a sign change (inference in [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §1).
- BLA table entries become 2×2 for non-conformal formulas; lookup code shared, entry width per
  pipeline.

## Affected modules

- `almondbread-math`: `orbit.rs`, `parallel.rs` (sign), `selection.rs` (2×2 Jacobian Newton
  and period).
- `almondbread-core`: `reference.rs`, `oracle.rs`, `orbit.rs`, `bla.rs` (matrix variant),
  `cycles.rs` and the interior probe (2×2), `seek.rs`, `places_data.rs`.
- `almondbread-render`: `mandelbrot.metal` (step functions, 2×2 derivative state, matrix BLA
  walk), `gpu_types.rs` (matrix entries), `metal/mod.rs`.
- FFI capabilities; Swift picker entry.

## Verification

- Oracle fixtures; `verify` vs oracle at shallow, 1e-50, 1e-300, 1e-1000; DE images looked at
  (dual-number DE correct along the boundary).
- BLA skip-off diff (as M4 did: "skip-off diff clean", [Roadmap](../roadmap.md) M4).
- Mandelbrot and Multibrot fast paths: bench A/B ≈ 1.0, bit-identical fields; `make regress`.
- `make interaction-test`, `make selftest`, iOS device check build.

## Review gate

Short: the user looks at Tricorn views and seek on the Mac. The real gate is that F6 can start
on a proven 2×2 path.

## Risks

- Register pressure from the 2×2 state; measure occupancy and GPU ms against Mandelbrot at the
  same view.
- Dual-number DE subtle at folds of the conjugation (none for Mandelbar; matters in F6).

## Open questions

- Is the matrix BLA lookup fast enough to pay at the f64 reference's depths, where Mandelbrot's
  BLA did not ([Roadmap](../roadmap.md) M4)?

## As built (2026-10-22, awaiting review)

Built on branch `worktree-agent-a68564f5421dd0d08` from main fb4fb56 (F3 + F4). Measurements in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F5"; the formula's maths in [Mandelbar (Tricorn)](../formulas/mandelbar.md) "As built in
almondbread (F5)". Differences from the sketch above:

- **Wirtinger pairs, not 2×2 matrices or dual numbers.** Every derivative of a non-conformal
  formula is a real-linear map `d ↦ p d + q conj(d)` (`almondbread_math::Linear`, `LinearX`
  in extended range; `from_matrix` / `matrix` convert). Two complex numbers carry what a 2×2
  matrix carries, the existing complex and floatexp arithmetic serves unchanged on the CPU and the
  GPU, and the singular values are free (`|p| ± |q|`). Mandelbar's step Jacobian is `(0, conj(2z))`.
- **Gate.** The plan named none; run first as one: the CPU model of float32 perturbation and the
  matrix BLA held against plain perturbation (class mismatch 0, |Δμ| below the float32 control,
  1e-13 to 1e-40) and the deep oracle matched direct BigFloat iteration pixel for pixel to
  1e-320. Passed, then the GPU path was built.
- **Kernel** (`mandelbrot.metal`, formula 3; `kConformal = kFormula != 3u`, derived, so every
  conformal pipeline folds the new code away). The delta step `conj((2Z + z) z) + c` in every
  regime; the q parts of `dz/dc` and `dz/dz1` live in `ncBuf` (binding 8, a `float4` per live
  slot, two halves by `probeHalf` like the probe state), so `PixelState` and the Mandelbrot
  pipelines keep their layout; matrix BLA tables reinterpret bindings 15 / 17 (`BlaEntryM`,
  `BlaEntryMX`). DE: the escaped z turns `(p, q)` into the equivalent complex derivative
  `p + conj(q) z / conj(z)` and the conformal DE / normal code runs on it. Interior test on
  `max|p| + max|q|`.
- **Interior probe on even periods** (not in the plan): `f^k` is holomorphic in z for even k, so
  the attracting-cycle probe evaluates even candidates only (an odd cycle proves as `f^(2P)`),
  with Mandelbrot's λ and F'' recursions conjugated after each step. Component Newton (CPU
  periods from `cycles.rs`) stays off.
- **BLA** (`bla.rs` "Non-conformal"): `Merged{M,MX}` with `Linear` / `Xl` coefficients, the
  radius rule with sup / inf singular values; `BlaTable{,X}::matrix` holds the entries, `data()`
  and `len()` serve either kind.
- **Newton and seek** (`selection.rs`): Jacobian Newton `c ← c - J^{-1} z_p`, the exact finish on
  registers (`JacobianNewton`), ball period detection unchanged (its bound holds), first-order
  with `sup|J|`. Size `1 / (|l| sqrt|det dz_p/dc|)`, no rotation. Seek has no shape test for a
  non-conformal set (no orientation to place a test point): every component counts, and when the
  "next smaller" one is the framed one again it goes on from it (`seek_for`). Map clicks snap to
  the edge of the minibrot along +re (bisection on escape).
- **Capabilities** (`FormulaCapabilities::MANDELBAR`): perturbation, rebasing (one reference),
  `BlaForm::Matrix`, DE, derivative interior test, probe, seek, orbit overlay; no LA, SA seeds,
  atom-domain interior colourings, closed-form components, π meter, rays, content, Julia panel.
- **Picker, links, places.** `formula_options()` lists Mandelbar last (family "mandelbar", title
  `z̄² + c`, no power picker); copied coordinates carry `formula mandelbar`; home view centred on
  -1/2 (`Viewport::fitting_for`: Reset, the autopilot's way out, the mini-map's whole level).
  Six places, all from automated runs (`seek` from the whole set, one `locate` beside the tip),
  with thumbnails: the whole set, small Tricorns of periods 3 and 7 on the antenna, skewed
  Mandelbrot sets of periods 20, 620 (1e-22) and 86 (1e-101).
- **Fix found on the way.** Placing the non-conformal BLA block before the conformal one in the
  plain loop slowed the plain Mandelbrot pipelines 5–17 % although it folds away; after it, A/B
  1.003 (benchmarks "F5").
- **`SchedulerConfig::bla`** (tests only): false builds no BLA table, the skip-off diff.

**Non-conformal interface for F6.** What a new non-conformal formula supplies, file by file:

1. `almondbread-math/src/formula.rs`: the variant, `is_conformal() == false`, `kernel_code()`
   (the kernels test `kFormula == N`, never `!= 0`), `name` / `parse`, `bailout` /
   `smooth_offset`, `step` / `step_x` (direct iteration), `delta_step` / `delta_step_scaled` /
   `delta_step_x` (the perturbation step: Burning Ship's `diffabs` goes here), `jacobian` /
   `jacobian_x` (the step's Jacobian at z as a `Linear`; Burning Ship's is `2z` or Mandelbar's
   `(0, conj(2z))` by the signs of `x y`, inference).
2. `orbit.rs` / `parallel.rs`: the reference loops (Mandelbar negates one product; the three-thread
   split takes a flag).
3. `selection.rs`: `Stepper::step` (the register step), `JacobianNewton` (the register Jacobian,
   Mandelbar's only so far: add the formula's case).
4. `core/bla.rs`: `single_radius` (Burning Ship's fold caps `|X|`, `|Y|` go here); merging and
   lookup are generic.
5. `mandelbrot.metal`: `kConformal` (add the code), `fStep` / `fStepX` / `fStepXT`, `ncMul` /
   `ncMulX` (the Jacobian applied to `(p, q)`); BLA, DE, interior test and the ncBuf state are
   generic. The probe's conjugation and even-period rule are Mandelbar's (an antiholomorphic
   step); a formula whose steps mix both kinds needs its own rule there.
6. `core/formula.rs`: capabilities, `family` / `family_title` / `title`, `selectable`;
   `viewport.rs` `fitting_for`; `places_data.rs`; `seek.rs` (the shape test is off for every
   non-conformal formula).

**Unverified.** A foreground session on the Mac, the iPad and a device (the review gate); the
period-620 minibrot's interior is 17 % unresolved at the auto limit (the probe's budget, see
benchmarks); seek frames satellite components too (no shape test); the minibrot size estimate's
accuracy for strongly skewed copies; Mandelbar GPU ms on a quiet machine.

## Effort breakdown

M overall (1–2 weeks).

- 2×2 derivative state, dual numbers, matrix BLA: 4 days.
- Jacobian Newton and period in math: 2 days.
- Oracle, places, UI: 1 day.
- Verification: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Mandelbar (Tricorn)](../formulas/mandelbar.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [Interior detection](../techniques/interior-detection.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
