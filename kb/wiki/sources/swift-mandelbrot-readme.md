---
title: "swift-mandelbrot README (drquicksilver)"
type: source
authors: ["drquicksilver"]
year: 2026
raw: "web/swift-mandelbrot-readme.md"
status: digested
tags: [swift-mandelbrot, metal, swiftui, ios, double-float, perturbation, tiles]
---

# swift-mandelbrot README (drquicksilver)

**URL:** <https://raw.githubusercontent.com/drquicksilver/swift-mandelbrot/main/README.md> · **Saved:** `raw/web/swift-mandelbrot-readme.md` · **Terms:** no licence stated; all rights reserved by default

## What it is

The README of a SwiftUI + Metal Mandelbrot explorer for iPhone, iPad and Mac. It is the closest existing project to almondbread's stack. GitHub `drquicksilver/swift-mandelbrot`, no licence, pushed 2026-09-24 (brief).

## What it covers

- Pan and zoom at display refresh rate at any depth; zoom past 10^1000.
- Precision ladder chosen automatically: Float, then double-float, then perturbation. The Metal kernels share a double-float arithmetic library (`Mandelbrot/Shaders`).
- Deep-zoom algorithms: perturbation, rebasing and bilinear approximation, reimplemented from the equations in mathr's "Deep zoom theory and practice" (2021).
- Tile cache and tile compositor for the viewer; a separate GPU pipeline path for benchmarks (`--pipeline gpu`, variants `metal`, `metal-double`).
- Vendored attaswift BigInt (MIT) holds deep coordinates and reference orbits.
- Headless CLI in the same binary: `--render` (PNG via the tile compositor, e.g. scale 1e1000, 5000 iterations), `--benchmark`, `--movie` (zoom movie to a `mandelbrot://` link).
- Julia-set companion view, bookmarks ("Places"), share links, rendered zoom movies, rotation, automatic detail and colour.
- Targets iOS 18 and macOS 15.7; needs Xcode 26.2+.
- Testing: no CI; `make test` before each commit. Golden images come from independent sources: a CPU Double renderer, a Python model of the compositor, and Python `Decimal` direct iteration at 1e50, 1e200, 1e1000 and a period-312 minibrot at 1e100.
- Not yet done: still export, periodicity checking, and validation on iPhone 11 Pro / 16 Pro (frame pacing, heat, memory unmeasured).
- Further docs referenced but not saved: `docs/Architecture.md`, `docs/Performance.md`, `docs/Performance-history.md`, `docs/plan.md`.

## Reliability

- Primary but README-level: no numbers, no description of thresholds or of how the precision ladder switches. The referenced Architecture and Performance documents would carry the substance; they are not in `raw/`.
- No licence: facts only, no code reuse at all.

## Useful sections

- §Mandelbrot (intro) — precision ladder, 10^1000 depth, refresh-rate interaction.
- §Building and running — CLI render/benchmark/movie options.
- §Testing — independent golden-image oracles.
- §Where things are — folder map (Shaders hold double-float maths, Rendering holds tile cache and perturbation).
- §Status — open items, phone validation not done.
- §Credits — BigInt, mathr's deep-zoom post.

## Used by

- [[implementations/swift-mandelbrot]]
- [[techniques/double-float]]
- [[techniques/frame-reuse-and-progressive]]
- [[platform/ios-ipad-considerations]]
- [[decisions/app-stack]]
