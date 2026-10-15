---
title: "Mini-Mandelbrot atlas"
type: decision
date: 2026-10-15
status: proposed
tags: [research, atlas, minibrot, journey, game, education]
---

# Mini-Mandelbrot atlas

Proposed research track: catalogue every mini-Mandelbrot ("minibrot", primitive
hyperbolic component) up to some period, plus a sample of deeper ones, with enough data
per entry to study its statistics and to drive the app. Not scheduled; the user wants it
written up and implemented later.

## Context

The user asked which open questions the explorer could help with. The big ones (MLC,
density of hyperbolicity, positive area of the boundary) cannot be settled by
finite-precision pictures (unverified, general knowledge; no source page yet). What a fast
deep-zoom engine can do is experimental mathematics. Of the candidates (area estimate,
renormalisation scaling, a minibrot catalogue), the catalogue was picked because it is real
data that is not published in a usable form, as far as we know (unverified), and it feeds
features we already have:

- Journey and autopilot pick targets by period detection, Newton nucleus and size estimate
  ([[decisions/controls]] "Journey", [[sources/muency-newton-raphson-zooming]]).
- Places catalogue and thumbnails ([[synthesis/places]]).
- Gamification ideas: an atlas to fill in, discoveries, depth missions
  ([[decisions/gamification]]).

## What one atlas entry holds

| Field | How we get it |
| --- | --- |
| period `p` | atom-domain / period detection ([[sources/muency-atom-domain]]), confirmed by Newton |
| nucleus `c₀` (arbitrary precision) | Newton on `f_c^p(0) = 0` ([[sources/muency-newton-raphson]]) |
| size and orientation | complex size estimate at the nucleus (the journey's σ, [[sources/muency-newton-raphson-zooming]]) |
| parent and island | which component or minibrot it hangs off; primitive vs satellite |
| external angles of its root (optional) | ray tracing; expensive, later |
| neighbourhood descriptor | the journey's look descriptor ([[decisions/controls]] "Journey variety"): spiral pitch, density, dendrite vs spiral |
| depth and location tags | scale, region (seahorse valley, needle, limb `p/q`, ...) |
| thumbnail | `places --render-thumbs` path ([[synthesis/places]]) |

## How to build it

1. **Local atlas (cheap, first):** for any view, find all minibrots inside it above a pixel
   size, as the journey already does for its candidates. Store the results. This alone gives
   an "atlas" overlay (labels with period and size) and a growing database from normal use.
2. **Shallow census:** cover the whole set down to a fixed size bound with a quadtree survey
   (period detection on a grid, Newton from each hit, de-duplicate by nucleus). Check
   completeness against the known number of hyperbolic components per period, which grows
   about like `2^(p-1)` (unverified; needs a source page). Exhaustive coverage is only
   realistic up to roughly period 20–25; beyond that, sample.
3. **Deep samples:** follow chains (minibrot → its embedded Julia sets → the next minibrot)
   along journeys at fixed seeds, to get statistics at 1e-20…1e-300.

Everything stays in Rust (core plus CLI `almondbread atlas ...`), with results in a versioned
file format so the app, the journey and later a game server can read them.

## Questions it can answer

- How minibrot size scales with period and with depth, overall and per region. Known
  heuristic: near −2, distances shrink about like `4^-p` (our own note, see [[synthesis/places]]).
- How minibrots cluster: along needles, in valleys, around Misiurewicz points.
- How regular the surroundings of a minibrot are (descriptor statistics): when does a dive
  look "new"? This feeds journey scoring directly.
- Educational views: period histogram of a view, "the atlas so far", nearest unvisited minibrot.

## Not in scope

- Claims about MLC, queer components or the boundary's area: floating-point pixels cannot
  decide them. The atlas will say so in the app.
- Rigorous (interval-arithmetic) proofs of entries; Newton convergence plus a residual check
  is enough for a catalogue.

## Effort and risks

- Mostly reuse: period detection, Newton, size estimate, descriptors and thumbnails exist.
  The new parts are the census driver, de-duplication, storage and the overlay.
- Risk: the census at higher periods is dominated by Newton failures and near-duplicate
  nuclei; needs the 128-extra-bit Newton the journey uses at depth ([[decisions/controls]]).
- Risk: a large dataset with no consumer. Mitigation: build step 1 with the overlay first, so
  every entry is visible in the app from day one.

## Status

Proposed 2026-10-15. Implementation waits for the user's go.
