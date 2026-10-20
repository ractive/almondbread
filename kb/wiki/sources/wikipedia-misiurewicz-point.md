---
title: "Wikipedia: Misiurewicz point"
type: source
authors: ["Wikipedia contributors"]
year: 2026
raw: "web"
status: digested
tags: [misiurewicz, places, dendrite, spiral]
---

# Wikipedia: Misiurewicz point

**URL:** <https://en.wikipedia.org/wiki/Misiurewicz_point> · **Not saved** (read 2026-10-14) ·
**Terms:** CC BY-SA 4.0; numbers are facts.

## What it is

Encyclopedia article on parameters c whose critical orbit is strictly preperiodic:
`M(k,n)`, preperiod k, period n. They are the tips, branch points and spiral centres of the
set's filaments; the set is asymptotically self-similar around them.

## What it covers

- Examples: `M(2,1)` c = -2 (antenna tip, one external ray 1/2); `M(2,2)` c = i (tip of a
  1/3-limb branch, ray 1/6); `M(3,1)` c ≈ -0.228155493653962 + 1.115142508039937i (tip of
  the other branch); `M(4,1)` c ≈ -0.10109636384562 + 0.95628651080914i (principal
  branch point of the 1/3 limb, three rays 9/56, 11/56, 15/56); `M(3,1)` c ≈
  -1.54368901269208 (principal point of the antenna, rays 5/12 and 7/12); "M(23,2)"
  c ≈ -0.77568377 + 0.13646737i (centre of a two-arm spiral in seahorse valley).
- Equation: `f_c^k(0) = f_c^{k+n}(0)` with k, n minimal.

## Reliability

Good for the definitions and the examples' approximate values. Two slips, found while
refining them with Newton (`almondbread locate --misiurewicz k,p`, 2026-10-14):

- The label `M(3,1)` is used for two different points (the 1/3-limb tip and the antenna
  point).
- The seahorse spiral point is labelled M(23,2), but with z_0 = 0 (the convention that
  makes c = -2 M(2,1)) its orbit satisfies z_24 = z_26 and not z_23 = z_25: it is
  M(24,2). Newton for (23,2) from the given digits wanders off to another root; (24,2)
  converges in 5 steps to -0.775683768009053797... + 0.136467368294690124...i.

## Contradictions

The off-by-one above; see [Places: a catalogue of famous views to visit](../synthesis/places.md). Not worth a `questions/` page: the
orbit check settles it.

## Useful sections

- §Examples: the six points above.
