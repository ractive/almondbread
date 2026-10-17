---
title: "mathr blog: atom domain size, Misiurewicz domains, periodicity scan"
type: source
authors: ["Claude Heiland-Allen"]
year: 2017
raw: "web"
status: digested
tags: [atom-domain, misiurewicz, period-detection, newton, automation]
---

# mathr blog: atom domain size, Misiurewicz domains, periodicity scan

**URLs** (read 2026-10-17, not saved; facts only):
"Atom domain size estimation" (2013-12-10)
<https://mathr.co.uk/blog/2013-12-10_atom_domain_size_estimation.html>,
"Misiurewicz domains" (2015-02-01) <https://mathr.co.uk/blog/2015-02-01_misiurewicz_domains.html>,
"Periodicity scan" (2017-05-17) <https://mathr.co.uk/blog/2017-05-17_periodicity_scan.html>.
The blog's Mandelbrot tag page <https://mathr.co.uk/blog/mandelbrot.html> lists the related
posts (external angles, Newton for periodic and Misiurewicz points, filtered atom domains,
domain coordinates).

## What it covers

- **Atom domain size** of a nucleus `c_0` of period `p`:
  `R_a = |F^q(0, c_0)| / |∂F^p/∂c (0, c_0)|`, with `q < p` the index minimising `|F^q|`.
  Derived by linearising `G = F^p / F^q` at the nucleus; checked visually only.
- **Misiurewicz domains**: for a fixed period `p`, per pixel the index `q` minimising
  `|z_{p+q} − z_q|` (two orbits, `p` apart). The domains surround Misiurewicz points whose
  period divides `p` and are "much larger than a single point", so they locate them.
- **Periodicity scan**: over successively finer grids (from `mingridsize << 8` down), at each
  cell run Munafo's Jordan-curve period test in a box ~4 grid cells wide, refine by Newton
  (≤ 16 steps), re-check the period in a much smaller box ("if the period is wrong, the size
  estimates will be way off"), and record period, nucleus, component size, atom-domain size
  and shape (cardioid or disc). Duplicates are suppressed with a 2D mask; double precision
  only (shallow views).

## Reliability

Primary; the size estimate is first-order and "seem[s] to match up pretty well" by eye.

## Used by

[[synthesis/finding-deep-places]]
