---
title: "F2: Pluggable-formula refactor"
type: plan
milestone: F2
status: in-progress
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

Status: built on branch `worktree-agent-a6270c6cca3092d81` (2026-10-19, merged with main after
F1), awaiting the driver's review of the A/B table ("As built" below). Enables
[F3: Multibrot z^p + c](f3-multibrot.md) and every milestone after it. Design from [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §6.

## Goal

Make the formula a parameter of the engine without changing a single bit or a single
microsecond of Mandelbrot rendering. Mandelbrot stays formula 0 and compiles to exactly
today's code; each new formula is a set of capabilities, and what a formula lacks is hidden in
the UI rather than faked (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §6 "Principle").

## Scope

- Kernel: factor the ~10 inline step sites of `iterate` and friends into inline functions
  (`fStep` float2, `fStepX` floatexp, `fDeriv`, `fJac`, the seed-SA coefficient recurrences),
  each branching on a new function constant `kFormula` (uint). Add `kConformal` (bool) now, used
  from [F5: Mandelbar (Tricorn) and the non-conformal path](f5-mandelbar.md) on.
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
- Runtime MSL generation for hybrids ([F7: Hybrid formulas via an opcode model](f7-hybrids.md)).
- 2×2 BLA and dual numbers (F5).
- Performance work on the Mandelbrot path beyond keeping it equal.

## Design sketch

- Function constants already specialise one MSL source into pipelines (`kDE`, `kInterior`,
  `kBLA`, `kDeep`, `kInteriorColor`, `kSeeded`, `kProbe`, `kLA`, ...;
  [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §1, [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)). A `kFormula` branch is
  folded at pipeline compile time, so the formula-0 pipeline is the current code.
- Pipelines are compiled lazily per formula; Mandelbrot's set stays warm as today.
- `Formula` is an enum, not a trait object, so the inner loops stay monomorphic.
- `FormulaCapabilities { perturbation, rebasing: One|PerPhase|CriticalOrbit, bla: Scalar|Matrix,
  la, sa_seeds, interior_derivative, interior_probe, closed_form_components, de, seek,
  atom_domain, pi_meter, external_rays, orbit_overlay, content }`.

## Affected modules

The ~12 formula-specific modules from [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §1:

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
  1.014, fields bit-identical" ([almondbread benchmarks and verification record](../synthesis/benchmarks.md) M7).
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
  yes, so the CLI can hide features too.) Answered: yes (`core::formula::FormulaCapabilities`).
- One MSL file: answered for now, one file with the hooks in a section; revisit with F7's
  runtime generation.

## As built (2026-10-19, awaiting review)

Architecture in [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](../platform/render-architecture.md) "F2: pluggable formulas"; the A/B proof in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F2: pluggable formulas, Mandelbrot A/B".

- **Choices where the plan left one.** The test-only second formula is `Multibrot(p)` with the
  power generic (`p ≤ 8`), not a p = 2 special case, so F3 mostly opens it rather than writes it;
  the kernels get `kPower` (constant 17) beside `kFormula` (16) and `kConformal` (18) so the
  generic loops unroll. Formula constants are read through `is_function_constant_defined`, and
  Mandelbrot pipelines never set them, which keeps their compile requests, warm-up list and
  shader cache keys exactly as before (no disassembly check needed: the fields and timings are the
  proof). Public APIs keep their names as Mandelbrot and gain `*_for(formula, ...)` variants
  (selection, oracle, BLA, cycles, f64 reference), so Mandelbrot-only callers (journeys, seek,
  target lock, precision tools) are untouched. Only `selectable()` formulas (Mandelbrot) pass the
  FFI; the CLI's `--formula multibrotP` reaches the test path.
- **Capabilities** (core, mirrored over UniFFI): the plan's list plus `julia_panel` (F1's panel
  is `z² + k`). Multibrot lacks LA, closed-form components, seek, π meter, external rays, content
  and the Julia panel; the renderer gates LA, the component test and SA seeds, the FFI keeps the
  π meter, journeys, seek keys and the Julia panel off, Swift hides places, tours, journey
  controls, seek and the orbit toggle by `model.has(\.x)`. Places and tours set their formula when
  applied.
- **Proof.** Bench fields and PNGs byte-identical to main (84/84 plain; 40/42 shaded + period, the
  two others a 1-texel run-to-run nondeterminism present on main); GPU ms geomean 1.004 over 42
  cases; regress and history sessions OK; interaction test not slower; selftest OK in both apps;
  rust-check, build, app, ios-sim, ios-device-check, swift-test green; core-check's 5 failures are
  main's too. Multibrot(2) on the GPU matches Mandelbrot (bit-identical at four locations, chaos
  elsewhere), z³ + c matches its oracle.
- **For F3.** Make `Multibrot(3..8)` selectable (`formula::selectable`) with a UI picker
  (`Explorer::formulas`); Multibrot oracle fixtures in `almondbread-math/tests/fixtures`; seek
  (Newton framing: `find_nucleus_for` works, `minibrot_size` and `seek.rs` are Mandelbrot);
  a size estimate and an exact Newton finish for powers > 2; the DE renormalisation interval
  (the kernel renormalises every 8 steps, which `|f'| = p|z|^(p-1)` may outgrow near escape) and
  the BLA radius validated at high p; places/tours/journeys per power or hidden; the orbit
  overlay's probe key does not include the formula (a formula switch at a shallow view keeps the
  old overlay until the pointer moves); `floatfloat.metal` (M7 precision tool) and the π meter
  stay Mandelbrot. Its generic path is 1.3–7× slower than Mandelbrot where LA or the closed-form
  test do the work.

## Effort breakdown

M overall (1–2 weeks).

- Kernel factoring and pipeline keys: 3–4 days.
- Maths and core hooks, gates, settings: 2–3 days.
- Capabilities over FFI, Swift hiding: 1 day.
- A/B proof runs and fixes: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
