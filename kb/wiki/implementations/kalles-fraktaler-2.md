---
title: "Kalles Fraktaler 2 +"
type: implementation
status: draft
license: "AGPL-3.0-or-later"
sources: ["[[sources/kf-page]]", "[[sources/kf-manual]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/muency-bla]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/fractalforums-interior-detection]]"]
tags: [implementation, perturbation, series-approximation, windows, opencl]
---

# Kalles Fraktaler 2 +

Karl Runmo's Kalles Fraktaler 2, forked by Claude Heiland-Allen (mathr), who
replaced the custom bignum with optimised libraries (GMP/MPFR) and added many
features. Windows binaries cross-compiled from Linux; runs in Wine
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)). The renderer that industrialised perturbation.

## Algorithms

- Perturbation plus series approximation (SA) with probe points; knighty's
  NanoMB1 (biseries, skips whole periods near a minibrot) and the
  experimental NanoMB2 (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md);
  [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Series Approximation).
- Glitches: Pauldelbrot detection (2-norm), multi-reference correction with
  "Original", "argmin|z|" and "Random" reference choice, max 10000 references;
  since 2.15.5 Zhuoran's single-reference rebasing is on by default, with the
  caveat that it does not work with every hybrid and "does not cure all
  glitches" (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)). mathr prototyped it in KF in Nov 2021:
  1 reference rendered Dinkydau's Flake in 18.5 s versus several minutes for
  97 references, with smaller image differences than the usual 22-reference
  setting (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #9).
- Newton-Raphson zooming to minibrots; ball-period method default, box method
  alternative (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Rescaled perturbation (2.15.3): "arbitrarily deep" for Mandelbrot powers 2
  and 3, Burning Ship power 2 and hybrids (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Guessing (interpolate when neighbours have equal counts), Adam7-style
  progressive interlacing, uniform jitter AA, exponential map (2.15.1),
  auto iterations (on by default, undocumented algorithm), GLSL colouring
  (Ctrl-G), hybrid designer (Ctrl-H) (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md),
  [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Number types and GPU

- float (shallow, to ~1e20, off by default: "undetected glitches at some
  locations"), double (to ~1e300), scaled double (~1e600 power 2, ~1e400
  power 3), long double (x87), scaled long double (beyond ~1e4900), floatexp
  (beyond ~1e9800) (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md); [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).
- OpenCL, including single-precision OpenCL for devices without fp64 (2.15.3).
  Between ~1e300 and ~1e4900 the CPU beats OpenCL because OpenCL lacks x87
  long double (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- Limits: MPFR precision 2 gigabits, ~40 bytes per reference iteration, ~50
  bytes per pixel, 64 CPU threads, periods up to 2 giga-iterations
  (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Formulas

Mandelbrot powers 2-10, Burning Ship, Buffalo, Celtic, Mandelbar,
perpendicular variants, many cubic-quintic hybrids, SimonBrot,
TheRedshiftRider, HPDZ Buffalo; user hybrids via stanzas
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)). See [Hybrid formulas](../formulas/hybrids.md).

## Performance (attributed)

- Minibrot at 5e-433: about 20 min with NanoMB1 on a 16-thread Ryzen 7 2700X,
  versus about 2 min for fractalshades and Fraktaler 3 and 30 s for Fractal
  Zoomer with BLA plus boundary tracing (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).
- Dinkydau's Flake 2400x2400: KF 2.15.5.2 CPU 7.5 s (SA), OpenCL 6.6 s;
  mathr's new BLA program 5 s CPU, 3 s OpenCL; fractalshades 14 s
  (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md) §Reply #9).

## Status

Latest kf-2.15.5.2 (2022-06-07). "Development is mostly stopped"; BLA is
"unlikely to be implemented" because porting it to over 100 formulas is too
much work. Known bug: crash past 1e291 in some formulas (workaround: disable
Rescaled Double). The page points users to Imagina, fractalshades, Fractal
Zoomer, FractalShark and Fraktaler 3 (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).

## Contradictions

- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md) (mathr, the maintainer): BLA is "much better" than SA
  and is unlikely to be added to KF. [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md): "newer versions
  of Kalles Fraktaler" use BLA. The maintainer's statement is more
  authoritative; Mu-Ency may mean the experimental cross-platform KF fork or
  mathr's successor program. See [Does Kalles Fraktaler 2+ implement BLA?](../questions/kf-bla-support.md).

## What almondbread can learn

Algorithms only (AGPL code must not be copied): the number-type ladder with
depth thresholds, rescaled iterations, EXR raw-data export compatible with
zoomasm, GLSL-style user colouring, exponential-map export. Avoid SA,
NanoMB and multi-reference glitch correction
([Series approximation (legacy)](../techniques/series-approximation.md), [Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](../techniques/glitch-detection.md)).

# Citations

- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
