---
title: "Steamworks: Leaderboards"
type: source
authors: ["Valve"]
year: 2026
raw: "web"
status: skimmed
tags: [steam, steamworks, leaderboards, game]
---

# Steamworks: Leaderboards

**URL:** <https://partner.steamgames.com/doc/features/leaderboards> · API <https://partner.steamgames.com/doc/api/ISteamUserStats> (fetched 2026-10-14)

## What it is

Valve's documentation of Steam leaderboards (ISteamUserStats).

## What it covers

- Up to 10,000 leaderboards per app; one entry per player; int32 score plus up to 64 int32 details; KeepBest or ForceUpdate; ascending/descending; Numeric, Seconds or Milliseconds display.
- `AttachLeaderboardUGC` links a shared file (e.g. a replay or ghost) to an entry; the handle comes from `ISteamRemoteStorage::FileShare`.
- Trusted writes: scores only through the `SetLeaderboardScore` Web API (needs a server with a publisher key); reads can be limited to friends.
- No daily or reset feature is described.

## Reliability

Primary vendor documentation. Runtime creation limits beyond the 10k cap are not documented.

## Used by

[[synthesis/fractal-games-and-steam]]
