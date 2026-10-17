---
title: "FractalShark README"
type: source
authors: ["Matt Reiman"]
year: 2026
raw: "web/fractalshark-readme.md"
status: digested
tags: [fractalshark, cuda, gpu-reference-orbit, linear-approximation, reference-orbit-compression, gpl]
---

# FractalShark README

**URL:** <https://raw.githubusercontent.com/mattsaccount364/FractalShark/main/README.md> · **Saved:** `raw/web/fractalshark-readme.md` · **Terms:** GPL-3.0 repository; README text copyright the author

## What it is

The README of FractalShark (GitHub `mattsaccount364/FractalShark`, GPL-3.0, pushed 2026-10-12): a Mandelbrot-only deep-zoom renderer for NVIDIA GPUs (CUDA) on Windows and Linux. It lists the project's engineering experiments and points to a longer design document, `FractalShark.pdf`.

## What it covers

- GPU reference orbit: the "Ref2/v2" path computes the whole high-precision recurrence in one persistent fused CUDA kernel, using Number Theoretic Transform multiplication, fused pointwise recurrence algebra, signed carry propagation and normalisation.
- Reference orbit timings (author's): v0.532, 16384 32-bit limbs (~158,000 digits): ~10x faster than the multithreaded MPIR + AVX-2 CPU path on an RTX 4090, ~9x on an RTX 5090. v0.54: ~70x faster on an RTX 5090 than a single-threaded AVX-2 MPIR orbit. The README says the multithreaded CPU test was broken, so v0.54 has no fair CPU comparison.
- Crossover: above roughly 1e10000 zoom the GPU orbit may win; shallower, the CPU usually wins.
- Built-in deepest example: a minibrot at ~1e650452 (View #34), found with GPU Newton–Raphson and the GPU orbit.
- Many distinct CUDA rendering kernels; two CUDA linear approximation (LA) implementations, ported from Fractal Zoomer and going back to Claude Heiland-Allen and Zhuoran.
- CPU reference orbit "MT3": three threads (two squaring, one coordinating), best on an AMD 5950X. A custom allocator supports "perturbed perturbation": caching intermediate-precision orbit values for reuse across successive zooms.
- "2x32 + exponent" type: two f32s plus a shared exponent, ~48-bit mantissa, without native f64; much faster on consumer GPUs, with a modest precision loss. CUDA only; on CPU native double is preferred.
- Reference orbit compression (idea from Imagina, Zhuoran's fractalforums post "Reference compression"; Claude's `mandelbrot-reference-compression` sample). FractalShark decompresses segments on demand during per-pixel rendering. In a period-600,000,000 case this saved multiple GB. Save files are Imagina-compatible.
- Feature Finder: period detection plus high-precision Newton–Raphson; modes Direct, Perturbation, Linear Approximation, each with optional grid scan. The Newton loop can run on the GPU inside the same NTT kernel.
- Rendering: GPU supersampled antialiasing, palette colouring, per-frame iteration statistics via GPU reduction.
- Requirements: CUDA compute capability 7.5+ (GTX 16 / RTX 20 and newer); RTX 20+ for the GPU orbit. Linux support since v0.53 (2026-06).
- LA table layout: for HDR-float with 32-bit indices, an LA row is 52 bytes and a stage descriptor 16 bytes on both CPU and GPU. `MinMag` and per-row `LAThresholdC` live in setup-only storage; each stage keeps its first row's threshold.
- Inspirations: Kalles Fraktaler and mathr's deep-zoom posts for perturbation; Imagina for reference periodicity detection; Fractal Zoomer for float/exp and LA.
- No AMD support, no non-Mandelbrot formulas, CPU path "not optimized".

## Reliability

- Primary for FractalShark's own design; all speed numbers are the author's, on his hardware, often without a matched baseline (stated openly). Self-described as experimental and buggy.
- Author name: the manifest says "Matt Reiman"; the Desarso README credits "Matt Renzelmann". The README itself gives no full name. Unresolved; check the GitHub profile.
- The README says the CPU-side LA came from Fractal Zoomer, whose own README does not mention LA (see [[sources/fractalzoomer-readme]]).

## Useful sections

- §1. GPU-accelerated reference orbit computation — NTT orbit, timings, 1e650452 minibrot.
- §3. CUDA-based linear approximation implementations — provenance of the LA code.
- §4. Multithreaded high-performance CPU reference orbit calculation — MT3, crossover depth, perturbed perturbation.
- §5. Custom "2×32 + exponent" numeric type — the ~48-bit float-float-plus-exponent type.
- §6. Reference orbit compression and on-the-fly decompression — runtime decompression, memory savings.
- §7. Feature Finder / periodic point detection — period + Newton, GPU Newton.
- §What other projects are used for inspiration? — lineage of each technique.
- §Build instructions (end) — LA row/stage byte sizes.

## Used by

- [[implementations/fractalshark]]
- [[techniques/linear-approximation-imagina]]
- [[techniques/reference-orbit-precision]]
- [[techniques/extended-range-floats]]
- [[techniques/double-float]]
- [[questions/reference-orbit-compression]]
- [[techniques/reference-selection]]
- [[synthesis/finding-deep-places]]
