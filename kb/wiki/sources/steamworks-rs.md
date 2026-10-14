---
title: "steamworks-rs (Rust bindings for the Steamworks SDK)"
type: source
authors: ["Noxime", "steamworks-rs contributors"]
year: 2026
raw: "web"
status: skimmed
tags: [steam, steamworks, rust]
---

# steamworks-rs (Rust bindings for the Steamworks SDK)

**URL:** <https://github.com/Noxime/steamworks-rs> · <https://crates.io/crates/steamworks> · <https://docs.rs/steamworks/latest/steamworks/>

## What it is

The main Rust wrapper of Valve's Steamworks SDK (MIT/Apache-2.0, except bundled Steam files).

## What it covers

- 0.13.1 (2026-05-05) targets SDK 1.64; commits through 2026-08; ~485 stars, 43 open issues.
- Wraps Apps, Friends, Input, RemoteStorage, UGC, User, UserStats, Utils, timeline, screenshots, networking.
- `steamworks-sys` uses bindgen and links `steam_api` dynamically on Windows, Linux and macOS; the library must sit next to the binary.

## Reliability

Community project, active; quality of the UGC and Input wrappers not reviewed.

## Used by

[[synthesis/fractal-games-and-steam]]
