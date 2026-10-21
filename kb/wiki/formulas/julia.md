---
title: "Julia sets"
type: formula
status: draft
sources: ["sources/fractalforums-zhuoran-rebasing-bla", "sources/swift-mandelbrot-readme", "sources/fractalzoomer-readme"]
tags: [julia, formula, perturbation, rebasing]
---

# Julia sets

## Iteration

`z_{n+1} = z_n^2 + k` with fixed `k`; the pixel sets the start `z_0`. Deep
Julia zooms are much rarer than parameter-space zooms; the main use in an
explorer is a companion view of the Julia set at the cursor's `c`.

## Perturbed form (our derivation)

Reference start `Z_0` (high precision), pixel start `Z_0 + z_0`:

`z_{n+1} = 2 Z_n z_n + z_n^2`, with `z_0` = pixel offset and no `+c` term.

So in BLA terms `B = 0` and `z_{m+l} = A_{m,l} z_m`; the c-dependence of the
radius disappears and only `|z| < R` matters (inference). The Mandelbrot
radius derivation does not carry over verbatim because there is no
`max|c|` term; with `c = 0` it gives `r = eps |Z| / (2|Z| + 1)` (inference).

## Rebasing

Zhuoran's rule resets to reference iteration 0 because `Z_0 = 0` there. mathr
states the only requirements are "basic property of perturbation, and an
iteration at which the reference orbit is 0"
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10). A Julia
reference starts at the reference pixel, not at 0, so the plain rule does
not apply unless the orbit passes through (or is rebased against) a point of
value 0, e.g. a reference that starts at the critical point's preimage
(inference). superheal (Fractal Zoomer's author) reported that "it also works
in julia sets" in the same thread, referring to another post that we have not
read (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #11). How he
handles the missing zero is unknown (unverified).

Fallback: Pauldelbrot glitch detection plus extra references
([Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](../techniques/glitch-detection.md)), which needs no zero in the orbit.

**Critical-orbit rebasing (our method, measured).** Two references: the view centre's orbit
(`Z_0 = centre`, `c = k`) for a pixel's first steps, and the critical orbit (the orbit of 0
under `z² + k`, which is exactly the Mandelbrot orbit of `c = k`) as the rebase target. The
pixel starts with `z = pixel − centre` at the centre orbit's index 0; when `|Z_m + z| < |z|` or
the centre orbit ends it rebases to the critical orbit's index 0 (`z ← Z_m + z`, exact because
that orbit's point is 0) and stays there, rebasing to its index 0 like a Mandelbrot pixel. In
float32 and floatexp this matches a direct high-precision oracle at five `k` (cardioid, period-3
bulb, Misiurewicz `i`, a Siegel disc, outside the set) at 1e-20 … 1e-1000 as closely as the
oracle matches itself under a 0.01-texel shift; without the `|Z + z| < |z|` test the pictures
break where pixel orbits pass near 0 (measured 2026-10-21, M1 Pro;
[F4: Deep Julia sets](../plans/f4-deep-julia.md) "Step 1 results"). BLA with `B = 0` works on both
orbits (same table, built on their concatenation).

**`k` must be exact.** At depth the set moves with `k` by about the change in `k`, so `k` is a
fixed binary value carried at its own precision; a decimal `k` names the binary value its
digits imply (measured: the same 16-digit decimal at 128 and 400 bits gives different pictures at
1e-100; same page).

## Support

Fractal Zoomer lists Julia sets and a Julia map
(src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)); swift-mandelbrot has a Julia
companion view (src: [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)).

## Verdict for almondbread

Shallow Julia companion view (direct float32 or double-float) is cheap and
useful ([F1: Julia view (companion inset)](../plans/f1-julia-view.md), built). Deep Julia zoom: built in F4 with critical-orbit
rebasing (above), to the same depths as Mandelbrot views; interior by the derivative test
(every interior pixel falls into `k`'s attracting cycle); Siegel discs stay unresolved (no
attracting cycle to converge to).

# Citations

- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [swift-mandelbrot README (drquicksilver)](../sources/swift-mandelbrot-readme.md)
- [Fractal Zoomer README](../sources/fractalzoomer-readme.md)
- [F4: Deep Julia sets](../plans/f4-deep-julia.md) (our measurements)
