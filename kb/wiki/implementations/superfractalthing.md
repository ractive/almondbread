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
(src: [[sources/mathr-deep-zoom-theory]] §Introduction). Ultra Fractal credits
K.I. Martin and SFT as the origin of the algorithm
(src: [[sources/ultrafractal-perturbation-calculations]]). Wikipedia cites the
fractalforums thread and the "Superfractalthing Maths" PDF and claims about
two orders of magnitude speedup from SA
(src: [[sources/wikipedia-plotting-algorithms]]).

## Status

Historical. Current code availability and licence not researched; Wikipedia's
archive link is dated 2014, the 2013 date comes from mathr (notes:
perturbation_core §1 Gaps).

## What almondbread can learn

Origin of [[techniques/perturbation]] and [[techniques/series-approximation]];
SA itself is superseded by [[techniques/bla]].
