---
title: "Does local series approximation hold in the float32 perturbation kernel?"
type: question
status: answered
sources: ["sources/mathr-deep-zoom-theory"]
tags: [series-approximation, float32, dense-regions, verification, measured]
---

# Does local series approximation hold in the float32 perturbation kernel?

## Question

[Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md) was measured on the CPU in f64 with direct
iteration (R3, [Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md)): 5× fewer iterations at the dense view with
error statistics equal to the exact render. The app iterates float32 deltas against a
reference with rebasing. Does branching `dz = dz_seed + A δ + B δ²/2` in float32 keep the
samples statistically equivalent, and does the gain survive the GPU's costs (seed passes,
state copies, lower occupancy of the seed phase)?

## Answer (R5, measured 2026-10-14, M1 Pro, release, GPU shared with other agents)

**Yes in float32, and yes on the GPU where the orbits are coherent; no gain where they are
not.** A Metal prototype (quadtree seeds 32 → 2 texels, ε = 1e−2, float32 deltas with the
production perturbation and rebasing, floatexp seeds in the deep kernel) gives at 1600 × 1000
(full numbers: [almondbread benchmarks and verification record](../synthesis/benchmarks.md) R5):

| view | steps plain → SA | GPU ms plain → SA (median of 9, interleaved) |
|---|---|---|
| dense-5e-14 | 4.61e9 → 5.29e8 (8.7×) | 78 → 15.6 (5.0×) |
| deep-5e-14 | 8.9× | 49 → 10.6 (4.6×) |
| neck-2e5 / root-third | 12.4× / 5.6× | 147 → 49 / 111 → 34 (3.0×, 3.2×) |
| filament-1e6 | 5.6× | 19 → 10 (1.9×) |
| sea-1e20 (deep, BLA on both) | 17× (steps after BLA) | 293 → 34 (8.5×) |
| needle-1e305 / dendrite-1e300 (deep) | 1.3× / 6.8× | 1.2× / 1.9× |
| interior- or unresolved-heavy (home, elephant, minibrot-p4, cusp, Feigenbaum, minibrots-5e-12, sea-8007) | 1.03–1.6× | 0.75–1.07× (slower or equal) |

So the 5× of R3 survives, and grows at the 1× limit (R3 ran to 4× the limit, where the
unshared tail is longer): 8.7× fewer steps and 5× less GPU time at the dense view.

**Correctness.** Against the CPU oracle (800 × 500 crops at the 1600-wide pixel size; 1600 ×
1000 for the needle) the SA render's class mismatch equals the plain kernel's at all 18
locations, with **zero GPU-interior/CPU-exterior pixels**. In chaotic views escape-n
differences and median |Δμ| equal the plain kernel's own float32 error (dense 16.0 → 16.9 %,
median 0.0109 → 0.0123; deep-5e-14 12.8 → 12.7 %; Feigenbaum 62.2 → 62.2 %; sea-1e20 9.9 →
10.1 %). In smooth views the truncation shows as a larger but still invisible μ error:
median |Δμ| 1.4e−6 → 1.8e−5 (elephant), 1.5e−5 → 4.9e−5 (filament), escape-n differs 3.9 →
5.8 % (elephant), 6.2 → 8.0 % (seahorse): boundary pixels whose escape count flips. Images
(default look, 100 % and 300 % crops at nine views): no tile seams, no wrong-coloured patches,
no missing minibrots; plain-vs-SA differences are single boundary pixels (0.2–0.6 % of pixels
differ by more than 3 % in colour; 0.44 % at sea-1e20 after the deep DE fix, 3.1 % before it, where the plain kernel's own DE bugs differed).

## Where it breaks

1. **Interior-test derivative restarted at the branch: wrong.** The R3 design restarted the
   interior test's `w = dz_n/dz_m` at the branch. At the deep needle (period-255 nucleus
   reference) a restarted w meets the next passage of the orbit near 0, where `|2z| ~ 1e−300`,
   without the earlier growth that balances it: **96 % of the image painted interior** (55 %
   at 400 × 250). Fix: carry `w` and its derivative `wd = dw/dc · texel`
   (`wd ← 2 (A w + z wd)`) through the seeds and start each sample at `w + wd δ`. After the
   fix: 0 false interior everywhere. The R3 CPU study did not hit this (no needle view).
2. **Pixel-phase throughput.** Samples start at their leaf's reference index; a SIMD group
   holds 8 leaves with unrelated `m`, so reference reads lose coherence (the M2 compaction
   lesson). Where SA saves little (interior and unresolved pixels iterate to the limit
   anyway), the kernel is slower than plain: Feigenbaum 0.75× (3 campaigns: 0.51–0.83×),
   cusp-1e5 0.9× at 1.56× fewer steps, home-100k 0.84×.
3. **Seed passes at low occupancy.** The root level has 1 600 threads (1/1024 of the
   pixels); where seeds run long (unresolved, parabolic views: they run to the limit),
   the seed passes dominate: neck-7e6 33 of 44 ms, cusp 37 ms, Feigenbaum 188 ms. In dense
   views they cost 1–3 ms (seed steps 6–9 % of the total).
4. **Short orbits.** When pixels need only ~100 iterations after the branch, fixed costs per
   pixel (admission, state, output) dominate: filament 5.6× fewer steps but 1.9× GPU.
5. **Deep views need BLA in the seeds.** Without it the floatexp seeds iterate what BLA would
   skip: needle-1e305 1.08× fewer steps and slower than plain, sea-8007 0.34×. With BLA blocks
   in the seeds (`a' = A a + B texel`, `b' = A b`, `w' = A w`, validity tested at
   `|dz| + |a| r (1+ε)` and `|c| + texel r`) needle reaches 1.3–8× and sea-1e20 17× fewer steps.
6. **Minibrot-rich and fold-heavy views** (minibrots-5e-12, needle, sea-8007): seeds branch at
   the first fold (root seeds at n ≈ 8 near the period-7 cascade) and most iterations are in
   unresolved minibrot halos: 1.1–1.3× fewer steps, no time gain.

## Risks checked

- `|A δ| < 2⁻²⁴ |dz_seed|` (neighbours collapsing): not observed. A child's offset `A δ` is
  relative to the same reference as the seed's delta, so it carries the same float32
  resolution as a plain pixel's delta; no streaks or blocks in any crop.
- `B` range: plain seeds store `b · texel` (finite at all reachable plain depths); deep seeds
  are floatexp.
- Rebasing between branch and admission: children and pixels re-test `|Z_m + dz| < |dz|` at
  admission; no artefact.

Recommendation and the M5 integration design: [Local series approximation (tile and pixel seeds)](../techniques/local-series-approximation.md)
"GPU prototype (R5)".

# Citations

- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
