---
title: "Fraktaler 3 (documentation site, v3.1)"
type: source
authors: ["Claude Heiland-Allen"]
year: 2025
raw: "web/fraktaler-3-site.html"
status: digested
tags: [fraktaler-3, bla, rebasing, opencl, extended-range-floats, exponential-map, hybrids, gpu-timeouts]
---

# Fraktaler 3 (documentation site, v3.1)

**URL:** <https://fraktaler.mathr.co.uk/> · **Saved:** `raw/web/fraktaler-3-site.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

The manual of Fraktaler 3 version 3.1 (2025-12-15): a cross-platform (Linux, Windows, Android, Web)
renderer for "fast deep zooming of hybrid escape-time 2D fractals", successor to KF2+, using
perturbation, rebasing and BLA. Covers running, the GUI windows, all parameters with defaults,
build, alternatives and version history. Licence AGPL-3.0.

## What it covers

- Backends: regular CPU code and OpenCL (CPU and GPU), usable at once. A "wisdom" file stores measured speed of each (device, number type) plus precision, range and hardware grouping; F3 uses it to pick the number type and devices per location. Benchmarking takes a couple of minutes per device; speeds shown in log2 units. Without wisdom OpenCL is not used.
- OpenCL on CPU is usually faster than the plain CPU code, except roughly `1e300`-`1e4920` where x86 CPU code can use long double.
- Number types added in 3.1: `doubleexp` (double-precision floatexp, for when single floatexp is not precise enough) and `float128`. Single-precision float is a "low range type"; disabling it in wisdom is a workaround for iteration-band glitches with high powers (1.1).
- Formula model: lines with `|X|`, `|Y|`, `-X`, `-Y`, power `P`; Mandelbrot default, Burning Ship = both abs, Mandelbar = `-Y`. Multiple lines iterate interleaved (hybrids); N lines need N times the reference work and memory. Advanced mode exposes opcodes `store, sqr, mul, absx, absy, negx, negy, rot, add` (add C implicit, last). Version 2 rewrote the engine on this opcode model.
- Bailout parameters (defaults): `iterations = 1024`, `maximum_reference_iterations = 1024` (normally equal to iterations; too low corrupts images), `maximum_perturb_iterations = 1024` ("a few 1000" usually; raise for blobby spiral centres or soft minis), `maximum_bla_steps = 1024` ("a few 1000"; too low gives glitchy images), `escape_radius = 625`, `inscape_radius = 9.765625e-4` (= 1/1024). Escape radius auto-clamped to avoid overflow rings for high powers; "low bailout" below 2 allowed for all powers > 2.
- Raising perturbation and BLA step limits too far can cause GPU timeouts.
- Algorithm parameters: lock maximum reference iterations to the period (speedup if period is right, bad images if wrong); `reuse_reference` for zoom-out sequences (recomputed at each number-type change); `reuse_bilinear_approximation` not generally usable because BLA depends on zoom depth.
- Version 3 optimisations: "BLA skip levels" setting reduces memory; reference calculation has a periodicity check. Version 2: new BLA steps limit reduces computation time.
- OpenCL tiles: default 128x128; one test location took 3 min at 128x128, 1m34s at 960x1080, barely slower than a single 7680x4320 tile. Make tiles as large as possible without hitting OS timeouts (bad images or desktop crashes); avoid edge fragments; image size should be a common multiple of tile size and core/GPU count.
- Preferences window: cap image megapixels and iteration limits, important where GPU driver timeouts can kill the desktop session.
- Quality: Samples > 1 averages slightly different (jittered) renders; raw iteration EXR needs 1 sample. `image.subframes` = antialiasing samples per pixel; EXR with >1 subframe stores only RGB.
- Exponential map: mainly for zoom-out sequences for zoomasm; use zoom-out factor 2, very wide image. Recommended zoomasm keyframes: `12288x1360`, subframes 1, exponential map, zoom-out sequence, reuse reference, tiles `768x680`, lock iterations to period if known.
- Newton zooming window: Capture sets the start depth; Power presets adapt to the formula degree (2, 4, 8 ... for degree 2); defaults `newton.power = 0.5`, `factor = 4`.
- Colouring: GLSL snippet `vec3 colour()` returning linear RGB, accessors `getN0/N1/NF/T/DEX/DEY/DE/Histogram/Coord`; tiles cached so recolouring needs no recomputation (v3). Histogram colouring (3.1).
- Information window: iteration histograms (peak at the right edge = limit too low) and DE histogram for auto-unstretch.
- Location zoom 1 = vertical span -2..+2. Reference period 0 = unknown; a wrong period corrupts images.
- Web build needs SharedArrayBuffer (COOP/COEP headers); much slower than native.
- History: project started 2021-12-10; v1 2023-03-13 (fixes include high-power glitches); v2 2023-03-31; v3 2025-06-10 (needs OpenGL ES 3 / WebGL 2); v3.1 2025-12-15. Theory section points to [[sources/mathr-deep-zoom-page]].
- Alternatives using BLA: fractalshades, FractalShark, Fractal Zoomer, Imagina ("being rewritten"), Mandelbrot Perturbator GTK.

## Reliability

Primary, current (December 2025) documentation by the author. Describes behaviour and defaults,
not the algorithms in depth. Timing example is one location on unspecified hardware.

## Useful sections

- §Wisdom — per-device, per-number-type benchmarking and automatic choice.
- §Formula Window — formula toggles, hybrids, opcode list.
- §Bailout Window — perturbation and BLA step limits, GPU timeout warning, escape-radius clamping.
- §Bailout Parameters — defaults incl. inscape radius 1/1024.
- §Transform Window — exponential map.
- §Quality Window — samples, sub/super sampling.
- §Newton Zooming Window — capture, power presets by degree.
- §Preferences Window — iteration/megapixel caps against GPU timeouts.
- §Image Parameters — subframes.
- §Algorithm Parameters — lock reference to period, reuse reference, reuse BLA.
- §OpenCL Parameters — tile size measurements and advice.
- §Recommended Parameters for Zoomasm — exponential-map keyframe settings.
- §History — version notes (Version 3: BLA skip levels, periodicity check; Version 3.1: doubleexp, float128).
- §Alternatives — other BLA renderers.

## Used by

[[implementations/fraktaler-3]], [[techniques/bla]], [[techniques/rebasing]], [[techniques/extended-range-floats]],
[[techniques/exponential-map]], [[techniques/anti-aliasing]], [[techniques/interior-detection]],
[[techniques/frame-reuse-and-progressive]], [[techniques/reference-orbit-precision]], [[formulas/hybrids]],
[[formulas/burning-ship]], [[formulas/mandelbar]], [[platform/gpu-watchdog-and-chunking]],
[[questions/metal-watchdog-limit]], [[questions/reference-orbit-compression]]
