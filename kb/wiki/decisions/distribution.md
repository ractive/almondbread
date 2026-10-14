---
title: "Distribution: which stores, in which order"
type: decision
date: 2026-10-14
status: proposed
tags: [distribution, steam, app-store, business, platform]
---

# Distribution: which stores, in which order

## Context

The user asked whether almondbread can be sold on Steam, Apple's stores,
the Microsoft Store and others. Background:
[[synthesis/fractal-app-market-research]],
[[synthesis/fractal-games-and-steam]].

## Stores (fees from the research or from memory; "verify" = unchecked)

| Store | Platforms | Fee | Effort | Fit |
| --- | --- | --- | --- | --- |
| App Store / Mac App Store | iPhone, iPad, Mac (Universal Purchase) | 15 % (Small Business Program) | SwiftUI app, current track | core |
| Steam | Windows, Linux/Deck, Mac | 30 %, $100 per app | wgpu backend, Rust shell, Steamworks | core for the game edition |
| itch.io | Windows, Linux, Mac | developer-chosen, default 10 % | Steam build without Steam features | cheap extra, demos |
| Microsoft Store | Windows | 12 % for games (verify) | MSIX packaging, certification | optional |
| Epic Games Store | Windows, Mac | 12 %, reportedly 0 % on the first $1M per year (verify) | separate integration | only with traction |
| GOG | Windows, Mac, Linux | 30 % | curated application | unlikely |
| Google Play | Android | 15 % tier (verify) | wgpu on Vulkan, touch UI | later port, if ever |

## Proposal

1. Apple stores first (current plan).
2. Steam (Windows, Linux/Deck) for the game edition, behind the gates of
   [[synthesis/fractal-games-and-steam]] (wishlist velocity); itch.io with
   the same build.
3. Microsoft Store and Epic only after Steam shows traction.

## Architecture consequences

- One core, several builds: store features behind `almondbread-platform`
  traits (Steam: achievements, leaderboards, Workshop, Cloud; Apple: Game
  Center, CloudKit; local no-op for itch.io), selected by Cargo features.
- Cross-store features (atlas, daily challenge, leaderboards) on our own
  small backend, so players compete across stores; store achievements stay
  per store. Needs an account model, moderation and GDPR handling.
- No cross-buy between stores; Universal Purchase only links Apple
  devices. Sync of bookmarks and flights across stores only through our
  backend (optional).

## Rules to watch

Steam key pricing parity (no worse deal for Steam customers); the Mac App
Store and Steam Mac builds are separate apps (App Sandbox vs Steam);
IARC age ratings, with moderation for player-chosen atlas names.
