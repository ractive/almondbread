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

## Achievements (brainstorm, 2026-10-15)

User asked for achievements; Game Center and Steam both support them natively. Rule:
**everything is checked off the render path.** The core already computes zoom depth,
precision bits, periods, journey state and the π count. Achievements are cheap comparisons on
those values at the stats rate (~10 Hz) or on events the core already emits, with no extra GPU
or orbit work. Progress is stored locally; Game Center/Steam sync comes later.

**Depth**
- Ten thousand (1e4), Billion (1e9), Googol (1e100), Thousand digits (1e1000), Rock bottom
  (the 1e3000 depth limit).
- Bit-hungry: a view needing more than 1 000 / 5 000 bits.
- Past the universe: the whole set would be larger than the observable universe (cosmic scale,
  zoom ≈ 3e27). Into the desert: below the smallest scale ever measured (~1e-19 m).
  Planck diver: below the Planck length.
- Second gear: the ↑↑ dial of the "How big is big?" panel passes 2.5.

**Impossible ones, on purpose** (progress bars that never move, after Ganson's concrete gear):
- Googolplex: "3 000 of 10^100 digits", which shows 0.000…%.
- 10↑↑3 (10^10^10): a coordinate alone would need ~3.3e10 bits (about 4 GB); its reference
  orbit is out of reach. Graham's number and TREE(3): "no".

**Zoom odometer** (user idea): the total zoom travelled, in and out both counting, as a
lifetime total. Decades add up: `D = Σ |Δ log10 zoom|`, the total factor is `10^D`. One float
added per frame. Our arithmetic (2026-10-15) for time to reach `10^D`:

| total zoom | default speed (×4.95/s) | journey at 40 e-folds/s |
| --- | --- | --- |
| 10^10^3 | 24 min | 1.4 min |
| 10^10^4 | 4 h | 10 min |
| 10^10^5 | 40 h | 1.6 h |
| 10^10^6 | 17 days | 16 h |
| 10^10^7 | 5.5 months | 7 days |
| 10^10^8 | 4.6 years | 2.2 months |
| 10^10^10 (= 10↑↑3) | 456 years | 18 years |

So the odometer can move the "How big is big?" second gear from about 2.5 to 3: 10↑↑3 is a
lifetime of nonstop journeys; a googolplex would need 10^100 decades, so never. Farming by
holding zoom in/out is possible; harmless for local achievements, and Game Center/Steam versions
could count only frames that rendered new detail.

**Mathematics**
- First minibrot; Period collector (periods 2–10); Big period (find one with period above 1 000).
- Misiurewicz: land on a spiral or branch point found by Newton.
- Embedded Julia: reach a Julia stage of a deep minibrot (the journey's targets).
- Feigenbaum: zoom along the real axis through 5 period doublings.
- π at the neck / π at the cusp: 3, 6 or 9 correct digits on the π meter.
- Seahorse, elephant, needle, scepter, ... : visit every catalogue region.
- Interior artist: view every interior colouring at one minibrot.

**Flight and skill**
- Smooth operator: a 60 s manual flight without a late frame.
- Barrel roll: rotate 360° while zooming.
- Speed demon: zoom at more than 40 e-folds per second for 10 s.
- Pilot: land a fly-to within 1 % of the target framing; Daily journey: finish today's seed.
- Explorer: 100 jumps or places; Wanderer: one hour of journeys.

**Collection and sharing**
- Palette collector: use 20 palettes; Photographer: export a still / a video.
- Discoverer: first in the world to upload a minibrot (online atlas,
  [[decisions/minibrot-atlas]]); Census contributor: 100 verified uploads.

**Performance guard:** achievements never trigger work themselves. "Find a minibrot" uses the
period detection that runs anyway for interior colouring and the journey; anything that would
need extra computation stays out or waits for the atlas.

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
