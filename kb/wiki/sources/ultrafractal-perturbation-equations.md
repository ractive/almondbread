---
title: "Ultra Fractal help: Perturbation equations"
type: source
authors: ["Frederik Slijkerman"]
year: 2021
raw: "web/ultrafractal-perturbation-equations.html"
status: digested
tags: [ultra-fractal, perturbation, formulas, multibrot]
---

# Ultra Fractal help: Perturbation equations

**URL:** <https://www.ultrafractal.com/help/writing/formulas/perturbationequations.html> · **Saved:** `raw/web/ultrafractal-perturbation-equations.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

The Ultra Fractal formula-writer's guide to deriving a perturbed formula and adding it to a user formula, using Mandelbrot as the worked example.

## What it covers

- Derivation: reference `z' = z² + c`; pixel `z2 = z + dz`, `c2 = c + dc`; subtract to get `dz' = 2·z·dz + dz² + dc`. The `z²` and `c` terms cancel.
- Rule: every remaining term must be of order `dz` or `dc`. No "large" term may remain.
- Anti-pattern: `dz' = (z + dz)² − z² + dc` is algebraically equal but fails in double precision, because `|z| ~ 1` and `|dz| ~ 1/magnification`, so the small part is lost in the addition.
- `dz` starts at 0, then `dc`, so all terms are small enough for double precision.
- Only some parameters reduce cleanly: positive integer powers (2, 3, 4) work; a power of 2.5 or a complex power does not. A `perturb` setting says when the equations are valid.
- Formula language: `perturbinit` and `perturbloop` sections, with predefined `#dz` (the delta), `#dpixel` (`dc`) and `#z` (reference value). Mandelbrot: `#dz = 2*#z*#dz + sqr(#dz) + #dpixel`.
- The normal bailout is reused; `#z` for bailout is reference `z` plus `#dz`.
- Compatibility with older versions via the `VER60` compiler symbol (perturbation arrived in version 6).

## Reliability

Primary and correct for the basic derivation. It covers plain perturbation only: no rebasing, no glitch handling, no BLA. The power rule is stated without proof.

## Useful sections

- §Perturbation equations (page body) — derivation, the "no large term" rule, the cancellation anti-pattern, valid powers.
- §Notes — `#dz`, `#dpixel`, `#z`, bailout handling, `VER60`.

## Used by

- [Perturbation](../techniques/perturbation.md)
- [Mandelbrot set (power 2)](../formulas/mandelbrot.md)
- [Multibrot (integer power p)](../formulas/multibrot.md)
- [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)
- [Ultra Fractal 6](../implementations/ultra-fractal.md)
