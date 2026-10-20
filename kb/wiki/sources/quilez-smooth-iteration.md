---
title: "Quilez: Smooth iteration count for generalized Mandelbrot sets"
type: source
authors: ["Inigo Quilez"]
year: 2007
raw: "web/quilez-smooth-iteration.html"
status: digested
tags: [smooth-coloring, escape-time, multibrot, gpu-shader]
---

# Quilez: Smooth iteration count for generalized Mandelbrot sets

**URL:** <https://iquilezles.org/articles/msetsmooth/> · **Saved:** `raw/web/quilez-smooth-iteration.html` (+ `.txt`; formulas are images, reconstructed below from the text and the shader code) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Short article by Inigo Quilez deriving the smooth (continuous) iteration count for polynomial
fractals of any integer degree `d`, with a GLSL implementation (Shadertoy 4df3Rn).

## What it covers

- Setting: `f(z) = z^d + ...` (monic polynomial); critical orbit from `z_0 = 0` when `f'(0) = 0`; far from the origin `f(z) ~ z^d`.
- Bailout `B`: 2 is enough to decide escape for `d = 2`, but the smoothing needs the `z^d` approximation to hold, so use a larger `B` (around 50 is enough; the code uses 256).
- First smoothing: `log_B |z_n|` maps `[B, B^d]` to `[1, d]`; linear remap gives a fractional part, but it is not uniform because it grows by powers of `d`.
- Final formula: `n_smooth = n - log_d(log|z_n| / log B)` (the second log linearises the exponential growth).
- Works with `|z_n|^2` to avoid a square root. For `d = 2` with base-2 logs and constant `B`: `n - log2(log2(|z|^2)) + k`; with `B = 256` the constant is `k = 4` (check: `log2 log2 |z|^2 = 1 + log2 log2 |z|`, `log2 log2 B = 3`).
- Example with a degree-7 polynomial shows the formula generalises.

## Reliability

Secondary, widely used and mathematically sound for polynomial maps; the derivation is
informal. Formulas in the saved `.txt` are images; the forms above follow from the shader code in
the text (inference for the exact typeset forms). Not about deep zoom; at deep zoom the smoothing
term must be computed from the full value `Z + z` (inference).

## Useful sections

- §The basics — setting, critical points, bailout radius choice.
- §The technique — derivation, final formula, the `B = 256` shader with `+ 4.0` constant.

## Used by

[Smooth (continuous) iteration count](../techniques/smooth-coloring.md), [Colouring methods](../techniques/coloring-methods.md), [Multibrot (integer power p)](../formulas/multibrot.md), [Mandelbrot set (power 2)](../formulas/mandelbrot.md)
