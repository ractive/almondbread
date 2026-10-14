---
title: "Multibrot (integer power p)"
type: formula
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/ultrafractal-perturbation-equations]]", "[[sources/kf-page]]", "[[sources/kf-manual]]", "[[sources/fraktaler-3-site]]"]
tags: [multibrot, formula, perturbation, bla]
---

# Multibrot (integer power p)

## Iteration

`z_{n+1} = z_n^p + c`, `z_0 = 0`, integer `p >= 2`. The only critical point
is 0 (with multiplicity p-1), so rebasing works exactly as for
[[formulas/mandelbrot]].

## Perturbed form (our derivation)

Binomial expansion of `(Z+z)^p - Z^p`:

`z' = sum_{k=1..p} C(p,k) Z^{p-k} z^k + c`

Every term carries at least one factor `z`, so all terms are small: positive
integer powers "reduce" (src: [[sources/ultrafractal-perturbation-equations]]
§Perturbation equations). Horner-style evaluation avoids recomputing powers.
An equivalent factored form used in hybrid code is
`z' = z * S + c` with `S = sum_{i=0..p-1} W^i Z^{p-1-i}`, `W = Z + z`
(src: [[sources/mathr-deep-zoom-theory]] §Hybrid Fractals). The factored form
is shorter for large p and has no binomial coefficients.

```text
// our pseudocode, power p >= 2, complex arithmetic
// S_1 = 1, S_{k+1} = W * S_k + Z^k  gives  S_p = sum_{i=0..p-1} W^i Z^(p-1-i)
W = Z + z
S = 1; Zk = 1
for k in 1..<p:
    Zk = Zk * Z
    S  = S * W + Zk
z = z * S + c
```

A concrete kernel unrolls the loop per p via function constants, see
[[platform/metal-compute]]. Check: p = 2 gives `S = W + Z = 2Z + z`, the
Mandelbrot step.

## BLA form

- `A_{m,1} = p Z_m^{p-1}`, `B_{m,1} = 1` (Jacobian `J_f(Z) = p Z^{p-1}`)
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation:
  radius `r = max(0, eps (|Z| - max|c|)/(|J_f(Z)| + 1))` with general J).
- The radius formula was derived for power 2; its validity for high p is
  heuristic (inference). The neglected term is `C(p,2) Z^{p-2} z^2`, so
  linear validity needs `|z| << (2/(p-1)) |Z|` (inference).
- Merge rule identical to [[techniques/bla]].

## Rescaling

Terms `S^{k-1} w^k` appear; drop those that underflow
(src: [[sources/mathr-deep-zoom-theory]] §Rescaling). KF's scaled double
reaches about 1e600 for power 2 but only about 1e400 for power 3, and
analytic DE breaks at some power-3 locations unless SA is disabled
(src: [[sources/kf-manual]]).

## Support

KF powers 2-10 (src: [[sources/kf-page]]); Fraktaler 3 any power P per
formula line (src: [[sources/fraktaler-3-site]]); Fractal Zoomer.
Fraktaler warns plain float "can cause iteration-band glitches with high
powers" (src: [[sources/fraktaler-3-site]]).

## Verdict for almondbread

Easy second formula: same reference machinery, one critical point, scalar
complex BLA. Generate one kernel per p. Imagina-style LA is power-2 only
(inference from [[techniques/linear-approximation-imagina]]); use the BLA tree.
