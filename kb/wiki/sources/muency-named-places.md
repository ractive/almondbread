---
title: "Mu-Ency: named features and favourite views (coordinates)"
type: source
authors: ["Robert Munafo"]
year: 2024
raw: "web"
status: digested
tags: [places, locations, glossary, embedded-julia-set]
---

# Mu-Ency: named features and favourite views

**URL:** <https://mrob.com/pub/muency.html> (entries listed below) · **Not saved** (read
2026-10-14) · **Terms:** CC BY-NC 4.0 text; we take coordinates only (facts) and write our
own descriptions.

## What it is

Robert Munafo's Mandelbrot Set Glossary and Encyclopedia (1987-2024). Feature entries
name regions colloquially (seahorse, elephant, scepter valleys, medallions, paramecia,
embedded Julia sets) and give views as `re + im i @ r` with an iteration limit (`Nmax`).

## What it covers

Views used by [Places: a catalogue of famous views to visit](../synthesis/places.md) (entry: centre @ r):

- `r2c0` elephant valley: 0.29833 + 0.00111i @ 0.1; 0.291665 - 0.015555i @ 0.008249;
  0.2925755 - 0.0149977i @ 0.0004277 (the landscape prototype's `elephant`).
- `r2c13` triple spiral valley: -0.15490 + 0.65218i @ 0.00781; -0.156250 + 0.653411i @ 0.000976.
- `r212c12` scepter valley: -1.249783 + 0.029353i @ 0.00355; -1.2494989 + 0.0303330i @ 0.0004437.
- `r213c12` double scepter valley: -0.1296 + 0.8394i @ 0.0024.
- `r214a`, `r215a`, `r225a`, `r2f58b`, `r2f12b`, `r2f13b1s`: the 1/4, 1/5, 2/5 bulbs, the
  5/8 limb, the spike, an island on the 1/3 limb.
- `peacockeye`, `shepherdscrook`, `paramecia`, `embeddedjuliaset`,
  `secondorderembeddedjuliase` (a 13-step zoom to 2.7e-22), `2foldembeddedjuliaset`,
  `4foldembeddedjuliaset` (6e-15).
- Medallions near the island R2F(1/2(2/3B1)B1)S around -1.255 - 0.381i: seahorse,
  triple spiral, elephant, cauliflower medallions (1e-6 to 1.6e-8).
- `favoriteviews1992/1993/1994`: hand-zoomed views to 1.5e-27 near -1.7488 and
  -0.592 + 0.620i.
- `rationalcoordinates`: exact names of points (seahorse valley -3/4, elephant valley
  1/4, quad spiral valley 1/4 + i/2, c = i).

## Reliability

Careful primary work by one author over decades. **Convention:** `@ r` is the half-height
of the view (verified 2026-10-14 by rendering: span 2r on the shorter side frames each
view as its text describes, e.g. the spike's `@ 0.75` shows the whole antenna from -2 to
-1.25). Iteration limits are those of 1990s hardware and much lower than our automatic
limit.

## Useful sections

- The `Coordinates:` line under each figure; `favoriteviews*` for deep hand-made views.
