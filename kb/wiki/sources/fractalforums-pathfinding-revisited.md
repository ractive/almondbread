---
title: "Fractal Forums: Pathfinding in the Mandelbrot set - Revisited (2019)"
type: source
authors: ["wbarry", "Fraktalist", "claude", "quadralienne", "tavis"]
year: 2019
raw: "web"
status: digested
tags: [julia-morphing, shape-stacking, internal-address, places]
---

# Fractal Forums: Pathfinding in the Mandelbrot set - Revisited (2019)

**URL:** <https://fractalforums.org/fractal-mathematics-and-new-theories/28/pathfinding-in-the-mandelbrot-set-revisited/2916>
(live site returned HTTP 500 on 2026-10-17; read via the Wayback snapshot of 2024-12-26).

## What it covers

- wbarry: every minibrot is surrounded by concentric "shells" that record the path to it
  from the main cardioid (Schleicher's internal addresses as a road map); new moves are
  spliced in, doubling the shell variants; "zooming to 3/4 depth finds the magical shell with
  2 arms ... the next step in the Julia Morph. Same for the 4 and 8 armed automated zooms
  being at 7/8 and 15/16." A Julia morph is the last two-arm shell; to steer it, pick a new
  minibrot and the path to it is inserted "into the center of the current shape, inverted".
  "The path you choose to follow BUILDS the shape you will find."
- quadralienne: cusp, then seahorse valley, then the other side, then a minibrot gives a
  minibrot ringed by cusp-seahorse-otherside patterns.
- claude (Heiland-Allen): "history repeats, only twice as fast" on approaching mini-sets and
  period-doubling shape stacking.
- Fraktalist: the zoom path stores information (branch choices as bits); tavis: bulbs of
  bulbs record the path too, without visiting minibrots.

## Reliability

Forum discussion, no proofs (wbarry says so); the 3/4, 7/8, 15/16 rule matches the KF
Newton-zoom presets ([Kalles Fraktaler 2 + manual](kf-manual.md)) and [Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals](dinkydau-evolution-journals.md).

## Used by

[Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)
