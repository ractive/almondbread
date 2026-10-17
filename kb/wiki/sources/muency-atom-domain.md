---
title: "Mu-Ency: Atom Domain"
type: source
authors: ["Robert Munafo"]
year: 2023
raw: "web/muency-atom-domain.html"
status: digested
tags: [atom-domain, period, coloring-methods, glossary]
---

# Mu-Ency: Atom Domain

**URL:** <https://mrob.com/pub/muency/atomdomain.html> · **Saved:** `raw/web/muency-atom-domain.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Glossary entry (oldest revision 1996, updated 2023-07-08) defining atom domains (dominant period
regions), their computation and their size relative to the mu-atoms they contain.

## What it covers

- Origin: Peitgen and Richter, The Beauty of Fractals (1986) fig. 34: `index(c) = k` where `|p_c^k(0)|` attains `inf_k |p_c^k(0)|`.
- Definition: atom domain of `c` = the `N >= 1` at which `|Z_N|` is smallest over the orbit (excluding `Z_0 = 0`); it gives the period of a nearby "dominant" mu-atom, not proof that `c` is inside it.
- Algorithm: normal escape-time loop tracking the minimum `|z|` and its iteration (pseudocode given). Optional maximum period to suppress high periods.
- Uses: locate island mu-molecules and embedded Julia sets; colour by domain.
- Sizes: cardioid domain is roughly the square root of the cardioid size; the `.1/2a` bulb's domain is ~4x its diameter (ellipse for distorted islands); other descendants teardrops 3-4x wide, 6-7x long. For the R2t island series `size_island ~ 0.1 size_domain^2`; along a period-doubling (Feigenbaum) series domain ~20x the cardioid.

## Reliability

Secondary but careful and long-standing. Size rules are empirical from a few tables. Useful for period
guesses (see [[sources/mathr-interior-distance]]) and Newton-zoom targets (KF "Atom Domain" mode in
[[sources/kf-manual]]).

## Useful sections

- §Origin — Beauty of Fractals definition.
- §Definition — minimum-`|Z_N|` rule.
- §Practical Algorithm — pseudocode.
- §Size and Shape — domain vs component size.
- §Size Relationships for Islands — `0.1 size^2` and Feigenbaum tables.

## Used by

[[techniques/interior-detection]], [[techniques/coloring-methods]], [[techniques/reference-selection]],
[[questions/ball-period-recurrence]], [[synthesis/finding-deep-places]]
