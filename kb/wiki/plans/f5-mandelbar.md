---
title: "F5: Mandelbar (Tricorn) and the non-conformal path"
type: plan
milestone: F5
status: planned
priority: 5
effort: M
appeal: medium
audience: [enthusiasts, education]
depth: "unlimited"
realtime-ipad: likely
reuse: "high: the three-product reference split carries over; new 2×2 BLA, dual-number derivatives, Jacobian Newton"
depends-on: ["[[plans/f2-pluggable-formulas]]"]
risks:
  - "2×2 derivatives double the derivative state per pixel (registers, speed)"
  - "medium appeal on its own; its value is the path it builds for F6"
sources: ["[[synthesis/other-fractal-types]]", "[[formulas/mandelbar]]", "[[sources/mathr-deep-zoom-page]]", "[[techniques/bla]]", "[[techniques/interior-detection]]"]
tags: [formulas, mandelbar, tricorn, non-conformal, bla]
created: 2026-10-19
updated: 2026-10-19
---

# F5: Mandelbar (Tricorn) and the non-conformal path

Status: planned, after [F4: Deep Julia sets](f4-deep-julia.md).

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

## Effort breakdown

M overall (1–2 weeks).

- 2×2 derivative state, dual numbers, matrix BLA: 4 days.
- Jacobian Newton and period in math: 2 days.
- Oracle, places, UI: 1 day.
- Verification: 2 days.
