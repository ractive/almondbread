---
title: "Wikipedia: Knuth's up-arrow notation"
type: source
authors: ["Wikipedia contributors"]
year: 2026
raw: "web"
status: skimmed
tags: [big-numbers, up-arrow, how-big]
---

# Wikipedia: Knuth's up-arrow notation

**URL:** <https://en.wikipedia.org/wiki/Knuth%27s_up-arrow_notation> · **Saved:** not saved (`raw: "web"`) · **Terms:** CC BY-SA

## What it is

Encyclopedia article on the notation `a ↑^n b` for iterated exponentiation, tetration and
beyond. Read through a summarising fetch on 2026-10-15, not in full.

## What it covers

- Introduced by Donald Knuth in 1976.
- Definition for integers `a ≥ 0, n ≥ 1, b ≥ 0`: `a ↑^1 b = a^b`; `a ↑^n 0 = 1` for `n > 1`;
  otherwise `a ↑^n b = a ↑^(n-1) (a ↑^n (b-1))`.
- Examples: `3↑↑3 = 3^3^3 = 3^27 = 7 625 597 484 987`; `3↑↑↑3 = 3↑↑(3↑↑3)`, a tower of
  7 625 597 484 987 threes.
- The article does not mention googolplex as `10↑10↑100`.

## Reliability

Standard, stable mathematical definitions; fine for the notation.

## Used by

[[synthesis/how-big-is-big]]
