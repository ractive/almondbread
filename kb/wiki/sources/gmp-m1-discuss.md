---
title: "GMP on Apple M1 (gmp-discuss, 2020)"
type: source
authors: ["Torbjörn Granlund"]
year: 2020
raw: "web/gmp-m1-discuss.html"
status: digested
tags: [gmp, apple-silicon, arbitrary-precision, cpu, benchmarks]
---

# GMP on Apple M1 (gmp-discuss, 2020)

**URL:** <https://gmplib.org/list-archives/gmp-discuss/2020-November/006600.html> · **Saved:** `raw/web/gmp-m1-discuss.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

A mailing-list post, "Surprise performance from Apple M1", by GMP's lead developer, 21 November 2020, after the project bought an M1 Mac mini.

## What it covers

- "No other CPU runs GMP this well": almost every GMP inner loop runs at under 1 cycle per limb on M1, including `mpn_mul_1`, before any M1-specific tuning.
- The exception is `mpn_addmul_1`, the most important loop (the core of schoolbook multiplication).
- The 3.2 GHz M1 took the #2 spot in GMPbench; #1 was an AMD Ryzen at 4.4 GHz.
- Why `addmul_1` is slow on ARM64: only one carry flag (x86 has two, via `adcx`/`adox`) and no instruction for the high word of `a·b + c`. The 3-way accumulation needs repeated carry save/restore.
- Comparison: x86 fixed the same problem with `adox`/`adcx`; IBM Power added both a high-word multiply-add and multiple carry flags.

## Reliability

Primary and authoritative (GMP maintainer), but an early impression on base M1 with untuned code from 2020. No numbers beyond "< 1 cycle/limb" and the GMPbench ranking. Later M-series chips and GMP versions may differ.

## Useful sections

- §Surprise performance from Apple M1 (the whole post) — cycles/limb claim, GMPbench rank, the `addmul_1` carry-flag limitation.

## Used by

- [[platform/mpfr-gmp-on-apple-silicon]]
- [[techniques/reference-orbit-precision]]
