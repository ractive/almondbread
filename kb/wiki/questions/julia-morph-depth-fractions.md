---
title: "At what fraction of the log-depth to a minibrot does each doubling appear?"
type: question
status: open
sources: ["[[sources/muency-newton-raphson-zooming]]", "[[sources/dinkydau-evolution-journals]]", "[[sources/fractalforums-pathfinding-revisited]]", "[[sources/kf-manual]]"]
tags: [julia-morphing, embedded-julia-set, places, minibrot]
---

# At what fraction of the log-depth to a minibrot does each doubling appear?

## Question

Zooming from a view at log-depth `n` towards a minibrot at log-depth `m`, at which
fractions `f` of the way (`n + f (m − n)`) does the pattern double, quadruple, and so on,
and what sets the starting point `n`?

## What sources say

- Dinkydau: the view at `(n+m)/2` is a Julia morphing (2-fold) of the start; a doubling
  of a shape is at about 3/4 of the depth of a minibrot outside it
  ([Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals](../sources/dinkydau-evolution-journals.md)).
- wbarry: 2 arms at 3/4, 4 at 7/8, 8 at 15/16
  ([Fractal Forums: Pathfinding in the Mandelbrot set - Revisited (2019)](../sources/fractalforums-pathfinding-revisited.md)).
- KF: relative fold 0.5 doubles, 0.75 quadruples; absolute 0.75 of the minibrot's depth is
  "typically near an embedded Julia set" ([Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Mu-Ency: the 2-fold embedded Julia set is about `s^(3/4)` (with `n = 0`), read by Munafo
  as the 4-fold set ([Mu-Ency: Newton-Raphson zooming (embedded Julia set sizes)](../sources/muency-newton-raphson-zooming.md)).

## Our data

[Places: a catalogue of famous views to visit](../synthesis/places.md) (2026-10-16, by eye): starting at a Misiurewicz point (k arms) at
distance `r` from a minibrot of size about `r²`, the view shows 4k arms at 1/2 and 8k at 3/4.
So the start counts as already doubled once, or the fractions are measured from a different
`n`.

Second data point (2026-10-19, by eye, headless `render` and the doubling tour's captures):
from M(4,1) (3 arms) to the period-39 minibrot at −0.101095952005097279 + 0.956281332988737429i
(`locate --radius 1e-6` around M(4,1); distance r = 5.2e−6, size 10^−10.41 ≈ r²), views centred
on the nucleus: 6 arms at span 1.5e−6, 12 at 1.4e−8, 24 around a lace ring at 8e−10, the minibrot
at 1.2e−10. With n = log10(1/r) = 5.28 and m = 10.41: 12 arms at f ≈ 0.50 and 24 at f ≈ 0.74, so
the same "4k at 1/2, 8k at 3/4" as the places data ([Controls: keyboard, pointer and touch](../decisions/controls.md) "The doubling tour").

## How to resolve

Render a contact sheet at `f = 0.25 … 0.9375` for three starts (a tip, M(4,1), a nucleus)
and count arms; once `locate --frame f` exists this is one command per start
([Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)).
