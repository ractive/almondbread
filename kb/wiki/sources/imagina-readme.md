---
title: "Imagina README"
type: source
authors: ["Zhuoran"]
year: 2024
raw: "web/imagina-readme.md"
status: digested
tags: [imagina, agpl, renderer]
---

# Imagina README

**URL:** <https://raw.githubusercontent.com/5E-324/Imagina/master/Readme.md> · **Saved:** `raw/web/imagina-readme.md` · **Terms:** AGPL-3.0 code; do not copy into almondbread, read for facts only

## What it is

The very short README of Imagina (GitHub `5E-324/Imagina`), Zhuoran's "fast fractal renderer & explorer". Zhuoran is the author of rebasing and of the linear approximation (LA) scheme used by several renderers.

## What it covers

- Status note: "The project is being rewritten"; the repository may be renamed and replaced by the rewrite. Last push 2024-01-23 (brief, gh api 2026-10-14).
- Controls: drag to pan, scroll to zoom, double-click to "find nearby feature" and open a Feature Finder dialog; A/S colour density, D/F iteration limit, E/R colour cycling.
- Build dependencies: MPIR (arbitrary precision) and libpng; CMake variables `INCLUDE_PATH`, `LIBRARY_PATH`, `MPIR_USE_DLL` (default ON).
- Says nothing about algorithms. These are in the source; see [[sources/imagina-hinfla-evaluator]].

## Reliability

Primary but thin. The code on `master` is the pre-rewrite version; the rewrite may change the algorithms. MPIR plus Windows memory APIs in the evaluator suggest a Windows-first build (inference).

## Useful sections

- §Imagina — rewrite notice.
- §Controls — Feature Finder on double-click.
- §Build — MPIR, libpng.

## Used by

- [[implementations/imagina]]
