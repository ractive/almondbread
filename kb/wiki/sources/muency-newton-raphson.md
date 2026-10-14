---
title: "Mu-Ency: Newton-Raphson method"
type: source
authors: ["Robert Munafo"]
year: 2023
raw: "web/muency-newton-raphson.html"
status: digested
tags: [newton-raphson, nucleus, period, glossary]
---

# Mu-Ency: Newton-Raphson method

**URL:** <https://mrob.com/pub/muency/newtonraphsonmethod.html> · **Saved:** `raw/web/muency-newton-raphson.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Glossary entry (written 2010, updated 2023-06-22) on Newton's method and its use to find the
nucleus of a mu-atom (hyperbolic component) of known period.

## What it covers

- Newton step `x_{n+1} = x_n - f(x_n) / f'(x_n)`.
- Nucleus of period `P`: solve `L_P(C) = 0` where `L_P` is `z -> z^2 + C` iterated `P` times from `z_0 = 0`.
- No need for polynomial expansions: iterate `L_{N+1} = L_N^2 + C` and `L'_{N+1} = 2 L_N L'_N + 1` from `L_0 = L'_0 = 0`.
- Worked example: period 3, start `-0.12+0.76i`, converges to `-0.12256116687638 + 0.74486176662039i` in 3 steps (5 digits after two), quadratic convergence.

## Reliability

Secondary but correct and simple. Says nothing about convergence basins, the precision needed for
deep nuclei, period detection, or the ball/box period methods used in KF.

## Useful sections

- §Finding a Mu-atom Nucleus — the equation and example.
- §Iterative Evaluation of LN and its Derivative — the `L`, `L'` recurrences.

## Used by

[[techniques/reference-selection]], [[techniques/interior-detection]], [[questions/ball-period-recurrence]],
[[implementations/kalles-fraktaler-2]], [[implementations/fraktaler-3]]
