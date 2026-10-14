---
title: "Arbitrary precision: own Swift/C implementation, not GMP/MPFR"
type: decision
date: 2026-10-14
status: accepted
tags: [architecture, arbitrary-precision, ios]
---

# Arbitrary precision: own Swift/C implementation, not GMP/MPFR

## Context

Zooms past about 1e-15 need the reference orbit (and the viewport centre)
in arbitrary precision. GMP/MPFR are the usual choice
([[platform/mpfr-gmp-on-apple-silicon]]), but almondbread also targets
iPhone and iPad ([[platform/ios-ipad-considerations]]).

## Proposal

Write our own fixed-size multi-limb binary floating or fixed-point type
(UInt64 limbs, `multipliedFullWidth`, Karatsuba above a threshold if
needed) covering what the reference orbit and Newton/period detection
need: add, subtract, multiply, square, compare, conversion to and from
Double/floatexp and decimal strings.

## Reasons

- GMP and MPFR are LGPL. Static linking into an App Store app conflicts
  with the LGPL relinking requirement; an own type keeps iOS shippable.
- Reference orbits need only a handful of operations at fixed precision;
  hand-tuned fixed-size multiplication beat GMP's basecase by 80 % at 8
  limbs in one 2024 report ([[platform/mpfr-gmp-on-apple-silicon]]).
- No C library build per platform and architecture.

## Risks

- Correctness: needs tests against a known-good implementation (e.g. GMP
  via a test-only tool on macOS, or Python's `decimal`/`mpmath`).
- Speed at extreme depth (tens of thousands of digits) where GMP's FFT
  multiplication wins; not a near-term concern.

## Outcome (2026-10-14)

Built as `Sources/AlmondbreadMath` (pure Swift, builds for macOS 14 and iOS 17): a
correctly rounded floating `BigFloat` rather than fixed point, because Newton's
derivative and the period radii need the exponent range. GMP is used only by a
test-only benchmark. It beats GMP `mpf` on the reference orbit up to 1024 bits (1.1x to
1.55x) and loses from 4096 bits (0.56x at 16384); see
[[platform/mpfr-gmp-on-apple-silicon]] and [[synthesis/benchmarks]] (measured
2026-10-14, M1 Pro, release). The correctness risk is covered by bit-exact oracle tests.
