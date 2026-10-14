---
title: "Faster deep references: three-thread orbit, progressive delivery, cheap Newton"
type: technique
status: draft
sources: ["[[sources/fractalshark-readme]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]"]
tags: [reference, arbitrary-precision, threads, newton, latency]
---

# Faster deep references: three-thread orbit, progressive delivery, cheap Newton

What R6 did to the time from "Go to coordinates" (or any jump without a usable
reference) to the first frame at depths of 1e-300 to 1e-2000. Measurements are in
[[synthesis/benchmarks]] R6; the arithmetic itself is on
[[platform/mpfr-gmp-on-apple-silicon]] and [[decisions/arbitrary-precision]].

## Where the time went

A jump with no nucleus in the view (the dendrite benchmarks, c = i) waited for:

1. Period detection (ball method) up to the first period found.
2. Newton for that period, ~8 steps of `p` iterations with the derivative in full
   precision: four big products per iteration for `dz` plus two for `z`. At c = i the
   nucleus lands outside the view and is thrown away; at 1e-2000 that cost 1.3 s.
3. The centre orbit to twice the iteration limit (400 001 points for the default
   200 000), before any frame.

And one arithmetic accident: orbits of exact short values (c = i: `Z` is 0, i,
-1 + i, -i, ...) fell off the short product onto full Karatsuba products on nearly
every multiplication (see "Short products" below).

## Three threads per iteration (MT3)

The squaring form needs `X²`, `Y²` and `(X+Y)²`, three independent products
(FractalShark's MT3 splits the multiplication form the same way, src:
[[sources/fractalshark-readme]] §4). Ours (`parallel.rs`, safe Rust, no
dependencies):

```text
three threads, each holding Z = X + iY and C
thread r in {0, 1, 2}, iteration i:
    P_r = X² | Y² | (X+Y)²            (own product)
    publish P_r in slot[i mod 2][r]; ready[r] = i + 1      (release)
    for k != r: wait ready[k] >= i + 1; read P_k           (acquire)
    X = (P0 - P1) + Cx;  Y = (P2 - (P0 + P1)) + Cy          (same adds as the serial loop)
thread 0 also writes the float32 / floatexp point and tests escape
```

- One all-to-all exchange per iteration instead of publish-then-collect: every thread
  redoes the five additions, which is cheaper than a second cache-line round trip.
- Double buffering by iteration parity is enough: a thread cannot start iteration
  i + 2 before every thread has published i + 1, which each does only after reading
  all of iteration i.
- Values travel through `AtomicU64` slots padded to 128-byte lines (Apple silicon),
  relaxed element accesses ordered by release/acquire counters; no `unsafe`.
- Waits spin 4096 times, then `yield_now`, so a descheduled helper on a loaded machine
  costs a time slice, not a stall; a drop guard stops the helpers on any exit.
- The operations and inputs are those of the serial loop, so the orbit is
  bit-identical (`orbit_digest`, unit tests through escapes, cancellation and resume).
- The exchange costs ~0.1 µs; the measured crossover is ~1400 bits (22 limbs). The
  default (`Threads::Automatic`) uses it from 24 limbs (1536 bits) and 256 iterations,
  on machines with at least 8 cores (a 6-core iPhone has 2 performance cores; a
  helper on an efficiency core would set the pace).

Splitting each product further (column ranges of the short product on 6 threads, with
a carry merge) would cut the critical path by roughly another third at 1e-1000
(estimate, not built): the five additions and the exchange remain.

## Progressive delivery of centre references

With [[techniques/rebasing]] every prefix of an orbit is a correct reference: a pixel
that reaches its end rebases to `m = 0`. So the reference worker no longer withholds
the centre orbit until all 2x-limit points exist:

- `ReferenceOrbitComputer::resume` continues a computation from its `final_z`
  exactly (bit-identical to one long call), so the orbit runs in stages.
- `Reference::compute_big_progressive` hands out the prefix of 1/16 of the request
  (1/8 of the view's limit; at least 4096 points), then the prefix of half the request
  (the view's limit itself), then the whole orbit.
- `ReferenceWorker::poll(false)` returns such a prefix while the computation keeps
  running; the scheduler's adoption rules are unchanged: a prefix shorter than the
  limit is `Stale` (used now, the pending request still serves the view, so nothing is
  restarted); the limit-length prefix is `Good`. Offscreen renders still wait for the
  whole orbit.

A nucleus reference is one period long and is delivered whole.

## Newton with a cheap derivative

Newton's step `c -= z_p / z_p'` only needs the derivative to a few digits. Computing
`z'` in extended-range f64 ([[techniques/extended-range-floats]]) over the orbit
(which now runs through the orbit code, with fixed-size kernels and three threads)
makes a step ~3x cheaper. The derivative's relative error turns quadratic convergence
into linear convergence of ~50-90 bits per step once the error is below ~2^-50 in
units of the minibrot; from a typical start (the view centre, a view-size away from
the nucleus) the step count is unchanged (8 at c = i, 1e-1000). When the steps stop
accelerating with more than two such steps left to the precision floor (a start
already very close to the nucleus), exact Newton finishes. The nucleus is not
bit-identical to before, but within the same tolerance (the oracle tests ask for
2^-(bits - 24) against mpmath).

## Short products: round-down without the full product

`round_short_product` gives up when the bits of limb n-1 below the round bit are
zero, because the skipped columns could make the value a tie. That is only possible
when the round bit is set; with it clear the exact value is below half an ulp whatever
the skipped columns add (they add less than n units, checked). Deciding round-down
there keeps exact short values on the short path: dendrite orbits 2.5x faster, every
other orbit unchanged.

## What did not pay (measured or bounded)

- Toom-3 and NTT multiplication: the short Comba product beats full Karatsuba up to
  at least 256 limbs (R1, [[platform/mpfr-gmp-on-apple-silicon]]), and depths to
  1e-3000 need at most ~160 limbs. Toom-3 is asymptotically ~1.3x cheaper than
  Karatsuba but would still have to beat a short product that already does half the
  limb products of a full one; FFT crossovers are in the thousands of limbs.
- Lower precision for part of the orbit: rounding at iteration k propagates like a
  perturbation of C of size `ulp · |Z_k| / |dZ_k/dC|`; early iterations have
  `|dZ_k/dC| ~ 1`, so the full pixel precision is needed from the start. Only the
  guard bits could go, a ~2 % saving.

## See also

[[techniques/reference-orbit-precision]], [[techniques/reference-selection]],
[[techniques/rebasing]], [[decisions/arbitrary-precision]].
