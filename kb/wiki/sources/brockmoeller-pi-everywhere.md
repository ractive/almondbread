---
title: "Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)"
type: source
authors: ["Thies Brockmoeller", "Oscar Scherz", "Nedim Srkalovic"]
year: 2025
raw: "papers/pi-in-mandelbrot-everywhere-2025.pdf"
status: digested
tags: [pi, parabolic, neck, cusp, escape-time, proof, holomorphic-dynamics]
---

# Pi in the Mandelbrot set everywhere

**Citation:** T. Brockmoeller, O. Scherz, N. Srkalovic, "Pi in the Mandelbrot set
everywhere", arXiv:2505.07138v1 [math.DS] (May 2025) · saved
`raw/papers/pi-in-mandelbrot-everywhere-2025.pdf` (+ `.txt`), retrieved 2026-10-15 ·
**Terms:** arXiv licence; personal research copy.

## What it is

The first proof of the neck case (c = -3/4) and of c = -5/4, and a generalisation to every
parabolic parameter of the Mandelbrot set, by parabolic bifurcation theory (perturbed flower
theorem, Shishikura). Cites Siewert (2023) for a sharper proof at c = 1/4.

## Useful parts

- **Observation** (§1): with escape radius R ≥ 2, `c = -3/4 + it`: `N·|t| → π`;
  `c = 1/4 + t`: `N·|t|^{1/2} → π`; `c = -5/4 - t² + it`: `N·|t| → π/2`. With t = 10^-n the
  digits of N are the first n digits of π (neck) or n/2 digits (cusp, n even).
- **Theorem 1**: at a parabolic parameter c_0 of period n with a q-th root of unity multiplier,
  for c_k → c_0 along a parameter ray landing there, `N·|c_k - c_0|·τ(c_0) → π` (satellite) or
  `N·|c_k - c_0|^{1/2}·τ(c_0) → π` (primitive), `τ = |μ'_{qn}(c_0)| / (2qn)`; τ = 1 at -3/4 and
  1/4, 1/2 at -5/4.
- **Error**: the escape time is `π·(...) + O(1)` (eq. 3 in §3), so the product's error is
  O(ε) at satellite roots and O(√ε) at primitive ones; the choice of R changes N by a constant
  (§1). Our measurements agree ([[techniques/pi-in-the-mandelbrot-set]]).
- **Theorem 2**: points near the ray (within a disc not meeting M, radius a·distance, 1 < a ≤ 8)
  keep the limit; whether the vertical line -3/4 + it meets M only at -3/4 is still open.
- **Mechanism**: the critical orbit passes through a "gate" between the two fixed points the
  bifurcation split off; one full turn of 2π around the fixed point at angle arg μ per step
  gives `N ≈ 2π / arg μ`.

## Reliability

A preprint (not refereed when retrieved); consistent with Klebanoff, Edgar's heuristics and our
counts.
