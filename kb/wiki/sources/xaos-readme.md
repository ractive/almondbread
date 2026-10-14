---
title: "XaoS README"
type: source
authors: ["XaoS project"]
year: 2026
raw: "web/xaos-readme.md"
status: digested
tags: [xaos, realtime-zoom, qt, gpl]
---

# XaoS README

**URL:** <https://raw.githubusercontent.com/xaos-project/XaoS/master/README.md> · **Saved:** `raw/web/xaos-readme.md` · **Terms:** GPL-2.0 repository

## What it is

The README of XaoS (GNU XaoS), a real-time interactive fractal zoomer in development since 1996. GitHub `xaos-project/XaoS`, GPL-2.0-or-later, pushed 2026-09-11 (brief).

## What it covers

- Real-time smooth zooming into any point "without the long calculation" of other generators. The README does not explain the method; the zoom algorithm keeps only the previous frame and reuses its rows and columns (src: [[sources/wikipedia-xaos]] §Hubička algorithm).
- Features: several fractal types, autopilot, special colouring modes, random palettes, colour cycling.
- Since 4.3 (September 2023) a full web app at xaos.app; a simplified algorithm demo is XaoS.js.
- Built on Qt Widgets; tested on Windows, Mac and Linux.
- Original authors Jan Hubička and Thomas Marsh; maintained by J.B. Langston and Zoltán Kovács.
- Licence: GPL version 2 or later.

## Reliability

Primary but promotional; no technical content. No deep-zoom (perturbation) support is claimed. Algorithm facts must come from other sources.

## Useful sections

- §XaoS (intro) — real-time zoom claim, web version.
- §License — GPL-2.0-or-later.

## Used by

- [[implementations/xaos]]
- [[techniques/frame-reuse-and-progressive]]
