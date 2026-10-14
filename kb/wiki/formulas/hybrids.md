---
title: "Hybrid formulas"
type: formula
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/mathr-deep-zoom-page]]", "[[sources/fraktaler-3-site]]", "[[sources/kf-page]]", "[[sources/kf-manual]]"]
tags: [hybrids, formula, perturbation, bla, rebasing, code-generation]
---

# Hybrid formulas

A hybrid interleaves formulas in a loop, e.g. Mandelbrot and Burning Ship as
(M,BS,M,M), to combine their features (src: [[sources/mathr-deep-zoom-theory]]
§Hybrid Fractals).

## Formula models

- **KF stanzas**: 1-4 stanzas, each 1-2 lines, each line two operators
  combined by +, - or x; each operator has abs x, abs y, negate x, negate y,
  integer power p and complex multiplier a; `+c` at the end of each stanza;
  per-stanza repeat counts (src: [[sources/mathr-deep-zoom-theory]] §Hybrid
  Fractals; Ctrl-H designer in [[sources/kf-page]]).
- **Fraktaler 3 lines/opcodes**: each line is power P with |X|, |Y|, -X, -Y
  toggles; lines iterate in turn; advanced mode exposes opcodes store, sqr,
  mul, absx, absy, negx, negy, rot, add. N lines cost N times the reference
  work and memory (src: [[sources/fraktaler-3-site]]).

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

(src: [[sources/mathr-deep-zoom-theory]] §Hybrid Fractals; written in our own
form.) Combining operators within a line uses perturbation algebra for sums
and products; the reference `Z` is iterated alongside in low precision within
a stanza, since only `+C` needs high precision.

Rescaling threads the scale `s` through: start with `W = Z + z s`, use
`diffabs(Z/s, z)` in the abs cases, and do a full floatexp iteration whenever
either part of the reference is small (src: [[sources/mathr-deep-zoom-theory]]
§Hybrid Fractals).

## Code generation

Interpreting the toggles per iteration is very slow; generate source per
formula (OpenCL in KF and Fraktaler) so the branches are resolved once. This
is faster even on the same device that would interpret the parametric code
(src: [[sources/mathr-deep-zoom-theory]] §Code Generation). For almondbread:
emit MSL per formula, specialise with function constants
([[platform/metal-compute]]).

## Rebasing and BLA

- The reference iteration can reset at any phase, so a loop of k phases
  needs k references: (M,BS,M,M) needs (M,BS,M,M), (BS,M,M,M), (M,M,M,BS) and
  (M,M,BS,M). A pre-periodic prefix needs one more per prefix step. Each
  phase needs as many references as the starting formula has critical
  points; they can be computed in parallel
  (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing).
- One BLA table per reference; rebasing switches to the current phase's
  reference (src: [[sources/mathr-deep-zoom-page]] §4.3 Hybrid BLA).
- Non-conformal phases force the 2x2 matrix BLA everywhere
  ([[formulas/mandelbar]], [[formulas/burning-ship]]).
- KF's single-reference method "does not work with every hybrid formula"
  (src: [[sources/kf-manual]]).

## Verdict for almondbread

Last of the realistic formulas. Requires: opcode model, per-phase references
and tables, matrix BLA, code generation. Imagina-style LA does not apply.
