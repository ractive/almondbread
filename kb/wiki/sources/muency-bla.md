---
title: "Mu-Ency: Bivariate Linear Approximation"
type: source
authors: ["Robert Munafo"]
year: 2023
raw: "web/muency-bla.html"
status: digested
tags: [bla, rebasing, perturbation, glossary]
---

# Mu-Ency: Bivariate Linear Approximation

**URL:** <https://mrob.com/pub/muency/bivariatelinearapproximati.html> · **Saved:** `raw/web/muency-bla.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed (page itself states CC BY-NC 4.0)

## What it is

Glossary entry in Robert Munafo's Mandelbrot Set Glossary and Encyclopedia (first version
2023-06-20, rewritten 2023-07-03, page updated 2025-09-10) attempting to explain BLA and rebasing.

## What it covers

- Defines BLA as a degree-1 series in two variables: `dz_n = A_l dz_m + B_l dc`, `n = m + l`, with `dz_m` the delta at the last rebase.
- Coefficient recurrence: `A_0 = 1`, `B_0 = 0`, `A_{l+1} = 2 Z_n A_l`, `B_{l+1} = 2 Z_n B_l + 1` (dropping `dz^2`, `dz dc`, `dc^2` terms).
- Shows that substituting the model back gives `dz_{n+1} = 2 Z_n dz_n + dc`, and concludes the coefficients are "not needed", so the code is simply the perturbation step `dz = 2 Z dz + dz^2 + dc`; quotes Zhuoran's step and notes Heiland-Allen's refactor `z = (2Z + z) z + c` saves one multiplication.
- Says rebasing repairs accumulated roundoff by recomputing from the high-precision reference when `Z + dz` nears the origin.
- Says BLA matters mainly for dwell limits in the millions or higher.

## Reliability

Low. Secondary and partly wrong or confused:
- It misses the point of BLA: the coefficients exist to skip many iterations at once with a validity radius and a merged table ([mathr: Deep zoom (reference page)](mathr-deep-zoom-page.md) §3.2-§3.4); concluding they are "not needed" describes plain perturbation instead.
- Dates the rebasing test to "2021 Dec"; Zhuoran posted it on 3 Aug 2021 ([fractalforums.org: Another solution to perturbation glitches (topic 4360)](fractalforums-zhuoran-rebasing-bla.md)).
- Says the reference must be periodic (nucleus or Misiurewicz point); rebasing and BLA do not require this.
- Describes rebasing as "two tests, one for a c delta and one for a z delta"; the published rule is one test `|Z+z| < |z|` plus running out of reference.
- Says Zhuoran collaborated with Heiland-Allen on Imagina; Imagina is Zhuoran's project (unverified either way).
- Says BLA is used in "more recent versions of Kalles Fraktaler"; KF never got BLA ([Kalles Fraktaler 2 + (home page)](kf-page.md) §Future).
The single-step coefficient recurrence and the one-multiply refactor are correct.

## Useful sections

- §The Model — `A_l`, `B_l` recurrences.
- §Converting the Modeled Iteration to Code — the (misleading) argument that coefficients are unnecessary; the `(2Z + z) z + c` refactor.

## Used by

[Bivariate linear approximation (BLA)](../techniques/bla.md), [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md), [Does Kalles Fraktaler 2+ implement BLA?](../questions/kf-bla-support.md)
