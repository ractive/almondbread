---
title: "π in the Mandelbrot set (neck and cusp escape counts)"
type: technique
status: draft
sources: ["[[sources/boll-pi-post]]", "[[sources/klebanoff-pi-in-mandelbrot]]", "[[sources/brockmoeller-pi-everywhere]]", "[[sources/edgar-pi-and-mandelbrot]]", "[[sources/muency-pi]]"]
tags: [pi, parabolic, escape-time, maths-moment, overlay, precision]
---

# π in the Mandelbrot set

A "maths moment" the app shows with the π meter ([[decisions/controls]] "π meter"): near two
pinch points of the set the escape count, times the distance, tends to π.

## The phenomenon

Escape count: `z_0 = 0`, `z_{n+1} = z_n² + c`, N = the first n ≥ 1 with `|z_n| > 2`
(src: [[sources/klebanoff-pi-in-mandelbrot]] eq. 3; the convention of Boll's tables).

- **Neck** `c = -3/4 + iε` (where the period-2 disc touches the cardioid): `N·ε → π`
  (src: [[sources/boll-pi-post]]; first proof [[sources/brockmoeller-pi-everywhere]] Thm. 1,
  2025).
- **Cusp** `c = 1/4 + ε`, ε > 0: `N·√ε → π` (src: [[sources/klebanoff-pi-in-mandelbrot]]
  main theorem, the first proof, 2001).
- Also `c = -5/4 - ε² + iε`: `N·ε → π/2` (src: [[sources/brockmoeller-pi-everywhere]] §1;
  David Petry in [[sources/edgar-pi-and-mandelbrot]] saw near-(half-)integers of `Nε/π` at
  `-5/4 + iε`). Not in the app.
- General form: at every parabolic parameter `c_0` (a root of a hyperbolic component), along a
  parameter ray, `N·|c - c_0|·τ(c_0) → π` (satellite roots, like -3/4) or
  `N·|c - c_0|^{1/2}·τ(c_0) → π` (primitive roots, like 1/4), `τ` from the multiplier map's
  derivative; τ = 1 at -3/4 and 1/4 (src: [[sources/brockmoeller-pi-everywhere]] Thm. 1).
  The vertical line at -3/4 is not a parameter ray, but points close enough to the ray keep
  the limit (Thm. 2).

**Why π (cusp, heuristic).** With `x = 1/2 + y` the real map becomes `y' = y + y² + ε`. Near
y = 0 the steps are tiny, so `dy/dn ≈ y² + ε`, solved by `y = √ε tan(√ε n + C)`; the orbit
enters near one pole of tan and leaves near the next, so `√ε N ≈ π`
(src: [[sources/edgar-pi-and-mandelbrot]], Petry 1992; made rigorous in
[[sources/klebanoff-pi-in-mandelbrot]] §2). **Neck:** the fixed point -1/2 of `z² - 3/4` has
multiplier -1, so the second iterate is parabolic; Edgar's integral gives π/(2ε) steps of
f∘f, i.e. π/ε steps (src: [[sources/edgar-pi-and-mandelbrot]]). Conceptually: the critical
orbit passes through the "gate" between the two fixed points the bifurcation split off, and
it takes `2π / arg μ` steps to turn once around, μ the perturbed multiplier
(src: [[sources/brockmoeller-pi-everywhere]] §1).

**Error order.** The proofs give `N = π/ε + O(1)` at the neck and `N = π/√ε + O(1)` at the
cusp, and the escape radius only shifts N by a constant (src:
[[sources/brockmoeller-pi-everywhere]] eq. 3 and §1). So `|N·ε − π| = O(ε)` and
`|N·√ε − π| = O(√ε)`: each decade of ε adds one digit at the neck, every second decade one at
the cusp. Our measurements confirm both (below).

## Measurements (ours)

`cargo run --release -p almondbread-core --example pi_check -- 4e7 4e9 4e9`, measured
2026-10-15, M1 Pro, release, one core. `N − π/ε` (neck) and `N − π/√ε` (cusp) are the O(1)
offsets; "comp" is the meter's count (below), "dd" double-double, "f64" plain f64 in the
shifted coordinates, "naive" textbook `z² + c` in f64, "big" a 256-bit BigFloat orbit with c
exact (`-` = did not escape within 1.5 × π/ε iterations).

| neck ε | N | N·ε | N − π/ε | f64 | naive | big |
| --- | --- | --- | --- | --- | --- | --- |
| 1e-1 | 33 | 3.3 | 1.58 | = | = | = |
| 1e-2 | 315 | 3.15 | 0.84 | = | = | = |
| 1e-3 | 3143 | 3.143 | 1.41 | = | = | = |
| 1e-4 | 31417 | 3.1417 | 1.07 | = | = | = |
| 1e-5 | 314160 | 3.1416 | 0.74 | = | = | = |
| 1e-6 | 3141593 | 3.141593 | 0.35 | = | = | = |
| 1e-7 | 31415927 | 3.1415927 | 0.46 | = | = | = (14.8 s) |
| 1e-8 | 314159266 | 3.14159266 | 0.64 | = | - | |
| 1e-9 | 3141592655 | 3.141592655 | 1.41 | = | - | |

| cusp ε | N | N·√ε | N − π/√ε | f64 | naive | big |
| --- | --- | --- | --- | --- | --- | --- |
| 1e-2 | 30 | 3.0 | -1.42 | = | = | = |
| 1e-4 | 312 | 3.12 | -2.16 | = | = | = |
| 1e-6 | 3140 | 3.14 | -1.59 | = | = | = |
| 1e-8 | 31414 | 3.1414 | -1.93 | = | = | = |
| 1e-10 | 314157 | 3.14157 | -2.27 | = | = | = |
| 1e-12 | 3141591 | 3.141591 | -1.65 | = | 3141625 | = |
| 1e-13 | 9934586 | 3.14159194 | -2.27 | = | 9935818 | = |
| 1e-14 | 31415925 | 3.1415925 | -1.54 | = | 31430913 | = (12.4 s) |
| 1e-15 | 99345881 | 3.14159260 | -1.66 | = | 99367235 | |
| 1e-16 | 314159263 | 3.14159263 | -2.36 | +1 | 302015641 | |
| 1e-17 | 993458825 | 3.14159265 | -1.58 | +2 | - | |
| 1e-18 | 3141592652 | 3.141592652 | -1.59 | -13 | - | |

(Odd decades of the cusp and 3.7e-6 / 2.2e-8 at the neck, 3.7e-11 at the cusp, behave the
same; full output in the example.) Findings:

- **Error order confirmed.** The neck offset stays between 0.35 and 1.6 over nine decades and
  the cusp offset between -1.4 and -2.5 over eighteen: O(ε) and O(√ε). At the cusp the offset
  is systematically about -2 (N falls short), at the neck about +1. Because N is an integer,
  part of the error is quantisation of the same order.
- **Precision.** Textbook f64 fails at the neck from ε = 1e-8 (the orbit never escapes: the
  rounding of `c = -0.75 + iε` and of each step piles up over the slow passage; Mark
  VandeWettering's 2011 program hit the same at ε = 1e-9, [[sources/boll-pi-post]]) and drifts
  at the cusp from ε = 1e-12 (`0.25 + ε` is not even representable below about 3e-17). Plain
  f64 in coordinates centred on the parabolic fixed point (`z = -1/2 + w` at the neck,
  `z = 1/2 + w` at the cusp, so ε is never rounded into c) is exact at the neck down to 1e-9
  and off by 1, 2 and 13 iterations at the cusp at 1e-16 ... 1e-18. Double-double agrees with
  the 256-bit reference everywhere tested.
- **Costs per iteration** (one core): f64 shifted 3.3–3.8 ns, compensated 11–13 ns,
  double-double 28–29 ns, BigFloat 256 bits 0.4–0.5 µs.

## The meter's count

`almondbread_core::pi_moment::escape_count`: the shifted map with a *compensated* update.
Each step adds a small increment to w (`w' = w + (w² + ε)` at the cusp; at the neck
`w' = -w + (a² − b²) + i(2ab + ε)`, the sign flip is exact), and the rounding error of that
one addition is what accumulates over the π/ε steps of the slow passage. TwoSum recovers it
exactly and carries it in a second f64 (`w = w_hi + w_lo`); the increment itself only needs
relative precision. It matched double-double on every ε above (neck to 1e-9, cusp to 1e-18) at
less than half the cost. Pseudocode (cusp):

```text
wh, wl = -1/2, 0
for n = 1, 2, ...
    inc = (wh² + ε) + 2·wh·wl
    s, e = two_sum(wh, inc)           # s + e = wh + inc exactly
    wh, wl = quick_two_sum(s, e + wl)
    if |wh + 1/2| > 2: return n
```

**Depth limit:** 10⁹ iterations (about 13 s at the neck, 11 s at the cusp), i.e. ε ≥ 3.1e-9 at
the neck and ε ≥ 9.9e-18 at the cusp. Deeper, the panel says so instead of counting.

## Rendering near the points

The image is left alone (the meter does not raise the iteration limit). Measured 2026-10-15,
M1 Pro, 1600x1000: the cusp view at ε = 1e-8 with the limit at 130 000 (4 × π/√ε) took 8 s of
GPU per frame and showed a nearly uniform dark wedge; the neck view at ε = 1e-6 with
1.3e7 iterations was killed after 10 minutes. Interior pixels next to a parabolic point have
multipliers close to 1 in modulus, so interior detection needs about as many iterations as the
exterior escape, and every pixel runs to the limit. The exterior at the neck is also a sliver:
the two discs touch, so the gap at height ε is O(ε²) wide, far below a pixel. Instead the meter
draws the line of approach from the parabolic point to the centre on the canvas. At the places'
depths (ε = 0.02 neck, 2e-4 cusp) the automatic limit shows the wedges and their dwell bands.

## In the app

Places "π at the Neck" (ε = 0.02, N = 158) and "π at the Cusp" (ε = 2e-4, N = 220); the
guided flight zooms about the parabolic point from the place to ε = 1e-8 (neck, about 10 s at
the default zoom speed of 1.6 e-folds per second) or 1e-16 (cusp, about 18 s): an exponential
zoom ε(t) = ε₀·e^(−rate·t), each frame's view framed like the place, eased over the last
e-fold (unit test `flight_zooms_about_the_point_and_lands`). The fly-to path was tried first
and rejected: between two views on the line it pans most of the way before zooming, so the
centre reached ε ≈ 1e-6 while the view was still wide and the parabolic point sat under the
centre ring (seen in the ui-demo screenshot, 2026-10-15). Controls: [[decisions/controls]] "π meter". Code:
`crates/almondbread-core/src/pi_moment.rs`, `crates/almondbread-render/src/pi_meter.rs`.

## Contradictions

- Boll's table as quoted by Klebanoff gives N = 31415928 at ε = 1e-7 (src:
  [[sources/klebanoff-pi-in-mandelbrot]] p. 394); the 256-bit reference, double-double, the
  compensated count and plain f64 all give 31415927. The other neck and cusp entries agree.
- Mu-Ency's neck table is one higher throughout (34, 316, 3144, 31418, 314161 for ε = 1e-1 ...
  1e-5; src: [[sources/muency-pi]]): another counting convention. See
  [[questions/pi-escape-count-conventions]].
