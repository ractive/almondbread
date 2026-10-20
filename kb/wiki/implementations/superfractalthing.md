---
title: "SuperFractalThing"
type: implementation
status: stub
license: "unknown"
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/ultrafractal-perturbation-calculations]]", "[[sources/wikipedia-plotting-algorithms]]"]
tags: [implementation, java, perturbation, series-approximation, history]
---

# SuperFractalThing

K.I. Martin's Java program and white paper `sft_maths.pdf` (2013) that
popularised the two techniques every later renderer built on: perturbation
against one high-precision reference orbit, and series approximation in the
pixel offset `c` to skip early iterations
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Introduction). Ultra Fractal credits
K.I. Martin and SFT as the origin of the algorithm
(src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)). Wikipedia cites the
fractalforums thread and the "Superfractalthing Maths" PDF and claims about
two orders of magnitude speedup from SA
(src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)).

## Status

Historical. Current code availability and licence not researched; Wikipedia's
archive link is dated 2014, the 2013 date comes from mathr (notes:
perturbation_core §1 Gaps).

## What almondbread can learn

Origin of [Perturbation](../techniques/perturbation.md) and [Series approximation (legacy)](../techniques/series-approximation.md);
SA itself is superseded by [Bivariate linear approximation (BLA)](../techniques/bla.md).

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md)
- [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md)
