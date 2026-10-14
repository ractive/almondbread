---
title: "swift-mandelbrot (drquicksilver)"
type: implementation
status: draft
license: "none stated (all rights reserved)"
sources: ["[[sources/swift-mandelbrot-readme]]"]
tags: [implementation, swift, metal, ios, macos, bla, rebasing]
---

# swift-mandelbrot (drquicksilver)

A SwiftUI + Metal Mandelbrot explorer for iPhone, iPad and Mac: the closest
existing project to almondbread (src: [[sources/swift-mandelbrot-readme]]).

## What it does

- Panning and zooming at display refresh rate at any depth; zoom "goes past
  10^1000".
- Precision ladder chosen automatically: Float, then double-float (Metal has
  no fp64), then perturbation.
- Perturbation, rebasing and bilinear approximation "follow Claude
  Heiland-Allen's deep zoom theory and practice, reimplemented from the
  equations"; deep coordinates and reference orbits use a vendored BigInt
  (MIT), not GMP/MPFR.
- Tile cache, worker and compositor; Julia companion view; bookmarks as
  `mandelbrot://` links; zoom movies; headless CLI rendering and benchmarking
  through the same renderers.
- Testing: golden images never produced by the code they judge: CPU double
  for reference counts, an independent Python model of the compositor, and
  Python `Decimal` direct iteration at 1e50, 1e200, 1e1000 and a period-312
  minibrot at 1e100.
- Needs Xcode 26.2; targets iOS 18 and macOS 15.7.
(all src: [[sources/swift-mandelbrot-readme]])

## Status

Active: last push 2026-09-24 (gh api), (c) 2026. Plan: still-image export,
periodicity checking and validation on iPhone 11 Pro and 16 Pro, where frame
pacing, heat and memory "have not yet been measured"
(src: [[sources/swift-mandelbrot-readme]] §Status). No licence file, so the
code is not reusable; ideas and test strategy are.

## What almondbread can learn

- Proof that float32 Metal plus BLA plus rebasing works on Apple Silicon.
- The oracle-based golden-image test strategy is exactly the verification
  harness the roadmap asks for ([[synthesis/deep-zoom-rendering-techniques]]).
- iOS thermal and memory behaviour is still unmeasured there too
  ([[platform/ios-ipad-considerations]]).
