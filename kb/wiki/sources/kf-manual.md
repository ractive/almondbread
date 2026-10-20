---
title: "Kalles Fraktaler 2 + manual"
type: source
authors: ["Claude Heiland-Allen", "Karl Runmo"]
year: 2022
raw: "web/kf-manual.html"
status: skimmed
tags: [kalles-fraktaler, perturbation, glitch-detection, rebasing, extended-range-floats, series-approximation, newton-raphson, guessing, exponential-map, opencl, interior-detection]
---

# Kalles Fraktaler 2 + manual

**URL:** <https://mathr.co.uk/kf/manual.html> · **Saved:** `raw/web/kf-manual.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

The full KF2+ manual (about 160 KB of text): limits, known bugs, a change log from 2017 to
kf-2.15.5.2 (2022), TODO, build notes, a menu-by-menu user manual, the GLSL colouring API, formula
list and the EXR channel spec. Skimmed: read Limits, Known Bugs, Differences, the 2.15.x change
logs and keyword hits in older ones, TODO, and the user-manual sections on View, Navigation,
Advanced, Formula, Bailout, colouring DE, GLSL pixel API, command line and EXR channels. The formula
list and build sections were not read.

## What it covers

- **Number types and depth ranges.** single float until about zoom `1e20` (since 2.15.3, off by default because of undetected glitches); double with SIMD up to about `1e300`; scaled double for Mandelbrot power 2 to about `1e600`, power 3 to about `1e400` (2.14.8); x87 long double to about `1e4900`; scaled long double for `1e4900`-`1e9800` (2.12.9); floatexp beyond. 2.15.3 added rescaled perturbation (single or double, with or without derivatives and OpenCL) for Mandelbrot p2, p3, Burning Ship p2 and hybrids, usually faster than long double and floatexp, plus single-precision floatexp. A Number Type Selection dialog limits which types KF may choose; types near the top are faster.
- **OpenCL.** Used for perturbation iterations only; fp64 detected automatically but GPUs are usually much faster in fp32. Between ~`1e300` and ~`1e4900` the CPU is likely faster because OpenCL has no x87 long double and must fall back to floatexp. For hybrids OpenCL is much faster because branching is resolved at kernel compile time. Users may need to change the OS GPU timeout. Experimental OpenCL arrived in 2.15.2.4 (2021).
- **Glitch handling options.** Glitch tolerance (0..1, fractional since 2.15.3; higher detects more, 1 = good but slow); "glitch low tolerance"; auto solve glitches with up to 10000 references (default and hard limit); reference selection methods; "near pixel" method re-renders only connected pixels; "ignore isolated small glitches" interpolates single-pixel glitches from neighbours; derivative glitch test (power 2 Mandelbrot only, not in OpenCL); "reference strict zero" for rescaled iterations (needed near the Burning Ship needle; default on for non-Mandelbrot formulas).
- **Rebasing ("single reference").** kf-2.15.5 (2021-12-05) added Zhuoran's single-reference method, enabled by default, also in NanoMB1. Known issues: only simple hybrids work, may fail with more than one critical point; 2.15.5.1 fixed it with rescaled calculations. Known bug: hybrids broken with single reference method because they need references at each phase of the loop.
- **Series approximation.** SA tolerance (0..1), approximation terms automatic by pixel count, rescaled SA for power 2 Mandelbrot (about 30% faster than all-floatexp, 2.15.3). NanoMB1 / NanoMB2 (knighty's bivariate super-series, power 2 only) always use floatexp; period must be set first (Newton zoom sets it); NanoMB2 disables glitch detection. TODO lists automatic and rescaled number types for NanoMB. Burning Ship SA stops at the first fold.
- **Interior checking** exists only for NanoMB1/2 (experimental). Cited elsewhere as making a deep minibrot render feasible ([fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](fractalforums-interior-detection.md)).
- **Newton-Raphson zooming.** Actions Period, Center, Size, Skew (each includes previous); Ball method by default, Box method otherwise. Modes: Minibrot relative (fold factor: 0.5 doubles the pattern for power 2, 0.75 quadruples; power `P` uses `1-(1/P)^M`), Minibrot absolute (0.75 near an embedded Julia set, 1 at the minibrot), Atom Domain absolute (1 at the atom domain, 1.125 sometimes near a Julia morphing). Recommended zoom size 128 for the search box. Fails fast if `C` leaves the target (2.12.9 fix suggested by knighty). Known bugs: presets bad for powers other than 2, poor in skewed locations.
- **Auto iterations** on by default; known bug: zoom-out sequences can get black regions because maximum iterations are reduced too much (workaround: disable). The manual gives no algorithm.
- **Guessing.** Interpolates neighbour pixel data when iteration counts match, speeding interior and glitch regions; only interpolates when neighbours share glitch status (2.12.8). Off in the Best preset, on in Fast. Known bugs: race condition with progressive interlacing; with OpenCL on GPU only about 1.5x instead of 4x speedup; needs even image dimensions with OpenCL; poor in exterior.
- **SIMD.** Vector size default 2, chunk size 64 iterations between escape/glitch checks; the last chunk is replayed per pixel after rollback.
- **Jitter.** Non-zero seed enables per-pixel position jitter (uniform default since 2.12.11, Gaussian option, scale in pixels, 1 is sensible); stacking jittered renders is an alternative to supersampling; Best preset enables seed 1. Analytic DE recommended for slopes when jitter is on.
- **Exponential map** (2.15.1): use a wide aspect ratio (9:1, window 1152x128 or 1536x170) and zoom size 2 for zoomasm; much more efficient than flat keyframes.
- **Bailout.** Smooth method Log (renormalised) or Linear; escape radius High / 2 / Low bailout / Custom; norm power with `pow(|b_x |z_x|^p + b_y |z_y|^p|, 1/p)`.
- **Distance estimation** via numerical differencing of smooth iteration (Traditional, Forward 3x3, Central 3x3, Diagonal 2x2, Least Squares 2x2/3x3, Laplacian 3x3) or Analytic (needs derivatives).
- **Triangle Inequality Average** colouring (2.15.5) requires OpenCL and disables SA; stored in the phase channel.
- **GLSL colouring** has a `float49` type (49 bits) for iteration counts because float has only 24 bits; `N = (N1 << 32) + N0 + (1 - NF)`.
- **EXR channels:** uint32 `N` (0xFFFFFFFF = non-escaped, 0 = glitch / no data) with `IterationsBias`, split `N0`/`N1` above 32 bits, float `NF` in [0,1), phase `T` in turns, directional DE `DEX`, `DEY` normalised so a neighbouring boundary pixel is distance ~1.
- **Limits:** iterations up to 9 exa (64-bit since 2.14.8), periods 2 giga, MPFR 2 giga-bits, ~50 bytes/pixel, ~40 bytes/reference iteration, max 64 threads.
- Mentions Mandel Machine as a fast Windows renderer "written in Java and assembly".

## Reliability

Primary, by the maintainer, very detailed about the program's behaviour. Depth thresholds are
approximate ("or so", "about"). The change log documents many past bugs at number-type transitions
(e.g. corrupt image near `e600` fixed in 2.13.10), which is itself useful evidence. KF is frozen
since 2022 and never got BLA, so its performance advice reflects SA-era practice.

## Useful sections

- §Limits — hard limits, memory per pixel and per reference iteration.
- §Known Bugs — single reference with hybrids, auto-iterations black regions, guessing races, Rescaled Double slow spots, NanoMB limits.
- §kf-2.15.5 — single-reference (rebasing) introduction and caveats.
- §kf-2.15.3 — single float to `e20`, single floatexp, rescaled perturbation, number type dialog, deep needle workaround.
- §kf-2.14.8 — SIMD and depth ranges for double / scaled double.
- §kf-2.12.9 — scaled long double `e4900`-`e9800`.
- §TODO — open ideas (store one period of the reference, NanoMB number types).
- §Exponential Map — aspect ratio and zoomasm workflow.
- §Newton-Raphson zooming — ball/box methods, relative/absolute/atom-domain modes.
- §Perturbation and Series Approximation Tuning — glitch tolerance, derivative glitch test, reference strict zero, rescaled SA.
- §Number Type Selection — which types exist; single precision warning.
- §Use NanoMB1 (experimental) / §Interior checking (NanoMB only)
- §Use auto iterations / §Use guessing
- §SIMD chunk size — chunked escape checks with rollback.
- §OpenCL — fp32 vs fp64, long double gap, kernel-time branch elimination for hybrids.
- §Jitter seed / §Jitter scale — position jitter.
- §Bailout dialog — smooth methods and norm power.
- §Distance estimation dropdown — numerical DE stencils.
- §float49 numbers — high-precision iteration counts in GLSL.
- §EXR Channels and Metadata — raw-data format shared with zoomasm and Fraktaler 3.
- §Presets — what Fast and Best change.

## Used by

[Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md), [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md), [Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](../techniques/glitch-detection.md),
[Rebasing (Zhuoran, 2021)](../techniques/rebasing.md), [Series approximation (legacy)](../techniques/series-approximation.md), [Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md),
[Guessing, border tracing and rectangle subdivision](../techniques/guessing-and-subdivision.md), [Automatic maximum-iteration control](../techniques/auto-iterations.md), [Exponential map (log-polar) rendering](../techniques/exponential-map.md),
[Anti-aliasing and supersampling](../techniques/anti-aliasing.md), [Distance estimation](../techniques/distance-estimation.md), [Smooth (continuous) iteration count](../techniques/smooth-coloring.md),
[Colouring methods](../techniques/coloring-methods.md), [Interior detection](../techniques/interior-detection.md), [GPU watchdog and chunked, resumable kernels](../platform/gpu-watchdog-and-chunking.md),
[Does Kalles Fraktaler 2+ implement BLA?](../questions/kf-bla-support.md), [What auto-maxIter rule should almondbread use?](../questions/auto-iteration-heuristic.md), [What exactly is the ball-method period recurrence?](../questions/ball-period-recurrence.md),
[Mandel Machine](../implementations/mandel-machine.md), [Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)
