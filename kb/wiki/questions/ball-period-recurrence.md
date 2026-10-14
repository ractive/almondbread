---
title: "What exactly is the ball-method period recurrence?"
type: question
status: open
sources: ["[[sources/kf-manual]]", "[[sources/muency-newton-raphson]]", "[[sources/muency-atom-domain]]"]
tags: [reference-selection, period-detection, newton]
---

# What exactly is the ball-method period recurrence?

## Question

What recurrence does the knighty/gerrit "ball-period" method use for the radius of
the disc image (`R ← 2|z|R + R² + r`, or `R ≈ |dz/dc|·r`, or something else), and
what is the stopping test?

## Why it matters

Reference selection ([[techniques/reference-selection]]) finds the period of the
nearest minibrot before Newton refines its nucleus. A wrong recurrence gives wrong
periods, so Newton converges to the wrong nucleus or not at all.

## What sources say

- KF2+ uses an interval-arithmetic ball-period method by default, with the box
  method as an alternative; it "works much better in skewed locations"
  (src: [[sources/kf-manual]]). The theory note (knighty 2017, "A little note about
  Ball arithmetic") was not fetched.
- Newton for the nucleus iterates L_p and L_p' without expanding polynomials
  (src: [[sources/muency-newton-raphson]]).
- Atom domains give a cheap candidate: the iteration where |Z_n| is minimal
  (src: [[sources/muency-atom-domain]]).
- The pseudocode in the research notes is marked as unverified (notes: perturbation_core §5).

## How to resolve

Find knighty's note or read the period code of an MIT-licensed or
documentation-level source; otherwise derive the ball bound ourselves
(|z + e|² + c + d with |e| ≤ R, |d| ≤ r gives R' = 2|z|R + R² + r) and test against
the box method and against atom-domain periods on known locations.

## Findings (2026-10-14)

Implemented all three candidates (milestone 3a, [[techniques/reference-selection]]):
the rigorous ball derived above, the first-order `|dz/dc|·r` ball and the box method. The
rigorous ball matches an independent mpmath implementation of the same recurrence on 8
locations; it reports lower periods than the other two in skewed places (973 vs 998
around the seahorse point at r = 1e-10; 96084 vs 280245 vs none at r = 9e-36), and its
period's nucleus can lie outside the disc, so Newton must be checked. Still open: which
variant KF uses, and which one gives the best references in practice.
