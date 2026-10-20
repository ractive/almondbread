---
title: "Faster deep references: three-thread orbit, progressive delivery, cheap Newton"
type: technique
status: draft
sources: ["sources/fractalshark-readme", "sources/fractalforums-zhuoran-rebasing-bla"]
tags: [reference, arbitrary-precision, threads, newton, latency]
---

# Faster deep references: three-thread orbit, progressive delivery, cheap Newton

What R6 did to the time from "Go to coordinates" (or any jump without a usable
reference) to the first frame at depths of 1e-300 to 1e-2000. Measurements are in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) R6; the arithmetic itself is on
[Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md) and [Arbitrary precision: own Swift/C implementation, not GMP/MPFR](../decisions/arbitrary-precision.md).

## Where the time went

A jump with no nucleus in the view (the dendrite benchmarks, c = i) waited for:

1. Period detection (ball method) up to the first period found.
2. Newton for that period, ~8 steps of `p` iterations with the derivative in full
   precision: four big products per iteration for `dz` plus two for `z`. At c = i the
   nucleus lands outside the view and is thrown away; at 1e-2000 that cost 1.3 s.

Result for a jump to dendrite-1e1000: first frame after ~0.1 s instead of ~1.8 s
([almondbread benchmarks and verification record](../synthesis/benchmarks.md) R6).
3. The centre orbit to twice the iteration limit (400 001 points for the default
   200 000), before any frame.

And one arithmetic accident: orbits of exact short values (c = i: `Z` is 0, i,
-1 + i, -i, ...) fell off the short product onto full Karatsuba products on nearly
every multiplication (see "Short products" below).

## Three threads per iteration (MT3)

The squaring form needs `X²`, `Y²` and `(X+Y)²`, three independent products
(FractalShark's MT3 splits the multiplication form the same way, src:
[FractalShark README](../sources/fractalshark-readme.md) §4). Ours (`parallel.rs`, safe Rust, no
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
- Waits spin 1024 times, then `yield_now`, so a descheduled helper on a loaded machine
  costs a time slice, not a stall; after 2 ms of yielding (a real stall: the partner is
  descheduled or the caller busy elsewhere) a wait sleeps 0.2 ms between polls instead of
  burning a core (since 2026-10-20, review P1). In lockstep no wait gets past the spin
  phase. A drop guard stops the helpers on any exit.
- The operations and inputs are those of the serial loop, so the orbit is
  bit-identical (`orbit_digest`, unit tests through escapes, cancellation and resume).
- The exchange costs ~0.1 µs; the measured crossover is ~1400 bits (22 limbs): 1.8x at
  3456 bits (1e-1000), 2.2x at 6784 (1e-2000), 2.3x at 10048 (measured 2026-10-15, M1
  Pro, release, medians, lightly loaded; [almondbread benchmarks and verification record](../synthesis/benchmarks.md) R6).
  `Threads::Automatic` uses it from 24 limbs (1536 bits) and 256 iterations, where the
  core layout allows (`parallel::helpers_pay`): at least 6 **performance** cores when the
  platform reports them (the Metal backend reads `hw.perflevel0.logicalcpu` and hands it
  to the platform-free math crate), else at least 8 cores in all. The three threads run at
  the pace of the slowest, so a helper on an efficiency core slows the orbit below one
  thread; the old 8-core rule admitted 4P+4E iPads and M1/M2 base Macs (review P1, 2026-10-20).
  M1 Pro (8P+2E) keeps the helpers; 4P devices now run one thread. Whether 4 performance
  cores could still gain while the GPU scheduler and the display link run is unmeasured
  (no such device was available; (unverified)).
- It is opt-in (`Threads::Single` is the default): the reference worker and Newton ask
  for it. Callers that already run many orbits at once (the CPU oracle under rayon)
  must not triple their threads: with helpers on by default one core test went from
  4 s to 99 s on an oversubscribed machine. Spinning helpers also lose under load (one
  series at load 85 gained only 1.27x at 1e-1000).

Splitting each product further (column ranges of the short product on 6 threads, with
a carry merge) would cut the critical path by roughly another third at 1e-1000
(estimate, not built): the five additions and the exchange remain.

## Progressive delivery of centre references

With [Rebasing (Zhuoran, 2021)](rebasing.md) a prefix of an orbit is a correct reference for every
pixel that escapes before its end; a pixel that reaches the end rebases to `m = 0`
with a rounding loss and is wrong (rebasing, "Truncated references"). So the frame
drawn with a prefix is provisional, and the reference worker hands prefixes out
before all 2x-limit points exist:

- `ReferenceOrbitComputer::resume` continues a computation from its `final_z`
  exactly (bit-identical to one long call), so the orbit runs in stages.
- `Reference::compute_big_progressive` hands out the prefix of 1/16 of the request
  (1/8 of the view's limit; at least 4096 points), then the prefix of half the request
  (the view's limit itself), then the whole orbit.
- `ReferenceWorker::poll(false)` returns such a prefix while the computation keeps
  running; a prefix shorter than the limit is `Stale` (used now, the pending request
  still serves the view, so nothing is restarted); the limit-length prefix is `Good`.
  Offscreen renders still wait for the whole orbit.
- (2026-10-16) A job drawn with a prefix shorter than its limit polls the worker every
  refresh and renders the view again when an orbit that reaches the limit arrives;
  until then it is not settled and the learner ignores it. Before, nothing polled the
  worker while a job's key stood, and the provisional frame stayed (flat mini-maps
  near `spiral-1e1000`).

A nucleus reference is one period long and is delivered whole.

(2026-10-20) Storage grows with the orbit instead of being reserved for the request: each
stage resizes it, and a centre orbit longer than 2^20 points runs in extra stages that double
it (resuming is exact; the prefixes handed out are the same). The worker uploads every
reference it hands out to the GPU on its own thread, and the points are shared (`Arc`), so
neither the copy nor the table builds touch the orbit on the frame thread; see
[Hang prevention](hang-prevention.md#long-references-as-built-2026-10-20).

## Newton with a cheap derivative

Newton's step `c -= z_p / z_p'` only needs the derivative to a few digits. Computing
`z'` in extended-range f64 ([Extended range: floatexp, rescaled iterations, depth thresholds](extended-range-floats.md)) over the orbit
(which now runs through the orbit code, with fixed-size kernels and three threads)
makes a step ~3x cheaper (dendrite@1e-2000: Newton 1333 -> ~230 ms with the threads). The derivative's relative error turns quadratic convergence
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
  at least 256 limbs (R1, [Arbitrary precision on Apple Silicon (GMP, MPFR)](../platform/mpfr-gmp-on-apple-silicon.md)), and depths to
  1e-3000 need at most ~160 limbs. Toom-3 is asymptotically ~1.3x cheaper than
  Karatsuba but would still have to beat a short product that already does half the
  limb products of a full one; FFT crossovers are in the thousands of limbs.
- Lower precision for part of the orbit: rounding at iteration k propagates like a
  perturbation of C of size `ulp · |Z_k| / |dZ_k/dC|`; early iterations have
  `|dZ_k/dC| ~ 1`, so the full pixel precision is needed from the start. Only the
  guard bits could go, a ~2 % saving.

## See also

[Reference orbit: precision, cost, storage, compression](reference-orbit-precision.md), [Reference selection: nucleus, period detection, Newton](reference-selection.md),
[Rebasing (Zhuoran, 2021)](rebasing.md), [Arbitrary precision: own Swift/C implementation, not GMP/MPFR](../decisions/arbitrary-precision.md).

# Citations

- [FractalShark README](../sources/fractalshark-readme.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
