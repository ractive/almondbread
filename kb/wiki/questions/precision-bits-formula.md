---
title: "How many bits does the reference orbit need?"
type: question
status: open
sources: ["[[sources/kf-manual]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/muency-newton-raphson]]"]
tags: [reference-orbit, precision, mpfr]
---

# How many bits does the reference orbit need?

## Question

What precision (bits) should the reference orbit and the Newton nucleus search use
as a function of zoom, image size and iteration count?

## Why it matters

Too few bits give a wrong reference (and wrong images); too many waste CPU on every
iteration ([Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md), [Reference orbit: precision, cost, storage, compression](../techniques/reference-orbit-precision.md)).

## What sources say

- No fetched source states the rule KF or Fraktaler 3 uses (notes: perturbation_core §6).
- The reference is computed in high precision and rounded to double for storage,
  "which works fine most of the time" (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Perturbation).
- Newton zooming makes the nucleus position "exact to within the precision being
  used" (src: [Mu-Ency: Newton-Raphson method](../sources/muency-newton-raphson.md)).
- MPFR caps precision at about 2 gigabits (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).

## Working rule (inference)

bits ≈ log2(magnification) + log2(width in pixels) + 32 to 64 guard bits; about 380
bits at 1e100. Open: whether chaotic orbits near the boundary need extra bits that
grow with iteration count, and how the nucleus search should ramp precision.

## How to resolve

Render benchmark locations with the rule and with +64 and +128 bits; diff images.
Check the KF2+ and Fraktaler 3 documentation for their precision setting.

## Findings (2026-10-14)

Implemented rule: `ceil(-log2 spacing + log2 px + log2 max(|C|,1) + 1) + 48`, rounded up
to 64-bit limbs ([Reference orbit: precision, cost, storage, compression](../techniques/reference-orbit-precision.md)). Measured error of the final
Z against a 256-bit-more orbit: 1 to 3 bits lost over up to 2000 iterations on
periodic orbits, 6 bits over 3000 iterations at the seahorse point, 24 bits over 476
iterations at a chaotic point (measured 2026-10-14, M1 Pro). Loss grows with the orbit's
Lyapunov exponent, so a fixed guard is not enough for long chaotic orbits; the image
diff test above is still needed.

# Citations

- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [Mu-Ency: Newton-Raphson method](../sources/muency-newton-raphson.md)
