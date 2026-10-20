---
title: "Wikibooks: Fractals/emndl (emndl_autotune)"
type: source
authors: ["Wikibooks contributors"]
year: 2024
raw: "web"
status: skimmed
tags: [automation, places, newton, period]
---

# Wikibooks: Fractals/emndl (emndl_autotune)

**URL:** <https://en.wikibooks.org/wiki/Fractals/emndl> (read 2026-10-17, not saved).

## What it covers

emndl is Claude Heiland-Allen's older zoom-video renderer (exponential strips unwarped into
frames). `emndl_autotune` (Haskell, MPFR) "attempts to find interesting points in the
Mandelbrot Set that might look nice"; its sample output is a chain of nuclei with growing
period and shrinking size (P 1, 4, 31 ... with sizes 1, 8.3e-3, 2.3e-7) and an ASCII preview
per step; `emndl --auto 96` logs a chain of periods 1, 5, 21, 115, 596. The page does not say
how candidates are scored.

## Reliability

Secondary and thin; the selection criterion is unknown (inferred from output only).

## Used by

[Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)
