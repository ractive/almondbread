---
title: "Fixed-size multiplication kernels on M1 (gmp-devel, 2024)"
type: source
authors: ["Albin Ahlbäck"]
year: 2024
raw: "web/gmp-fixed-size-mul.html"
status: digested
tags: [gmp, apple-silicon, arbitrary-precision, multiplication, cpu]
---

# Fixed-size multiplication kernels on M1 (gmp-devel, 2024)

**URL:** <https://gmplib.org/list-archives/gmp-devel/2024-April/006255.html> · **Saved:** `raw/web/gmp-fixed-size-mul.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

A gmp-devel reply by Albin Ahlbäck, 5 April 2024, in the thread "How to calculate cycles/limb in assembly routines". He reports hand-written fixed-size multiplication routines that beat GMP's general basecase multiply on Apple M1.

## What it covers

- Routines of the form `mpn_mul_N(rp, ap, an, bp)` for N = 1 … 15: one operand has a fixed, compile-time length N limbs; the code is "half-hardcoded" per N.
- For N = 8 they are 80 % faster asymptotically than `mpn_mul_basecase` on M1 (tested on the cfarm Mac mini).
- Used as the base case under Toom-22, the crossover to GMP's Toom-33 moved to about 480 limbs.
- Downside: each N is hand-coded, so the code is large; the author doubts it would fit into GMP.

## Reliability

Primary but anecdotal: one message, one machine, no table of timings, no code. The "80 %" is for one N. Still a useful signal that fixed-size, unrolled limb kernels pay off on M1 at the sizes a reference orbit needs (inference).

## Contradictions

- The manifest lists the title as "Fixed-size multiplication kernels (gmp-devel)" and authors as "gmp-devel contributors". The message itself is by Albin Ahlbäck, and its subject line is "How to calculate cycles/limb in assembly routines". This page uses the real author.

## Useful sections

- §How to calculate cycles/limb in assembly routines (the message) — the `mpn_mul_N` results and the Toom crossover.

## Used by

- [Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md)
- [Reference orbit: precision, cost, storage, compression](../techniques/reference-orbit-precision.md)
