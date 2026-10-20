---
title: "π in the Mandelbrot set (neck and cusp escape counts)"
type: technique
status: draft
sources: ["[[sources/boll-pi-post]]", "[[sources/klebanoff-pi-in-mandelbrot]]", "[[sources/brockmoeller-pi-everywhere]]", "[[sources/edgar-pi-and-mandelbrot]]", "[[sources/muency-pi]]"]
tags: [pi, parabolic, escape-time, maths-moment, overlay, precision]
---

# π in the Mandelbrot set

A "maths moment" the app shows with the π meter ([Controls: keyboard, pointer and touch](../decisions/controls.md) "π meter"): near two
pinch points of the set the escape count, times the distance, tends to π.

## The phenomenon

Escape count: `z_0 = 0`, `z_{n+1} = z_n² + c`, N = the first n ≥ 1 with `|z_n| > 2`
(src: [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md) eq. 3; the convention of Boll's tables).

- **Neck** `c = -3/4 + iε` (where the period-2 disc touches the cardioid): `N·ε → π`
  (src: [Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](../sources/boll-pi-post.md); first proof [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) Thm. 1,
  2025).
- **Cusp** `c = 1/4 + ε`, ε > 0: `N·√ε → π` (src: [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md)
  main theorem, the first proof, 2001).
- Also `c = -5/4 - ε² + iε`: `N·ε → π/2` (src: [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) §1;
  David Petry in [Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)](../sources/edgar-pi-and-mandelbrot.md) saw near-(half-)integers of `Nε/π` at
  `-5/4 + iε`). Not in the app.
- General form: at every parabolic parameter `c_0` (a root of a hyperbolic component), along a
  parameter ray, `N·|c - c_0|·τ(c_0) → π` (satellite roots, like -3/4) or
  `N·|c - c_0|^{1/2}·τ(c_0) → π` (primitive roots, like 1/4), `τ` from the multiplier map's
  derivative; τ = 1 at -3/4 and 1/4 (src: [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) Thm. 1).
  The vertical line at -3/4 is not a parameter ray, but points close enough to the ray keep
  the limit (Thm. 2).

**Why π (cusp, heuristic).** With `x = 1/2 + y` the real map becomes `y' = y + y² + ε`. Near
y = 0 the steps are tiny, so `dy/dn ≈ y² + ε`, solved by `y = √ε tan(√ε n + C)`; the orbit
enters near one pole of tan and leaves near the next, so `√ε N ≈ π`
(src: [Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)](../sources/edgar-pi-and-mandelbrot.md), Petry 1992; made rigorous in
[Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md) §2). **Neck:** the fixed point -1/2 of `z² - 3/4` has
multiplier -1, so the second iterate is parabolic; Edgar's integral gives π/(2ε) steps of
f∘f, i.e. π/ε steps (src: [Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)](../sources/edgar-pi-and-mandelbrot.md)). Conceptually: the critical
orbit passes through the "gate" between the two fixed points the bifurcation split off, and
it takes `2π / arg μ` steps to turn once around, μ the perturbed multiplier
(src: [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) §1).

**Error order.** The proofs give `N = π/ε + O(1)` at the neck and `N = π/√ε + O(1)` at the
cusp, and the escape radius only shifts N by a constant (src:
[Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) eq. 3 and §1). So `|N·ε − π| = O(ε)` and
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
  VandeWettering's 2011 program hit the same at ε = 1e-9, [Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](../sources/boll-pi-post.md)) and drifts
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

## The orbit

What the counted orbit looks like (ours: plotted with
`cargo run --release -p almondbread-core --example pi_trail -- outdir`, 2026-10-15, M1 Pro,
release; derivation ours, after the heuristics above).

- **Full z-plane: nothing to see.** At the scale of the set the orbit is a dot at the
  parabolic point with a cross through it: it comes in along one axis, spends almost all of
  its N steps within a few √ε of −1/2 (neck) or 1/2 (cusp), and leaves along another.
- **Neck, zoomed to the gate (±4√ε about −1/2): a universal shape.** The even and odd iterates
  lie on opposite arms (the multiplier is −1): one arm comes in along the real axis from the
  left and turns up, the other comes in from the right and turns down. They pass the gate
  between the fixed point `−1/2 + iε/2` and the 2-cycle `−1/2 ± √ε·e^(−iπ/4)` (the 2-cycle of
  `z² + c` solves `z² + z + c + 1 = 0`). Rescaled by √ε the picture is the same at 1e-3,
  1e-5 and 1e-7; only the number of points grows.
- **Cusp: the orbit is real.** In the z-plane it is a segment through the gate between the
  complex fixed points `1/2 ± i√ε`. Plotted against n, `(z_n − 1/2)/√ε` over `√ε·n` is the
  tangent curve `tan(√ε n − π/2)` from −∞ to +∞, so the escape sits at `√ε·n ≈ π`: the
  heuristic made visible, again the same curve at every ε.
- **Both: one turn.** Two steps of the neck map are `w'' = w − 2w³ − 2iεw + …`
  (w = z + 1/2). With `u = 1/w²` this is `du/dk ≈ 4 + 4iεu` per pair of steps, so u moves on
  a circle: `1 + iε/w²` lies on the unit circle and turns by 2ε per step, from 1 (start) back
  to 1 (escape), one turn in `2π/2ε = π/ε` steps. At the cusp `−(√ε + iw)²/(ε + w²)` does the
  same at 2√ε per step (it is `e^(iθ)` with `θ = 2·arctan(w/√ε) + π`). Checked numerically
  (unit test `trail_turns_once_and_does_not_change_the_count`): the angle unwraps to one turn
  within 5 %, and the point stays within 5 % of the unit circle away from the two ends.
- **Where the time goes.** The turn is uniform in n, so the head of the orbit moves at a
  constant pace through the gate, whatever ε: the crawl is visible only when the count is
  long (a few seconds at ε = 1e-8 neck, 1e-16 cusp).

The meter draws exactly this (below): the orbit plot (the neck's gate in the z-plane, the
cusp's tangent curve) and the turn circle, as density accumulated on the counting thread.
**Cost** (same run): drawing every k-th iterate (k = N / 4e6, odd at the neck so both arms
stay) costs 20 % at the neck (one division per sample for the turn) and 5 % at the cusp at
N = 3e8 (5.7 → 6.8 s and 3.8 → 4.0 s; the machine was shared with other builds); tone-mapping
and blurring a 604 x 300 picture takes 5 ms per publish, every 0.12 s. Counts are identical
with and without the trail.

**Frame cost** (measured 2026-10-15, M1 Pro, release, interaction test in a background
1280 x 820 pt window at 2x, machine shared with other agents): the "pi hunt" scenario (the
flight from home with the meter and trail on) draws in 0.34–0.38 ms CPU at p50 and
0.54–0.61 ms at p95 against 0.13–0.14 / 0.18–0.21 ms for "home zoom" in the same runs, so the
meter costs about 0.25 ms (p50) and 0.4 ms (p95) a frame. Two fixes got it there: the overlay
binning used a vector per 16 x 16 tile, which cost 0.68 ms a frame once the inset's large
rectangles touched thousands of tiles (now two passes, count then fill: 0.054 ms for the
same 453 primitives); and the trail picture (820 x 400 BGRA8 at 2x) goes up as a shared buffer
(0.12 ms) instead of a shared texture (0.26 ms, almost all of it the copy into the tiled
layout; allocation 0.008 ms). Unit test `trail_upload_is_cheap`.

## Rendering near the points

The image is left alone (the meter does not raise the iteration limit). Measured 2026-10-15,
M1 Pro, 1600x1000: the cusp view at ε = 1e-8 with the limit at 130 000 (4 × π/√ε) took 8 s of
GPU per frame and showed a nearly uniform dark wedge; the neck view at ε = 1e-6 with
1.3e7 iterations was killed after 10 minutes. Interior pixels next to a parabolic point have
multipliers close to 1 in modulus, so interior detection needs about as many iterations as the
exterior escape, and every pixel runs to the limit. The exterior at the neck is also a sliver:
the two discs touch, so the gap at height ε is O(ε²) wide, far below a pixel. So a view
zoomed into the point is honestly black; the guided flight no longer goes there (below). A
magnifier showing the probe's surroundings at a scale of ε was considered and not built: its
pixels would need about π/ε iterations each (the measurements above), and it would show black.
At the places' depths (ε = 0.02 neck, 2e-4 cusp) the automatic limit shows the wedges and
their dwell bands.

## In the app

Places "π at the Neck" (ε = 0.02, N = 158) and "π at the Cusp" (ε = 2e-4, N = 220).

**The π hunt (guided flight, reworked 2026-10-15 after "the π hunting is only on black
regions, it's not really exciting").** The first version zoomed the view about the parabolic
point down to ε = 1e-8 (neck) or 1e-16 (cusp), and the image there is black (above). Now:

1. The navigator's fly-to eases the view to the valley the pinch point opens into: seahorse
   valley above the neck (short side 0.45, the point 0.29 of it below the centre), elephant
   valley right of the cusp (0.15, the point 0.3 of it left of the centre). Chosen by looking
   at renders of six candidates each (`PiPoint::scenic_view`).
2. A glowing **probe** slides down the line from ε = 0.1 to the end on a **log ruler** drawn
   along the line (one tick per decade, two at the cusp; it is labelled "ε, log scale" and the
   probe's true position is written next to it). ε falls at the zoom speed counted in e-folds
   of the expected N (so both points take about 10 s at 1.6 e-folds per second; Q / E change
   it), eased over the last e-fold. The view zooms gently about the pinch point meanwhile
   (×1.4 at the neck, whose valley narrows to a wedge; ×2 at the cusp).
3. The meter counts the probe as it goes (each finished count is a plot point, digits fill)
   and draws its orbit (above, "The orbit"). The flight holds until the count at the end is
   done (about 4–7 s at N = 3e8) and 2.5 s more.

**Start hitch (fixed 2026-10-16).** The hunt's first frame took 127–146 ms (interaction test,
"pi hunt", 3 of 3 runs with a cold Metal shader cache; 1–2 ms with a warm one, so it showed
after every app update): the place's look turns on DE (shading, outline) and the app switches
the interior colouring to bubbles, and the frame compiled that iterate variant. The renderer
now warms the variants on threads at start ([Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md) "Pipeline warm-up"):
0.7–0.9 ms (3 of 3 cold runs).

The probe stays after the flight while its pinch point is in view; it wins over a view
centred on the line (the valley views are). The trail inset sits above the meter's panel
(434 x ~280 points; left of it when the view is too low, hidden when neither fits), clear of
the maps and the cosmic scale. Seen in `review/pi-neck.mp4`, `review/pi-cusp.mp4` (the `fly`
tool with `--pi-flight neck|cusp`) and headless stills (`almondbread render out.png
--pi-probe neck,1e-5`). Controls: [Controls: keyboard, pointer and touch](../decisions/controls.md) "π meter". Code:
`crates/almondbread-core/src/pi_moment.rs`, `crates/almondbread-render/src/pi_meter.rs`.

## Contradictions

- Boll's table as quoted by Klebanoff gives N = 31415928 at ε = 1e-7 (src:
  [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md) p. 394); the 256-bit reference, double-double, the
  compensated count and plain f64 all give 31415927. The other neck and cusp entries agree.
- Mu-Ency's neck table is one higher throughout (34, 316, 3144, 31418, 314161 for ε = 1e-1 ...
  1e-5; src: [Mu-Ency (Robert Munafo): pi and R2.C(1/2)](../sources/muency-pi.md)): another counting convention. See
  [Why do published π escape counts differ by one (Mu-Ency) or disagree at ε = 1e-7 (Boll)?](../questions/pi-escape-count-conventions.md).

# Citations

- [Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](../sources/boll-pi-post.md)
- [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md)
- [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md)
- [Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)](../sources/edgar-pi-and-mandelbrot.md)
- [Mu-Ency (Robert Munafo): pi and R2.C(1/2)](../sources/muency-pi.md)
