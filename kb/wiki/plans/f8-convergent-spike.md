---
title: "F8: Nova, Newton and Magnet research spike"
type: plan
milestone: F8
status: planned
priority: 8
effort: M
appeal: medium
audience: [artists, enthusiasts]
depth: "unknown (brute force needed 192-bit at 4.3e-21 for Nova)"
realtime-ipad: unknown
reuse: "low–medium: viewport, scheduler, colour; perturbation and BLA criteria unknown for convergent formulas"
depends-on: ["[[plans/f2-pluggable-formulas]]"]
risks:
  - "no published perturbation or BLA scheme for convergence to a root"
  - "rational maps need the quotient identity in every step site"
  - "the spike may end in 'not feasible deep'"
sources: ["[[synthesis/other-fractal-types]]", "[[formulas/others-feasibility]]", "[[questions/perturbation-convergent-formulas]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/hpdz-animations]]", "[[sources/wikipedia-newton-fractal]]"]
tags: [formulas, nova, newton, magnet, convergent, research]
created: 2026-10-19
updated: 2026-10-19
---

# F8: Nova, Newton and Magnet research spike

Status: planned, a time-boxed spike after the formula refactor; no feature is committed.

## Goal

Answer [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md): can convergent and rational formulas
(Nova, Newton, Magnet) be rendered deep with perturbation, rebasing and some form of BLA? They
have a distinct, smooth look and "mini-novas" in parameter space
(src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).

## Scope

- Literature and forum pass: Fractal Zoomer reportedly uses perturbation with rebasing for nova,
  magnet, lambda and Newton parameter space (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
  §Reply #8).
- A CPU prototype (Rust example, f64 and BigFloat reference) of perturbed Nova with rebasing,
  checked against direct BigFloat at 1e-20, 1e-50, 1e-100.
- A BLA validity criterion for convergence to a root, derived and tested on the prototype.
- A written verdict with effort for a real milestone.

## Non-goals

- GPU kernels, UI, places.
- Newton basins of polynomials as a shallow look (could be a separate small item; Newton basins
  are Julia sets of the Newton map, src: [Wikipedia: Newton fractal](../sources/wikipedia-newton-fractal.md)).

## Design sketch

- Nova: `z ← z − R (z³ − 1)/(3z²) + c`. The perturbed quotient uses
  `a/b − A/B = (aB − Ab)/(bB)` with deltas expanded (quotient identity; inference).
- Stop rule: convergence `|z_{n+1} − z_n| < ε` instead of escape; perturbed form of that test.
- Rebasing: needs a reference zero; check whether the critical points of the Nova map serve.

## Affected modules

None in the app during the spike. New `crates/almondbread-core/examples/nova_check.rs` (prototype),
fixtures under `tests/fixtures` if it continues.

## Verification

- Prototype vs direct BigFloat iteration counts and root indices at three depths, at least
  99.9 % agreement, with the error causes listed.
- The verdict page states measured numbers with date, machine and build.

## Review gate

The user reads the verdict and decides: drop, a shallow-only look, or a full milestone.

## Risks

- No published scheme ([Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)); the spike may not converge on one.
- Brute-force Nova at 4.3e-21 already needed 192-bit arithmetic (src: [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)),
  so shallow float32 alone will not go far.

## Open questions

- Which convergent formula first: Nova (parameter space) or Magnet?
- Is convergence-time colouring compatible with our smooth colouring and looks?

## Effort breakdown

M as a time box (one week; the scale's M is 1–2 weeks). A full milestone after a positive
verdict would be L–XL.

- Literature and derivation: 2 days.
- CPU prototype and checks: 2 days.
- Verdict and kb pages: 1 day.
