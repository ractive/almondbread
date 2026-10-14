---
title: "Fractalshades"
type: implementation
status: draft
license: "MIT"
sources: ["[[sources/fractalshades-readme]]", "[[sources/kf-page]]", "[[sources/fractalforums-interior-detection]]"]
tags: [implementation, python, bla, perturbation, interior-detection]
---

# Fractalshades

Geoffroy Billotey's (forum name GBy) Python package for static and
interactive deep-zoom exploration "(1.e-2000 and beyond) of a few fractals",
with a PyQt6 GUI and a Python API; documentation at
gbillotey.github.io/Fractalshades-doc (src: [[sources/fractalshades-readme]]).

## Algorithms

- Perturbation plus chained bilinear approximation for the Mandelbrot set and
  the Burning Ship (src: [[sources/kf-page]]).
- Early interior detection at depth by tracking `dz_n/dZ_1` and stopping when
  its modulus drops below eps = 0.001 (0.01 "seems safe in most cases");
  works with perturbation and BLA; GBy introduced it in fractalforums topic
  4802 in June 2022 (src: [[sources/fractalforums-interior-detection]]
  §Interior detection for deep Mandelbrot set zooms). See
  [[techniques/interior-detection]].
- Number type: floatexp (int32 exponent + float64 mantissa) below 1e-300
  (src: [[sources/fractalforums-interior-detection]] §Reply #8).
- Flexible colouring: stripe colouring and lighting effects
  (src: [[sources/kf-page]]).

## Performance (attributed)

- Minibrot at 5e-433 in just over 2 min (Ryzen 7 2700X, 16 threads), versus
  ~20 min in KF with NanoMB1 (src: [[sources/kf-page]]).
- mathr measured 2m12s at GBy's 5e-433 location with interior detection, and
  14-16 s for Dinkydau's Flake at 2400x2400
  (src: [[sources/fractalforums-interior-detection]] §Reply #9).

## Status

Active: last push 2026-08-30 (gh api); MIT; Linux, macOS, Windows.

## What almondbread can learn

MIT, so code could be read freely, but it is NumPy/Numba CPU code, not GPU.
The interior-detection result is the evidence that the derivative test works
with BLA at depth.
