# Dense-region sampling: experiment notes (R3)

Date: 2026-10-14. Machine: M1 Pro (10 cores), macOS 26, release build. CPU only; the GPU was
shared with another agent's measurements, so no GPU timing and no interaction test. Code:
`prototypes/dense-sampling/` (README there). Raw outputs: `runs/<location>.txt` (full study,
12-frame pan) and `runs/variance-<location>.txt` (static strategies plus the variance-stopped
AA rows added after the first seven locations had run; the other rows are identical to the
full runs because everything is deterministic).

## Setup

- Sample = direct f64 iteration from 0 (z, dz/dc, dz/dz1), bailout 2^16, smooth offset
  1 − log2(ln|z|), DE = 2|z|ln|z| / |dz/dc| in output pixels, normal angle arg(z conj(dz/dc)),
  derivative interior test every 8 iterations (|w| n² < 1e−3, w rescaled by 2^60 as in the
  kernel). Limit 4 × the location's (the app's ×4 extension passes).
- Colour = port of the kernel's `shade` for the default look (Aurora, palette size 1024, mip
  chain, palette filter, outline 0.7 at 1.6 px, shading 0.75, light 135° height 1.4, specular
  0.35, shininess 28). Unresolved → edge colour lifted 0.5 % to grey; interior → interior
  colour. ΔE = 100 × Euclidean OKLab distance of colours clamped to [0, 1].
- Lattice: 5 × 5 samples per output pixel (offsets −0.4 … 0.4 px), 800 × 500 output pixels at
  the location's 1600-wide pixel size (central crop), extended for a pan of (6, 3) lattice
  steps = (1.2, 0.6) px per frame over 12 frames (Feigenbaum: 400 × 250, 6 frames, 460 s of
  lattice computation; the others 20–60 s).
- Reference: mean of the 16 lattice samples per pixel outside the {0, 2, 4}² grid (i.e. not
  the centre and not FL's 3 × 3 AA positions). Reference noise from two 8-sample halves:
  dense view mean 2.37 → the 16-sample reference about 1.7.

## Chronology and dead ends

1. First run used the full 25-sample box as reference. Any strategy that uses the 3 × 3 grid
   shares 9 samples with it; a fresh re-draw at the same positions (branching AA samples off
   the pixel orbit after 28 iterations, ε = 1e−9, i.e. rounding-level change only) looked 15 %
   worse (ΔE 1.53 vs 1.34 at 400 × 250). Switched to the independent 16-sample reference;
   with it the fresh re-draw and the exact AA agree to two decimals.
2. "Texture" via footprint: first classified a sample as texture when its footprint
   |dz/dc|·pixel reached T before escape. Every escaping sample does (at escape the footprint
   is ≈ 1.5e6 / DE_px), so the class was 100 % of pixels and the strategy painted everything
   with the local mean (ΔE 8.9). Recorded the cut's iteration share instead: at T = 1e−3 a cut
   keeps 96.9 % of iterations at the dense view (98.3 % at 1e−2, 99.1 % at 0.1, 99.5 % at 1).
3. Sparse sampling with texture fill (corners of a g × g cell all exterior with DE < dmax →
   fill with a Gaussian-weighted mean of computed texture samples): dense view texture pixels
   ΔE 3.8–4.2 (σ 3: 4.1–4.2, σ 6: 3.8–4.0; an earlier probe-based variant on a 200 × 125
   test crop: σ 3 3.6, σ 12 2.8 overall but blotchier); the converged
   grey has radial fibres and small flowers at 1–2 px. Gallery crops show both losses.
4. Flat tile series approximation: tiles 4–32 px all branched at ~10 % of the orbit at the
   minibrot view 100× shallower (a fold: the orbit passes near 0 inside the tile); savings 8 %.
   The quadtree (children re-seed from the parent's branch state) recovered 2× there and
   raised the dense view from 3.6× (flat 32, ε 1e−2) to 5.2×.
5. ε: 1e−3 vs 1e−2 make no visible or statistical difference anywhere; 1e−3 makes child seeds
   fail on their first step (no gain past the parent). ε = 1e−9 is the rounding control.

## Probe: exact vs branched samples (dense view, AA offsets of six pixels)

Even a branch after 28 iterations (ε = 1e−9) changes escape counts by ±1 … ±60 and DE by 2×
on many samples (DE ~0.001–0.005 px: structure 1000× below the pixel): the samples are
chaotic at the rounding level. Only statistics and images are meaningful there.

## Main numbers (iterations relative to exact 1 spp; ΔE mean / p99 vs reference)

See `runs/`. Summary table in the wiki synthesis page `synthesis/dense-region-sampling`.

- dense-5e-14: exact 1 spp 5.41 / 18.1; FL AA 8.94×, 1.96 / 7.1; quadtree SA 1 spp 0.194×,
  5.41 / 18.2; quadtree SA + AA 1.244×, 1.96 / 7.1; AA from pixel seeds only 2.05×;
  colour guessing τ 10 0.73×; sparse g4 + texture + AA 2.91×, 3.23 / 17.2; variance-stopped AA
  τ 1.5 8.12×, 2.16; footprint cut keeps 96.9 %. Seed-mode iterations 5 % of the total.
- Pixel branch point (quadtree 32, ε 1e−2): 83.8 % of n at the dense view, 65.6 % at
  minibrots-5e-12 (400 × 250 test), 43.8 % at elephant, 37.7 % at minibrot-p4.
- Interior pixels gain nothing from branching: the branched sample restarts its interior
  derivative at 1 and needs the same contraction to fire.
- Mariani–Silver interior-only fill: 0 wrongly filled pixels at all 10 locations.

## Visual checks (crops at 200–300 %, looked at)

- dense: ref, FL AA and quadtree SA + AA indistinguishable; sparse g4 loses small flowers and
  turns the fibred grey into blotches.
- seahorse: SA + AA identical to FL AA including the minibrot and its satellites; sparse fill
  flattens the grey lace around the spirals.
- Feigenbaum: SA + AA keeps the one-pixel minibrots on the spike; sparse fill erases them and
  is blotchy, although its ΔE (1.28) beats FL AA (1.59): the reference is itself a 16-sample
  mean of a structureless haze, so smoothing scores well.
- 1 spp tile SA vs exact 1 spp at the dense view: different noise realisation, same look.
