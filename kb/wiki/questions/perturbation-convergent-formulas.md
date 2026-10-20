---
title: "Can perturbation and rebasing handle Nova, Newton and Magnet formulas?"
type: question
status: open
sources: ["sources/fractalforums-zhuoran-rebasing-bla", "sources/ultrafractal-perturbation-equations", "sources/fractalzoomer-readme"]
tags: [formulas, perturbation, rebasing, contradiction]
---

# Can perturbation and rebasing handle Nova, Newton and Magnet formulas?

## Question

Is there a working perturbation (and rebasing) scheme for convergent or rational
formulas such as Nova, Newton parameter space and Magnet, and what does it cost?

## Contradictions

- The research notes found no published perturbation scheme for Nova/Newton; the
  only deep Nova zooms found used brute-force fixed precision (192-bit at HPDZ)
  (notes: implementations_and_formulas §Q3).
- In the rebasing thread, superheal (Fractal Zoomer's developer) wrote that he had
  used rebasing on Mandelbrot powers 2 to 5, Burning Ship, lambda, nova, magnet and
  the Newton parameter space "without any issue", and that it works for Julia sets;
  mathr replied that nothing in rebasing depends on the Mandelbrot set: it needs the
  basic perturbation property and an iteration at which the reference orbit is 0
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)).
- Ultra Fractal lets formula authors write perturbed equations by hand and marks
  validity with a `perturb` flag; non-integer powers do not reduce
  (src: [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)).

## Why it matters

It moves Nova/Newton from "defer" to "feasible" in [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)
and shapes the opcode formula model ([Hybrid formulas](../formulas/hybrids.md)).

## How to resolve

Check Fractal Zoomer (GPL-3.0; read behaviour and docs, not code) for which
formulas support perturbation (src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)); derive the
perturbed Nova step ourselves (rational: perturb numerator and denominator,
quotient identity) and test rebasing at the formula's critical point.

# Citations

- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)
- [Fractal Zoomer README](../sources/fractalzoomer-readme.md)
