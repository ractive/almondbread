---
title: "Inigo Quilez: Mandelbulb"
type: source
authors: ["Inigo Quilez"]
year: 2009
raw: "web"
status: digested
tags: [3d, mandelbulb, distance-estimation, ray-marching, orbit-trap]
---

# Inigo Quilez: Mandelbulb

**URL:** <https://iquilezles.org/articles/mandelbulb/> · read 2026-10-19, not saved

## What it is

Quilez's article on the power-8 Mandelbulb: the triplex iteration, a distance
estimator derived from the Hubbard–Douady potential, ray marching, orbit-trap
colouring. Year approximate (the article dates from the 2009 Mandelbulb wave).

## What it covers

- Iteration: raise the radius to the 8th power, multiply both spherical angles by 8,
  add `c`. Quilez calls the formula "completely wrong and incoherent in terms its
  algebra, but it produces beautiful images".
- A trig-free polynomial form: faster on CPUs ("a 5x speed factor" in some cases), usually
  not on GPUs; the trig form adds rounding error.
- DE from the potential `G` of degree-8 polynomials and its gradient, computed in the loop;
  the gradient also gives the shading normal. Earlier forum renders used "the wrong distance
  computation".
- Rendering: distance-field ray marching; "realtime in modern GPUs for moderate screen
  resolutions (say, 720p) with shadows but no antialiasing"; a couple of seconds per image on
  the CPU.
- Colour: four orbit traps (origin, the three coordinate planes); the origin trap doubles as
  a fake ambient occlusion.
- No discussion of precision limits or zoom depth.

## Reliability

Primary, by an expert graphics programmer; formulas on the page are images (not
transcribed here).

## Used by

[Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)

# Citations

- <https://iquilezles.org/articles/mandelbulb/>
