---
title: "F2: Pluggable-formula refactor"
type: plan
milestone: F2
status: planned
priority: 2
effort: M
appeal: low
audience: [developers]
depth: "unchanged (Mandelbrot stays formula 0)"
realtime-ipad: "yes"
reuse: "refactor of ~12 formula-specific modules; scheduler, host, navigator, colour and UI unchanged"
depends-on: []
risks:
  - "a slower Mandelbrot path if the function-constant fold does not happen"
  - "pipeline count multiplies by formula (cold shader cache, memory)"
  - "a refactor with no visible result invites scope creep"
sources: ["[[synthesis/other-fractal-types]]", "[[platform/metal-compute]]", "[[techniques/bla]]", "[[techniques/double-float]]", "[[synthesis/benchmarks]]"]
tags: [formulas, architecture, refactor, metal, kernels]
created: 2026-10-19
updated: 2026-10-19
---

# F2: Pluggable-formula refactor

Status: planned, after [[plans/f1-julia-view]]. Enables [[plans/f3-multibrot]] and every
milestone after it. Design from [[synthesis/other-fractal-types]] §6.

## Goal

Make the formula a parameter of the engine without changing a single bit or a single
microsecond of Mandelbrot rendering. Mandelbrot stays formula 0 and compiles to exactly
today's code; each new formula is a set of capabilities, and what a formula lacks is hidden in
the UI rather than faked (src: [[synthesis/other-fractal-types]] §6 "Principle").

## Scope

- Kernel: factor the ~10 inline step sites of `iterate` and friends into inline functions
  (`fStep` float2, `fStepX` floatexp, `fDeriv`, `fJac`, the seed-SA coefficient recurrences),
  each branching on a new function constant `kFormula` (uint). Add `kConformal` (bool) now, used
  from [[plans/f5-mandelbar]] on.
- Maths: a `Formula` enum in `almondbread-math`, passed to `orbit.rs` and matched outside the
  limb-specialised loop; `parallel.rs` dispatch per formula; `selection.rs` period and Newton
  with a per-formula derivative hook.
- Core: `settings.rs` gains `formula`; per-formula hooks in `reference.rs`, `oracle.rs`,
  `orbit.rs`, `bla.rs`, `cycles.rs`; `la.rs`, `components.rs`, `pi_moment.rs` gated to
  Mandelbrot; places, tours and journey regions tagged with a formula.
- FFI: one `FormulaCapabilities` struct over UniFFI; Swift hides controls a formula lacks.
- Only Mandelbrot is implemented at the end of F2; the hooks are exercised by tests with a
  second, test-only formula (Multibrot p = 2 written through the generic path, which must equal
  Mandelbrot).

## Non-goals

- Any new user-visible formula (F3 is the first).
- Runtime MSL generation for hybrids ([[plans/f7-hybrids]]).
- 2×2 BLA and dual numbers (F5).
- Performance work on the Mandelbrot path beyond keeping it equal.

## Design sketch

- Function constants already specialise one MSL source into pipelines (`kDE`, `kInterior`,
  `kBLA`, `kDeep`, `kInteriorColor`, `kSeeded`, `kProbe`, `kLA`, ...;
  [[synthesis/other-fractal-types]] §1, [[platform/metal-compute]]). A `kFormula` branch is
  folded at pipeline compile time, so the formula-0 pipeline is the current code.
- Pipelines are compiled lazily per formula; Mandelbrot's set stays warm as today.
- `Formula` is an enum, not a trait object, so the inner loops stay monomorphic.
- `FormulaCapabilities { perturbation, rebasing: One|PerPhase|CriticalOrbit, bla: Scalar|Matrix,
  la, sa_seeds, interior_derivative, interior_probe, closed_form_components, de, seek,
  atom_domain, pi_meter, external_rays, orbit_overlay, content }`.

## Affected modules

The ~12 formula-specific modules from [[synthesis/other-fractal-types]] §1:

- `crates/almondbread-math/src/orbit.rs`, `parallel.rs` (reference orbit and three-thread
  split), `selection.rs` (period, nucleus), `rays.rs` stays Mandelbrot-only.
- `crates/almondbread-core/src/reference.rs` (f64 reference), `oracle.rs` (CPU truth),
  `orbit.rs` (orbit overlay), `bla.rs`, `la.rs` (gate), `components.rs` (gate), `cycles.rs`
  and the interior probe, `seek.rs`, `pi_moment.rs` (gate), `places_data.rs`, `tour.rs`,
  `journey.rs` + `journey/regions.rs`, `settings.rs`.
- `crates/almondbread-render/src/shaders/mandelbrot.metal` (step sites), `metal/mod.rs`
  (pipeline keys), `gpu_types.rs`, `la_gpu.rs` (gate), `pi_meter.rs` (gate).
- `crates/almondbread-ffi/src/explorer.rs` (capabilities, formula setting).
- Swift: `ExplorerModel.swift`, `ControlCenterView.swift` (hiding by capability).

## Verification

This milestone is accepted only on proof that Mandelbrot is unaffected:

- Fields bit-identical before/after at every benchmark location (`make bench ARGS="--out before"`
  on main, the same on the branch, `verify --fields a.iter --against b.iter` for each), plain,
  BLA, LA, deep, seeded and probe pipelines all hit; the M7 precedent: "`bench` A/B geomean
  1.014, fields bit-identical" ([[synthesis/benchmarks]] M7).
- `make bench` A/B GPU ms geomean ≈ 1.0 (accept 0.98–1.02; repeat on a quiet machine, report
  per location).
- `make regress` and `verify --history zoom|out|pan|jump` at 0 % mismatch.
- `make interaction-test`: frame pacing and per-frame FFI cost unchanged.
- Cold shader cache: `ALMONDBREAD_KERNEL_SALT=x` first-frame time and pipeline count before and
  after.
- The test-only generic Multibrot p = 2 equals Mandelbrot in the oracle and on the GPU.
- `make rust-check`, `make core-check`, `make selftest` in both apps, iOS device check build.

## Review gate

The driver session reviews the A/B table and the diff size before F3 starts. The user sees
nothing new; the gate is "no regression in a real window" (a short foreground session on the
Mac and the iPad).

## Risks

- The compiler might not fold a `kFormula` branch inside an inline function called from a
  loop; check the pipeline's disassembly size or measure, and fall back to preprocessor
  specialisation per formula if needed.
- More pipelines: cold-cache time after an update (measure, compile lazily).
- Tagging content (places, tours, journeys) touches data files; keep it mechanical.

## Open questions

- One MSL file or one per formula family with shared includes?
- Does the capability struct live in core (platform-free) with the FFI only mirroring it? (Likely
  yes, so the CLI can hide features too.)

## Effort breakdown

M overall (1–2 weeks).

- Kernel factoring and pipeline keys: 3–4 days.
- Maths and core hooks, gates, settings: 2–3 days.
- Capabilities over FFI, Swift hiding: 1 day.
- A/B proof runs and fixes: 2 days.
