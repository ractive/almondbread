---
title: "Other formulas: feasibility of deep zoom"
type: formula
status: draft
sources: ["[[sources/ultrafractal-perturbation-equations]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fractalzoomer-readme]]"]
tags: [formula, feasibility, nova, newton, mandelbulb, lyapunov]
---

# Other formulas: feasibility of deep zoom

Which formulas beyond [Mandelbrot set (power 2)](mandelbrot.md), [Multibrot (integer power p)](multibrot.md),
[Mandelbar (Tricorn)](mandelbar.md), [Burning Ship and abs variations](burning-ship.md), [Hybrid formulas](hybrids.md) and
[Julia sets](julia.md) can use perturbation, rebasing and BLA.

For the full survey (all families incl. Buddhabrot, Lyapunov, flames and 3D, a feasibility
matrix, the recommended order and the pluggable-formula architecture) see
[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md).

| Formula | Perturbation | Rebasing | BLA | Verdict |
|---|---|---|---|---|
| Non-integer or complex power `z^a + c` | does not reduce | n/a | n/a | no |
| Nova, Newton, magnet, lambda | reported working (Fractal Zoomer) | reported working | unknown | research |
| Formulas with several critical points | yes | one reference per critical point | one table per reference | later |
| Mandelbulb (3D) | no source | no | no | no |
| Lyapunov | no source | n/a | n/a | no |

## Non-integer and complex powers

Ultra Fractal's `perturb` setting marks when hand-written perturbation
equations are valid: positive integer powers reduce so every term contains
the delta, while non-integer or complex powers do not, and "not all fractal
types are suitable" (src: [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)
§Perturbation equations).

## Nova, Newton, magnet, lambda (convergent and rational maps)

The research notes found no published perturbation scheme for Nova/Newton and
pointed to HPDZ's brute-force 192-bit renders (notes:
implementations_and_formulas §Q3). But in the rebasing thread superheal
reports using rebasing for "Mandelbrot 2-5 powers and burning ships, lambda,
nova, magnet and the newton parameter space without any issue", and mathr
replies that nothing in rebasing is Mandelbrot-specific: it needs the
perturbation identity and an iteration where the reference orbit is 0
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #8, §Reply #10).
superheal is the author of Fractal Zoomer, which lists perturbation theory and
over 500 formulas (src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)). Rational maps need
a quotient-perturbation identity (`(N+n)/(D+d) - N/D = (nD - Nd)/(D(D+d))`,
our restatement), and convergent fractals test convergence to a root rather
than escape, so glitch and BLA radius criteria are not worked out (inference).

## Contradictions

- Notes: "no perturbation scheme found for Nova/Newton". Forum (2021):
  Fractal Zoomer uses perturbation with rebasing for nova, magnet, lambda and
  Newton parameter space. The forum claim is first-hand but unbenchmarked.
  See [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md).

## Multiple critical points

A reference per critical point, rebase to the orbit minimising
`|(Z - Z_o) + z|`, separate BLA table per reference; none implemented in
Fraktaler 3 (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.4 Multiple Critical
Points).

## Mandelbulb and Lyapunov

No source covers perturbation for either. The triplex power uses trig and
atan2 and is non-holomorphic; ray marching multiplies the cost. Lyapunov is a
real logistic-map sequence and deep zooms are rare (inference). Not for
almondbread v1.

# Citations

- [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fractal Zoomer README](../sources/fractalzoomer-readme.md)
