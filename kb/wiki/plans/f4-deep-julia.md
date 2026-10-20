---
title: "F4: Deep Julia sets"
type: plan
milestone: F4
status: planned
priority: 4
effort: M
appeal: medium
audience: [deep-zoom-creators, artists, education]
depth: "unlimited if critical-orbit rebasing holds in float32 (unverified)"
realtime-ipad: likely
reuse: "high: the critical orbit of k is the Mandelbrot reference orbit of k we already compute"
depends-on: ["[[plans/f2-pluggable-formulas]]", "[[plans/f1-julia-view]]"]
risks:
  - "critical-orbit rebasing for Julia is untested in float32"
  - "deep Julia zooms repeat themselves (less variety than Mandelbrot dives)"
  - "no seek: there are no minibrots in a Julia set"
sources: ["[[synthesis/other-fractal-types]]", "[[formulas/julia]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[techniques/rebasing]]", "[[techniques/bla]]"]
tags: [formulas, julia, perturbation, rebasing]
created: 2026-10-19
updated: 2026-10-19
---

# F4: Deep Julia sets

Status: planned, after [F3: Multibrot z^p + c](f3-multibrot.md).

## Goal

Open the Julia set of any `k` as the main view and zoom it as deep as Mandelbrot views. The
F1 inset becomes the entry point ("open this Julia set"). No Apple-native app offers deep Julia
zooms at interactive speed; Frax stops at about 1e14 (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §3).

## Scope

- Formula `Julia { k }` with `k` in high precision (copied from the point under the pointer,
  a place, or the view centre).
- Perturbed step `z' = 2Zz + z²` (no `+δc`), BLA with `B = 0` ([Julia sets](../formulas/julia.md)).
- Two references: the view-centre orbit for the first steps, then rebasing onto the critical
  orbit (the orbit of 0 under `z² + k`), which is exactly the Mandelbrot reference orbit of `k`
  (our observation; rebasing for Julia needs an orbit through 0, src:
  [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10).
- DE with `dz/dz₀`; interior by the attracting cycle of `k` (period from our existing period
  finder; inference).
- Julia twin: open the Julia set of the current deep view's centre at the same framing.
- Capabilities: no seek, no π, no LA (until measured), no closed-form components.

## Non-goals

- Julia sets of other formulas (Multibrot Julia, Burning Ship Julia) until their Mandelbrot-side
  formula exists.
- A "seek preimage of 0" feature (the nearest analogue of seek; inference). Maybe later.

## Design sketch

- Step 1 is a test, not a feature: at five `k` (cardioid, period-3 bulb, Misiurewicz `i`, a
  Siegel-disc boundary point, outside the set) render deep views against the oracle with
  critical-orbit rebasing. Fractal Zoomer's author reports rebasing "also works in julia sets"
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #11), but we have not seen it in
  float32. If it fails, the fallback is one reference per view at the centre with glitch
  correction by re-referencing (slower, bounded depth).
- The scheduler and reference worker gain a second reference slot (centre + critical); the
  critical orbit is cached per `k`.
- BLA table on the critical orbit with `B = 0` (longer skips than Mandelbrot's, inference).

## Affected modules

- `almondbread-core`: `reference.rs` (two references), `oracle.rs` (Julia truth), `bla.rs`
  (`B = 0`), `cycles.rs` (interior by `k`'s cycle), `orbit.rs` (overlay of `z₀`'s orbit),
  `settings.rs` (`k` in BigComplex), `places_data.rs` (Julia places), `tour.rs`
  (`formula: julia` step key), the Julia tour.
- `almondbread-math`: `orbit.rs` (reference from `z₀ ≠ 0` with fixed `k`).
- `almondbread-render`: `mandelbrot.metal` step functions for `kFormula = Julia`, scheduler
  reference handling (`scheduler.rs`).
- `almondbread-render/src/overlay.rs`: "open as main view" from the F1 panel.
- FFI and Swift: formula switch, `k` readout, Julia twin action.

## Verification

- The rebasing test above, `verify` vs the oracle at 1e-20, 1e-100, 1e-300, 1e-1000 for the
  five `k`; `precision` (artefact or chaos?) where they differ.
- `verify --history zoom|pan|jump` for a Julia view.
- Mandelbrot fast path: bench A/B geomean ≈ 1.0, bit-identical fields, `make regress`.
- `make interaction-test` with a deep Julia scenario; `make selftest`; stills looked at.

## Review gate

Show the rebasing test's verdict to the user first (go or fallback). Then the user zooms Julia
sets on the Mac and the iPad, including the twin. Decide whether journeys should visit Julia
sets.

## Risks

- Rebasing unproven (above): the main risk; the test comes first.
- Repetition at depth: a Julia set is mapped onto itself by `f`, so dives look alike
  (inference, [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- Two references raise reference memory on the iPad (small at default limits,
  [iOS and iPadOS on a device: first run, signing, memory, thermal, background](../platform/ios-device.md)).

## Open questions

- Does critical-orbit rebasing hold in float32? (open question in
  [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md))
- Is the twin framing useful away from Misiurewicz points?

## Effort breakdown

M overall (1–2 weeks).

- Rebasing test and verdict: 2 days.
- Two-reference scheduling, kernel steps, BLA: 3 days.
- Oracle, places, UI, twin: 2 days.
- Verification and review fixes: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Julia sets](../formulas/julia.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
