---
title: "Does local series approximation hold in the float32 perturbation kernel?"
type: question
status: open
sources: ["[[sources/mathr-deep-zoom-theory]]"]
tags: [series-approximation, float32, dense-regions, verification]
---

# Does local series approximation hold in the float32 perturbation kernel?

## Question

[[techniques/local-series-approximation]] was measured on the CPU in f64 with direct
iteration (R3, [[synthesis/dense-region-sampling]]): 5× fewer iterations at the dense view with
error statistics equal to the exact render. The app iterates float32 deltas against a
reference with rebasing. Does branching `dz = dz_seed + A δ + B δ²/2` in float32 keep the
samples statistically equivalent, and does the gain survive the GPU's costs (seed passes,
state copies, lower occupancy of the seed phase)?

## What would answer it

- A kernel prototype with a seed mode (`kSeed`: carries `B`, writes the branch state per
  quadtree node) and pixel admission from seed states.
- `verify` against the f64 oracle at all benchmark locations: escape-n mismatch, median |Δμ|
  and class mismatch no worse than the plain kernel's (dense view: 30 % escape-n differs,
  median |Δμ| 0.064, [[synthesis/benchmarks]] M4); OKLab ΔE of the default look against a
  converged reference no worse than the plain kernel's.
- Steps executed vs iterations (the existing `steps` counter) and GPU time interleaved with the
  plain pipeline on an idle GPU.

## Risks to check

- `|A δ| < 2⁻²⁴ |dz_seed|` at the branch: neighbours collapse onto one orbit until `+dc`
  separates them again (blocky or streaky results).
- `B` overflow: `|B|` grows like `|A|²`; scaled by the texel size (as `d` is) it stays in range
  at reachable depths; M3b needs the rescaled form.
- Rebasing between the seed's branch and the sample's start: the sample inherits the seed's
  reference index `m`; check rebase at admission.
