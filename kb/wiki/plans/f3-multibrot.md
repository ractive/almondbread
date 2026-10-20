---
title: "F3: Multibrot z^p + c"
type: plan
milestone: F3
status: planned
priority: 3
effort: M
appeal: medium
audience: [casual, artists, enthusiasts]
depth: "unlimited"
realtime-ipad: likely
reuse: "high: every per-formula hook with scalar complex arithmetic; no LA, no closed-form components, no π meter"
depends-on: ["plans/f2-pluggable-formulas"]
risks:
  - "float32 iteration-band glitches at high powers (F3 warns of them in plain float)"
  - "the shapes get repetitive; low novelty at depth"
  - "reference cost grows with p on the CPU"
sources: ["synthesis/other-fractal-types", "formulas/multibrot", "sources/kf-page", "sources/fraktaler-3-site", "techniques/bla"]
tags: [formulas, multibrot, perturbation, bla]
created: 2026-10-19
updated: 2026-10-19
---

# F3: Multibrot z^p + c

Status: planned, after [F2: Pluggable-formula refactor](f2-pluggable-formulas.md).

## Goal

The first new formula, chosen because it exercises every per-formula hook (reference, BLA,
oracle, derivative, Newton) with scalar complex arithmetic only. It is "the best test that the
refactor kept the Mandelbrot fast path intact" (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5).
Users get p-fold symmetric sets, `p = 3..8`, at any depth.

## Scope

- Formula `Multibrot { p }`, `p = 3..8` (integer only).
- Reference orbit in BigFloat; perturbed step from the binomial expansion of `(Z + z)^p − Z^p`;
  BLA with `A = pZ^{p−1}`, `B = 1` and its radius ([Multibrot (integer power p)](../formulas/multibrot.md), [Bivariate linear approximation (BLA)](../techniques/bla.md)).
- Rebasing as for Mandelbrot (one critical point at 0).
- DE and the interior derivative test with `pz^{p−1}`; Newton nucleus and period detection
  (seek minibrot) with the same derivative (inference in [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- CPU oracle, f64 reference, orbit overlay for the formula.
- A small places set (10–20 views per power, found with seek), formula picker in the control
  center, capability-driven hiding of LA, π, closed-form components, external rays.

## Non-goals

- Non-integer powers: they "do not reduce at all" for perturbation
  (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a "Transcendental").
- LA (power 2 only, [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)), local SA seeds for p > 2
  unless measured to pay.
- Tours and journeys tuned for Multibrot (journeys may run, untuned).

## Design sketch

- `fStep` for formula Multibrot: `z' = Σ_{j=1..p} C(p,j) Z^{p−j} z^j + δc`, evaluated in Horner
  form; `fStepX` the same in floatexp.
- `parallel.rs`: the three-product split serves only squaring; for p > 2 the reference runs on
  one thread, or squares are chained (p = 4, 8 as repeated squaring) (inference).
- BLA table built from `pZ^{p−1}`; skip lengths shorter than Mandelbrot's because the
  validity radius scales with `|Z|^{p−1}` (inference, to be measured).
- Iteration-band guard: compare float32 against the oracle at p = 8 deep; if bands show, use
  floatexp steps near small `|Z|` (F3 warns plain float "gives iteration-band glitches at high
  powers", src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## Affected modules

- `almondbread-math`: `orbit.rs` (Multibrot step), `parallel.rs` (dispatch), `selection.rs`
  (derivative for Newton and period).
- `almondbread-core`: `reference.rs`, `oracle.rs`, `orbit.rs`, `bla.rs`, `cycles.rs` and the
  interior probe, `seek.rs`, `places_data.rs` (tagged places), `settings.rs`.
- `almondbread-render`: `mandelbrot.metal` step functions for `kFormula = Multibrot`,
  `metal/mod.rs` pipeline keys.
- `almondbread-ffi`: formula setting, capabilities. Swift: formula picker, hiding.
- `tests/fixtures` + `tests/oracle/generate_fixtures.py`: Multibrot reference fixtures.

## Verification

- Oracle fixtures per power (BigFloat reference orbits vs the Python generator).
- `verify` metrics vs the CPU oracle at 5 views per power (shallow, 1e-20, 1e-100, 1e-300,
  1e-1000), class mismatch within the Mandelbrot tolerance; `precision` at p = 8.
- `verify --history zoom|pan` at two depths, 0 % mismatch vs fresh.
- Mandelbrot fast path: `make bench` A/B geomean ≈ 1.0 and bit-identical fields vs main, as in
  F2 (the new formula must not change formula 0's pipelines).
- `make interaction-test` with a Multibrot view (new scenario), `make selftest`, headless
  stills looked at.

## Review gate

The user explores p = 3..8 on the Mac and the iPad (picker, seek, places). Decide whether
Multibrot ships visible or stays as a hidden test formula, and whether to continue with F4.

## Risks

- Band glitches in float32 at high p (above).
- Repetitive shapes: appeal medium (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- Seek and the autopilot target lock tuned on Mandelbrot; may need per-formula thresholds.

## Open questions

- Which powers to offer (all of 3..8, or 3, 4, 5, 8)?
- Does BLA still pay at p = 8, or should the table be off for high powers?

## Effort breakdown

M overall (1–2 weeks).

- Maths, reference, oracle, fixtures: 2–3 days.
- Kernel step functions, BLA: 2–3 days.
- Places, UI picker, capabilities: 1–2 days.
- Verification and review fixes: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Multibrot (integer power p)](../formulas/multibrot.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
