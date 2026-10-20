---
title: "F7: Hybrid formulas via an opcode model"
type: plan
milestone: F7
status: planned
priority: 7
effort: XL
appeal: high
audience: [deep-zoom-creators, enthusiasts]
depth: "unlimited"
realtime-ipad: likely
reuse: "medium: F2's hooks, F5's 2×2 path and F6's diffabs as opcodes; new codegen and per-phase references"
depends-on: ["plans/f2-pluggable-formulas", "plans/f6-burning-ship"]
risks:
  - "runtime MSL generation on iOS may meet App Store review questions (unverified)"
  - "one reference and one BLA table per loop phase multiplies memory and reference time"
  - "even KF's rebasing had bugs with hybrids"
  - "low appeal for casual users"
sources: ["synthesis/other-fractal-types", "formulas/hybrids", "sources/mathr-deep-zoom-theory", "sources/mathr-deep-zoom-theory-again", "sources/kf-manual", "implementations/fraktaler-3"]
tags: [formulas, hybrids, opcodes, codegen]
created: 2026-10-19
updated: 2026-10-19
---

# F7: Hybrid formulas via an opcode model

Status: planned, after [F6: Burning Ship family](f6-burning-ship.md). This is the old roadmap M8 ("more formulas via
an opcode model", roadmap step 11 in [Deep zoom Mandelbrot rendering techniques](../synthesis/deep-zoom-rendering-techniques.md)).

## Goal

Let a formula be a short program of opcodes (square, power, abs, neg, conj, add c) repeated in
a loop of phases, as Kalles Fraktaler's stanzas and Fraktaler 3's opcode lines do
(src: [Hybrid formulas](../formulas/hybrids.md), [Fraktaler 3](../implementations/fraktaler-3.md)). This opens the whole space the
KF/F3 community explores, at almondbread's speed.

## Scope

- An opcode list per phase, a loop of phases, validated in core.
- Per-operator perturbation (each opcode's delta rule composed), one reference and one BLA table
  per loop phase (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Hybrid Fractals;
  [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing).
- Code generation: MSL source per formula, compiled at runtime with `newLibraryWithSource`, as
  KF and F3 do with OpenCL (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Code Generation).
- An editor in the control center (opcode rows), a few presets, save/share as text.

## Non-goals

- Transcendental opcodes (sin, exp): need BigFloat trig we do not have
  ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- Non-integer powers.
- A node-graph editor.

## Design sketch

- Core: `HybridFormula { phases: Vec<Vec<Op>> }`; the reference orbit runs the phases in
  BigFloat; rebasing needs references at each phase of the loop ("even KF's rebasing had bugs
  with hybrids", src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Codegen: emit `fStep`/`fStepX`/`fDeriv`/`fJac` bodies per phase from opcode templates into the
  F2 kernel skeleton; cache compiled libraries by a hash of the opcode list.
- BLA: per-phase 2×2 tables (scalar when every opcode is conformal).
- Capabilities computed from the opcodes (conformal or not, seeds allowed or not).

## Affected modules

- New `almondbread-core/src/hybrid.rs` (model, validation, codegen), `almondbread-math/src/orbit.rs`
  (phase loop), `selection.rs` (Newton per degree).
- `almondbread-core`: `reference.rs` (per-phase references), `bla.rs` (per-phase tables),
  `oracle.rs` (interpreted opcodes as truth), `orbit.rs`.
- `almondbread-render`: `metal/mod.rs` (runtime library compile, cache), `mandelbrot.metal`
  skeleton with insertion points, `scheduler.rs` (references × phases).
- FFI: opcode list in settings; Swift: editor UI.

## Verification

- Every opcode's delta rule checked against the interpreted oracle at random points.
- Hybrids that equal known formulas (e.g. one `sqr, add c` phase = Mandelbrot; `abs, sqr,
  add c` = Burning Ship) must give fields bit-identical to the built-in formulas.
- `verify` vs oracle for 5 presets at 1e-50 and 1e-300; `verify --history`.
- Mandelbrot fast path: bench A/B ≈ 1.0 and bit-identical fields (built-in formulas never go
  through codegen).
- Cold compile time of a generated library on the Mac and the iPad.

## Review gate

Before codegen work: the user (and, if wanted, someone from the KF/F3 community) agrees on the
opcode set and the editor sketch. After: review of presets in a real window.

## Risks

- App Store review and runtime MSL compilation: compiling MSL source is permitted on iOS, but
  review acceptance is unverified ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §6).
- References × phases: memory and reference time on the iPad.
- Rebasing per phase is subtle (KF's bugs).
- Small audience; may stay a Mac-first, enthusiast feature.

## Open questions

- Opcode set: match F3's (abs, neg, power) for interchange?
- Is a text format compatible with KF or F3 files worth it (licences: format facts are free,
  code is not)?

## Effort breakdown

XL overall (the research gives L–XL; months with the editor and content).

- Model, validation, interpreted oracle: 1 week.
- Codegen and runtime compile, caching: 1–2 weeks.
- Per-phase references and BLA tables: 2 weeks.
- Editor UI and presets: 1–2 weeks.
- Verification: 1 week.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Hybrid formulas](../formulas/hybrids.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Fraktaler 3](../implementations/fraktaler-3.md)
