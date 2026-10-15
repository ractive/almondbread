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

## Online atlas (user idea, 2026-10-15)

User: users upload their atlas to ractive.ch, or the app sends what it discovers in the
background; the server stores everything and publishes an online Mandelbrot atlas.

Driver's assessment:
- **Opt-in only.** The entries are mathematics, but an upload carries an IP address, a time and,
  for credit, an account: personal data under the Swiss nFADP and GDPR, and an App Store
  privacy label (unverified, needs a source page). Background sharing is a switch the user turns
  on ("Share discoveries"), never the default; a manual "Upload my atlas" comes first.
- **Never trust the client.** The server re-verifies every entry (Newton from the given nucleus,
  period and size check) with the same Rust core, which already builds for Linux. Junk fails
  verification; valid but boring entries are harmless.
- **Canonical key from day one:** period plus the nucleus rounded to a precision relative to the
  minibrot's size. The local atlas uses the same key, so uploading later needs no migration.
- **Store coordinates, not images.** Thumbnails are deterministic from the coordinates and
  settings; render them server-side (CPU oracle, slow but fine for small thumbnails) or in the
  browser later (core compiled to WebAssembly). No user images means no moderation.
- **Two kinds of data:** a server-side census (complete at shallow depth, systematic) plus user
  discoveries (deep, biased to where people fly, with "first found by" credit). The bias is
  itself interesting: a heatmap of where people explore.
- **Fits the game ideas** ([[decisions/gamification]]): discovery credit, daily journeys, a
  shared map.
- Order: local atlas plus export file → manual upload → opt-in background sharing and the
  website. Backend: a small Rust service with SQLite or Postgres on ractive.ch (not decided).

## Fog of war: two maps (user idea, 2026-10-15)

User: like a game where the fog lifts where you have been, with two layers: where people
looked around, and where minibrots were found.

- **Multi-scale, not one picture.** The set spans thousands of decades of scale, so a flat
  heatmap cannot hold it. Use a quadtree like web map tiles: a view marks the tiles at its own
  level (`z ≈ log2(zoom)`) that it covered at about its own resolution. A tile key is the
  quadtree path, 2 bits per level (about 2.5 KB at 1e-3000; deep keys can share prefixes).
- **Fog at every depth.** Zooming into a cleared region shows fog again at the finer level:
  explored is always relative to a level. "% explored" per depth decade drops towards zero
  with depth: the infinite map, made visible.
- **Layer 1, exploration:** visit counts per tile. **Layer 2, discoveries:** found minibrots
  (period, size, finder).
- **Combined with the census:** the census knows how many minibrots up to period N exist in a
  tile, so a tile can say "3 of 7 found", a game loop with no invented content.
- **Personal fog vs world fog.** Your own fog stays on the device, needs no upload, and is
  always available. The world fog is aggregated from opt-in uploads, coarse and anonymous.
- **Display:** fog darkens the mini-map; a subtle tint in the main view at the current level
  marks never-visited ground; the online atlas shows both layers.
- **Deferred (user, 2026-10-15): too complex to show.** Simpler first version instead: your
  trail as outlines in the mini-map's nested levels (they already handle scale), found minibrots
  as markers in the view, counters instead of maps ("12 found, 3 of the period-≤12 ones"), and
  on the website a plain 2D heatmap only for the shallow set (zoom ≤ ~1e6) plus search and lists
  for deeper entries. Multi-level fog only if a game mode needs it.

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
