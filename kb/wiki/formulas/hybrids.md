---
title: "Hybrid formulas"
type: formula
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fraktaler-3-site]]", "[[sources/kf-page]]", "[[sources/kf-manual]]"]
tags: [hybrids, formula, perturbation, bla, rebasing, code-generation]
---

# Hybrid formulas

A hybrid interleaves formulas in a loop, e.g. Mandelbrot and Burning Ship as
(M,BS,M,M), to combine their features (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
§Hybrid Fractals).

## Formula models

- **KF stanzas**: 1-4 stanzas, each 1-2 lines, each line two operators
  combined by +, - or x; each operator has abs x, abs y, negate x, negate y,
  integer power p and complex multiplier a; `+c` at the end of each stanza;
  per-stanza repeat counts (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Hybrid
  Fractals; Ctrl-H designer in [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).
- **Fraktaler 3 lines/opcodes**: each line is power P with |X|, |Y|, -X, -Y
  toggles; lines iterate in turn; advanced mode exposes opcodes store, sqr,
  mul, absx, absy, negx, negy, rot, add. N lines cost N times the reference
  work and memory (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## Perturbation per operator

Derive the delta mechanically, operator by operator. In our words, for one
operator with reference input `Z` and delta `z`:

```text
w = z; W = Z + z; B = Z            // B: reference value after transforms
if absX: w.re = diffabs(Z.re, w.re); W.re = |W.re|; B.re = |B.re|
if absY: same for imaginary parts
if negX: w.re = -w.re; W.re = -W.re; B.re = -B.re
if negY: same for imaginary parts
S = sum_{i=0..p-1} W^i B^(p-1-i)   // see formulas/multibrot
return a * w * S                   // delta of a * (transformed input)^p
```

(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Hybrid Fractals; written in our own
form.) Combining operators within a line uses perturbation algebra for sums
and products; the reference `Z` is iterated alongside in low precision within
a stanza, since only `+C` needs high precision.

Rescaling threads the scale `s` through: start with `W = Z + z s`, use
`diffabs(Z/s, z)` in the abs cases, and do a full floatexp iteration whenever
either part of the reference is small (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
§Hybrid Fractals).

## Code generation

Interpreting the toggles per iteration is very slow; generate source per
formula (OpenCL in KF and Fraktaler) so the branches are resolved once. This
is faster even on the same device that would interpret the parametric code
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Code Generation). For almondbread:
emit MSL per formula, specialise with function constants
([Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)).

## Rebasing and BLA

- The reference iteration can reset at any phase, so a loop of k phases
  needs k references: (M,BS,M,M) needs (M,BS,M,M), (BS,M,M,M), (M,M,M,BS) and
  (M,M,BS,M). A pre-periodic prefix needs one more per prefix step. Each
  phase needs as many references as the starting formula has critical
  points; they can be computed in parallel
  (src: [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing).
- One BLA table per reference; rebasing switches to the current phase's
  reference (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.3 Hybrid BLA).
- Non-conformal phases force the 2x2 matrix BLA everywhere
  ([Mandelbar (Tricorn)](mandelbar.md), [Burning Ship and abs variations](burning-ship.md)).
- KF's single-reference method "does not work with every hybrid formula"
  (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

## Verdict for almondbread

Last of the realistic formulas. Requires: opcode model, per-phase references
and tables, matrix BLA, code generation. Imagina-style LA does not apply.

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
