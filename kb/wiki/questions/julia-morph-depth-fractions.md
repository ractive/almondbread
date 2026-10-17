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
  ([[sources/dinkydau-evolution-journals]]).
- wbarry: 2 arms at 3/4, 4 at 7/8, 8 at 15/16
  ([[sources/fractalforums-pathfinding-revisited]]).
- KF: relative fold 0.5 doubles, 0.75 quadruples; absolute 0.75 of the minibrot's depth is
  "typically near an embedded Julia set" ([[sources/kf-manual]]).
- Mu-Ency: the 2-fold embedded Julia set is about `s^(3/4)` (with `n = 0`), read by Munafo
  as the 4-fold set ([[sources/muency-newton-raphson-zooming]]).

## Our data

[[synthesis/places]] (2026-10-16, by eye): starting at a Misiurewicz point (k arms) at
distance `r` from a minibrot of size about `r²`, the view shows 4k arms at 1/2 and 8k at 3/4.
So the start counts as already doubled once, or the fractions are measured from a different
`n`.

## How to resolve

Render a contact sheet at `f = 0.25 … 0.9375` for three starts (a tip, M(4,1), a nucleus)
and count arms; once `locate --frame f` exists this is one command per start
([[synthesis/finding-deep-places]]).
