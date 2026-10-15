---
title: "Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)"
type: source
authors: ["Aaron Klebanoff"]
year: 2001
raw: "papers/klebanoff-pi-in-mandelbrot.pdf"
status: digested
tags: [pi, parabolic, cusp, escape-time, proof]
---

# Klebanoff: π in the Mandelbrot set

**Citation:** A. Klebanoff, "π in the Mandelbrot set", *Fractals* 9(4), 393–402 (2001),
doi:10.1142/S0218348X01000828 · saved `raw/papers/klebanoff-pi-in-mandelbrot.pdf` (+ `.txt`)
from a course page at Imperial College (<https://www.doc.ic.ac.uk/~jb/teaching/jmc/pi-in-mandelbrot.pdf>),
retrieved 2026-10-15 · **Terms:** copyright World Scientific; personal research copy.

## What it is

The first proof of Boll's observation, for the cusp. Retells Boll's discovery (quoting his web
page) with both tables, then proves the π-theorem.

## Useful parts

- **Definition** (eq. 3): `N(ε) = min{n : Q^n_{1/4+ε}(0) > 2}`, `Q_c(z) = z² + c`; theorem:
  `lim_{ε→0+} √ε N(ε) = π` (eq. 4). Remark 1: the escape value 2 can be any number above 1/2.
- **Method**: the difference equation `x_{k+1} - x_k = (x_k - 1/2)² + ε` is compared with the
  ODE `dx/dt = (x - 1/2)² + ε`, solved by `x = 1/2 + √ε tan(√ε t)`; the time from x = 0 to x = 2
  is π/√ε in the limit (Lemma 1); then the iterates are shown to follow the ODE (series in n,
  remainder estimates).
- **Boll's tables** (p. 394): neck ε = 1 ... 1e-7: 3, 33, 315, 3143, 31417, 314160, 3141593,
  31415928; cusp ε = 1 ... 1e-10: 2, 8, 30, 97, 312, 991, 3140, 9933, 31414, 99344, 314157.
  The neck row for 1e-7 disagrees with our reference count 31415927
  ([[questions/pi-escape-count-conventions]]); every other entry matches
  ([[techniques/pi-in-the-mandelbrot-set]]).
- History: the result along the real axis was known by 1980 (refs 5, 6), Boll found the neck
  in 1991, popularised by Peitgen, Jürgens, Saupe *Chaos and Fractals* and Gerald Edgar
  ([[sources/edgar-pi-and-mandelbrot]]).

## Reliability

Peer-reviewed; cusp only (the neck stayed unproved until
[[sources/brockmoeller-pi-everywhere]]). The proof has no error bound.
