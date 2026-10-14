---
title: "Reference selection: nucleus, period detection, Newton"
type: technique
status: draft
sources: ["[[sources/kf-manual]]", "[[sources/muency-newton-raphson]]", "[[sources/muency-atom-domain]]", "[[sources/philthompson-bla]]", "[[sources/fractalshark-readme]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/fraktaler-3-site]]"]
tags: [reference, period-detection, newton, minibrot]
---

# Reference selection: nucleus, period detection, Newton

With [[techniques/rebasing]], any reference near the view works and none
has to outlast the deepest pixel. The best choice is still the nucleus of
the lowest-period minibrot in or near the view: its orbit is exactly
periodic, so only p high-precision iterations are needed, and the skip
tables follow its period structure.

## Options

| Reference | High-precision iterations | Notes |
| --- | --- | --- |
| View centre | up to maxIter | always works with rebasing; slow at high maxIter |
| Minibrot nucleus, period p | p | `Z_p = 0`; wrap at m = p is exact (inference) |
| Misiurewicz (pre-periodic) point | pre-period + period | no source found for using it as the reference for single-critical-point Mandelbrot |

Before rebasing, a reference that escaped early or sat in the wrong place
caused glitches; Zhuoran reported rebasing working at every location he
tried, "including those which don't have a central minibrot"
(src: [[sources/fractalforums-zhuoran-rebasing-bla]] §Reply #2).

## Step 1: find a period

**Atom domain (cheapest).** Iterate the view centre (or the current
reference); the dominant period is the iteration `n ≥ 1` at which `|Z_n|`
reaches a new minimum (Peitgen and Richter 1986)
(src: [[sources/muency-atom-domain]]). Cheap, but it gives the period of the
atom domain containing the point, not necessarily of a minibrot inside the
view.

**Box method (Munafo).** Iterate the corners of a rectangle around the view
and count how often the polygon winds around the origin; the first n where
it encloses 0 is the period of a mu-atom inside (notes:
perturbation_core §5, citing Mu-Ency's Jordan curve page, not saved).

**Ball method (knighty, gerrit).** Interval or ball arithmetic: iterate a
disc of radius r around c and stop at the first n whose image disc contains
0. KF uses it by default since it "works much better than box method in
skewed locations" (src: [[sources/kf-manual]] §kf-2.14.10). Very Plotter uses
gerrit's first-order version; the second-order one was too slow
(src: [[sources/philthompson-bla]] §Finding A Periodic Point). A common
first-order form (unverified, see [[questions/ball-period-recurrence]]):

```text
z = 0; dz = 0; R = 0                 // dz = dz/dc
for n in 1 ... maxPeriod:
    R  = 2*|z|*R + R*R + r           // some variants use R = |dz|*r
    dz = 2*z*dz + 1
    z  = z*z + c
    if |z| < R: return n             // disc image contains 0
```

Run it in enough precision for the view (the disc radius is the view size).

## Step 2: Newton for the nucleus

Solve `z_p(c) = 0`. Both `z_p` and its derivative come from ordinary
iteration, so polynomials are never expanded; an approximate starting point
is enough (src: [[sources/muency-newton-raphson]]).

```text
repeat:
    z = 0; dz = 0
    for i in 1 ... p:  dz = 2*z*dz + 1;  z = z*z + c
    step = z / dz;  c = c - step
until |step| < target (e.g. 1e-3 of the pixel spacing)
```

- Quadratic convergence: about log2(bits) iterations, each costing p
  high-precision iterations (inference). Raising precision as the step
  shrinks is a common optimisation (inference).
- Newton-Raphson zooming: the nucleus is "exact to within the precision
  being used"; the minibrot size follows from the first p − 1 iterates
  (notes: perturbation_core §5, citing Mu-Ency's NR zooming page).
- KF's Newton actions: Period, Center, Size, Skew; it "calculates a new
  reference after each zoom" (src: [[sources/kf-manual]] §Newton-Raphson Zooming).
- Very Plotter loops: find the dominant period, Newton to the minibrot, and
  if it is off screen repeat with a deeper, higher-period one
  (src: [[sources/philthompson-bla]] §Finding A Periodic Point).
- FractalShark's "Feature Finder" refines periodic points with
  high-precision Newton, and its reference periodicity detection comes from
  Imagina (src: [[sources/fractalshark-readme]]).

## Using the nucleus

- Store p + 1 values (`Z_0 ... Z_p`, with `Z_p = 0`); rebasing on
  exhaustion makes the orbit effectively infinite (inference).
- Fraktaler 3: locking maximum reference iterations to a known period "can
  give a good speedup for high iteration areas" (src: [[sources/fraktaler-3-site]]).
- Keep `|c|` small: a nucleus far outside the frame costs float32 sub-pixel
  bits and shrinks BLA radii through `max|c|` (inference).
- Hybrids and multiple critical points need one reference per phase and per
  critical point (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing).

## Misiurewicz points

Pre-periodic points matter for references in hybrids with pre-periodic
parts (src: [[sources/mathr-deep-zoom-theory-again]] §Rebasing). No source
found for choosing a Misiurewicz point as the main reference for plain
Mandelbrot; with rebasing there is little reason to (inference).

## For almondbread

Roadmap step 7: atom-domain period from the current reference, ball-method
confirmation in MPFR, Newton to the nucleus with doubling precision, then
use it if it lies within a few view widths; otherwise fall back to the view
centre. Recompute only when the view leaves the region the reference
serves ([[techniques/frame-reuse-and-progressive]]).

## In almondbread (milestone 3a, measured)

`PeriodDetection`, `NucleusFinder` and `ReferenceSelector` in `Sources/AlmondbreadMath`
(Swift; ported bit-exact to `crates/almondbread-math/src/selection.rs` in R1, the Swift
code deleted in R2b):

- **Period detection**: rigorous ball (`R' = 2|z|R + R² + r`, test `|z_n| < R_n`),
  first-order ball (`R_n = |dz_n/dc| r`) and box (corner quadrilateral winds around 0),
  all with z at full precision and R, dz in double + int64 exponent. Agreement with an
  independent mpmath implementation of the rigorous ball on 8 locations. They disagree
  with each other at depth: around the seahorse point with r = 1e-10 the rigorous ball
  says 973, first-order and box 998; with r = 9e-36 near the period-8007 minibrot the
  rigorous ball says 96084 (= 12 × 8007), first-order 280245, box none (measured
  2026-10-14). See [[questions/ball-period-recurrence]].
- **Newton**: z and dz/dc iterated p times at full precision on preallocated registers,
  `c -= z/dz`, stop when the step is within 16 bits of the working precision or stops
  shrinking. Converges in 3 to 11 steps on all tested locations; nuclei of periods 3, 4,
  8007 and 60/120/250 (real axis near -2, sizes 3e-71, 2e-143, 6e-300 at 512/1024/2048
  bits) match mpmath to within 1 bit of the precision (measured 2026-10-14, M1 Pro).
  Time: 20 ms for period 8007 at 192 bits, 7 ms for period 250 at 2048 bits.
- **Pitfall**: a period from the ball method belongs to an atom that touches the disc,
  not necessarily one whose nucleus lies in it. Newton for high periods then lands on a
  far nucleus, often one whose period divides p (z_kp = 0 there too): from a start
  1e-36 off the period-8007 nucleus, periods 11·8007 and 13·8007 both converged back to
  it. So the selector checks the result: nucleus within the view radius, else the view
  centre (with the detected period reported).
- **Size estimate**: `1/(b l²)` with `l = Π 2 z_i`, `b = 1 + Σ 1/l_i` over
  i = 1 ..< p (unverified; mathr's mandelbrot-numerics formula from memory). Checks:
  period 2 gives 0.5, the period-3 minibrot 0.0190 (its cardioid spans about 0.0185 on
  the real axis, unverified), seahorse period 8007 gives 8.9e-33. The complex value also gives the
  minibrot's rotation, which the deep-zoom test uses to step into minibrots of minibrots.
