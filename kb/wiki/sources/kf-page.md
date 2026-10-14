---
title: "Kalles Fraktaler 2 + (home page)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2022
raw: "web/kf-page.html"
status: digested
tags: [kalles-fraktaler, implementations, bla, benchmarks, licence]
---

# Kalles Fraktaler 2 + (home page)

**URL:** <https://mathr.co.uk/kf/kf.html> · **Saved:** `raw/web/kf-page.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Home page of Kalles Fraktaler 2+ (KF2+), the fork of Karl Runmo's Kalles Fraktaler maintained by
Claude Heiland-Allen: status, limits, downloads, source, benchmarks and a list of alternative
deep-zoom renderers with timings.

## What it covers

- KF uses perturbation and series approximation; the fork replaced the custom bignum code with GMP/MPFR and is cross-compiled to Windows with MINGW64.
- Development "mostly stopped"; BLA, described as "much better than series approximation (SA) (including NanoMB1 and NanoMB2)", is unlikely to be added because KF has over 100 formulas.
- Alternatives with BLA, timed on a 16-thread AMD Ryzen 7 2700X for one minibrot at `5e-433`: fractalshades just over 2 min vs about 20 min in KF with NanoMB1; Fractal Zoomer 30 s with perturbation/BLA plus boundary tracing (just over 2 min without); Fraktaler 3 just over 2 min, also has OpenCL. Also lists Imagina, FractalShark (NVIDIA GPUs), m-perturbator-gtk.
- Limits: Windows bitmap 2 GB (~715 Mpixel) and 64k max dimension; iteration count up to 9 exa-iterations; periods up to 2 giga-iterations; MPFR precision up to 2 giga-bits; up to ~50 bytes per pixel and ~40 bytes per reference iteration; max 64 CPU threads.
- Latest release kf-2.15.5.2 (2022-06-07); known bug: crash past `1e291` in some fractals, workaround disable Rescaled Double.
- Licence: binaries under AGPLv3+; source at code.mathr.co.uk/kalles-fraktaler-2.git (branches kf-2.12 ... kf-2.15).
- Benchmarks: 6 power-2 Mandelbrot locations, double / scaled double / long double; old WINE timings only (2.11.1 to 2.13.4); no data yet for scaled long double (deeper than ~`1e4900`) or floatexp (deeper than ~`1e9800`).

## Reliability

Primary, by the maintainer. Timings are single-location anecdotes on one machine, not a benchmark
suite. The alternatives list dates from about 2022 and may be stale; see the Fraktaler 3 site for a
newer list.

## Useful sections

- §Future — KF frozen, BLA vs SA judgement, alternatives with `5e-433` timings.
- §Limits — hard limits and memory per pixel / per reference iteration.
- §Download — release dates, the `1e291` Rescaled Double crash, AGPL note.
- §Source code — git branches.
- §Benchmarks — number-type depth ranges, old timings.

## Used by

[[implementations/kalles-fraktaler-2]], [[implementations/fractalshades]], [[implementations/fractal-zoomer]],
[[implementations/fraktaler-3]], [[implementations/imagina]], [[implementations/fractalshark]],
[[techniques/bla]], [[techniques/series-approximation]], [[questions/kf-bla-support]]
