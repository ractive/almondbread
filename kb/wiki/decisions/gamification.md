---
title: "Game mechanics for almondbread"
type: decision
date: 2026-10-14
status: proposed
tags: [game, product, steam, landscape, community]
---

# Game mechanics for almondbread

## Context

The user asked how to gamify the explorer. The Mandelbrot set is static
(no physics, no agency), so play has to come from navigation, discovery,
precision and time. Its strengths as a game world: infinite, every place
unique, exactly addressable and the same for every player. The market
research found that games sell on Steam and tools do not
([[synthesis/fractal-app-market-research]]).

## Candidate mechanics (best fit first)

1. **Atlas: first discoverer names it.** Minibrots and other structures
   (spirals, n-armed stars, Julia islands) are classified automatically
   (period, position; [[techniques/reference-selection]]); the first
   player to find one names it and it enters a shared global atlas.
   Rarity from the maths (period, depth, shape). Exact coordinates make it
   cheat-resistant. Needs a small backend (e.g. CloudKit public database
   or Steam leaderboards/Workshop).
2. **Daily challenge (GeoGuessr/Wordle-like).** One snapshot per day for
   everyone; find it from the full set; score by closeness and time;
   share a spoiler-free result card. Cheap: reuses share links.
3. **Canyon racing (landscape mode).** Time trials through boundary
   valleys with checkpoints at coordinates; ghosts from the flight
   recorder; speed ∝ altitude gives risk/reward; crash = terrain or lake.
   Depends on [[decisions/landscape-mode]].
4. **Depth missions.** "Reach a period-1000 minibrot in 3 minutes", "pass
   1e100 without touching the interior"; skill = reading the structure.
5. **Journey mode.** No score; guided flights with music following the
   structure (Journey/Proteus-like), for the relaxation audience.

Avoid: grafted-on combat, enemies, upgrade trees, resource systems tied
to iteration budgets.

## Similar games (from memory, to be verified by research)

Marble Marcher (CodeParade, fractal levels), Fractal Block World (Steam,
~1650 reviews), Manifold Garden, Superliminal, Everything (scale),
Hyperbolica (non-Euclidean), GeoGuessr and Wordle (daily/shareable).

## Proposed order

Daily challenge (small, viral, ships with share links) → atlas (unique
hook, community) → canyon racing (after landscape mode). A game-leaning
edition could justify the Windows/Linux port on Steam.

## Open

Verified competitor list and player reception; Steam technical and
business implications; whether a game engine is needed — see the
research run "Fractal games and Steam" (in progress).

## Research outcome

The research run is done: [[synthesis/fractal-games-and-steam]]. It confirms
the order daily → atlas, adds depth missions as launch content (goals
counter "not a game" reviews and the two-hour refund window), keeps journey
mode as a mode only, and moves canyon racing to a post-launch update gated
on a landscape prototype and a motion-sickness playtest. No commercial
deep-zoom Mandelbrot game exists; Fractal Block World (1,652 reviews) is the
only fractal hit (src: [[sources/fractal-block-world-steam]]). Steam
leaderboards carry daily boards and race ghosts
(src: [[sources/steamworks-leaderboards]]), but the atlas needs an own small
backend with server-side re-verification and name moderation. No game
engine: a Rust app on wgpu + winit with steamworks-rs, Metal kept on Apple,
game rules in a GPU-free `almondbread-game` crate so the daily challenge can
ship in the SwiftUI app first. A Steam Coming Soon page's wishlist velocity
is the go/no-go gate for the full Steam build.
