---
title: "A11Y: Accessibility of the canvas and the Rust-drawn controls"
type: plan
milestone: A11Y
status: in-progress
priority: 11
effort: S
appeal: medium
audience: [casual, education]
depth: "any (describes what is drawn; no rendering change)"
realtime-ipad: "yes"
reuse: "high: the overlay layout the UI tests already read (cockpit cells, help badges, panel rects), the same actions a click or a tap calls"
depends-on: []
risks:
  - "overlapping elements (controls on the full-screen canvas): VoiceOver's touch exploration may pick the canvas over a control"
  - "per-frame cost if the elements were rebuilt every refresh"
  - "UI tests read the canvas element's value as a state string and find round buttons by label: new labels must not collide"
sources: ["decisions/controls"]
tags: [accessibility, voiceover, controls, ios, macos]
created: 2026-10-20
updated: 2026-10-20
---

# A11Y: Accessibility of the canvas and the Rust-drawn controls

Status: built on its branch 2026-10-20, awaiting review (from the external code review's
finding S1, medium).

## Goal

VoiceOver, Switch Control and the Mac's accessibility clients can find, read and use the controls
Rust draws into the Metal frame (the cockpit's level and lane icons, the "?" help badges, the
compass, the mini-map panels, the Julia panel, the π meter, the cosmic size scale), and the canvas
itself says what it shows in one short sentence instead of 60 digits (Mac: it had no role or label
at all; iPad: one element whose value was the UI tests' state string).

## Scope

- Rust describes every control drawn in the last scene: an id, a role (button, image, text), a
  label, a value and its rect in view points; plus a one-sentence summary of the view (zoom,
  centre to 7 digits, turn, driving level). The words live in Rust so every platform says the same.
- Swift turns these into `UIAccessibilityElement`s (iOS, iPadOS) and `NSAccessibilityElement`s
  (macOS) of the fractal view and routes activation to the calls a click or a tap makes:
  cockpit icons switch the level or toggle lane assist, help badges open their popover, the compass
  turns to north, a mini-map panel jumps to the point at its centre.
- The canvas element: Mac role image, label "Mandelbrot set", value the summary, custom actions
  Zoom in / Zoom out; iOS the same label and value, direct interaction (gestures pass through),
  identifier `fractal` (the UI tests' anchor), and the old state string as its value only with
  `ALMONDBREAD_UITEST=1`.
- Values: the current level's icon says "current, gear 5, zooming in ×4.2 per second"; lane assist
  "on" / "off"; compass "turned 35° counter-clockwise"; mini-map "whole set, level 1 of 3"; Julia
  panel "c = …"; π meter the latest count, product and digits; cosmic scale its sentence.

## Non-goals

- Describing the picture's content (where the bulbs are): no image recognition.
- Keyboard navigation of the drawn controls: every one already has a key (kb: decisions/controls).
- A Mac XCUITest target (none exists); the Mac check runs through the `--ui-demo` machinery.

## Design sketch

- `almondbread-render/src/accessibility.rs`: `Host::accessible_controls(now)` walks what the last
  scene laid out (`cockpit_controls`, `help_badges`, `overlay.panels`, `Overlay::map_panels` with
  each level's decade, the compass centre while shown, the Julia `k`, `PiMeter::state`,
  `CosmicOverlay::reading`) in reading order (compass, maps, Julia, cosmic, π, cockpit, each
  panel's help badge after it); pure value functions with unit tests.
- FFI (`almondbread-ffi/src/accessibility.rs`, additive): `Explorer.accessibleControls()`,
  `Explorer.accessibilitySummary()`.
- Swift `CanvasAccessibility.swift`: id → action mapping, a cache (`CanvasAccessibilityState`) that
  reads Rust at most every 0.1 s and counts generations; `CanvasAccessibilityElements.swift`:
  the element classes, each keeping its object identity per id (VoiceOver keeps focus on the
  object). Nothing runs per frame: elements are read on demand; while VoiceOver or Switch Control
  runs, the view refreshes at the stats cadence (~10 Hz while animating) and posts a
  layout-changed notification when controls come, go or move.
- Hit testing: Mac `accessibilityHitTest` picks the control under the point, else the canvas; iOS
  lists the controls before the canvas element.

## Affected modules

- `crates/almondbread-render/src/accessibility.rs` (new), `overlay.rs` (`map_panels`),
  `cosmic.rs` (`reading`), `lib.rs`.
- `crates/almondbread-ffi/src/accessibility.rs` (new), `lib.rs`.
- `Sources/AlmondbreadKit/CanvasAccessibility.swift`, `CanvasAccessibilityElements.swift` (new),
  `FractalView.swift` (both views), `SelfTest.swift` (`--ui-demo=accessibility`).
- Tests: `Tests/AlmondbreadKitTests/CanvasAccessibilityTests.swift`,
  `Tests/iOSUITests/AccessibilityUITests.swift`.

## Verification

- Rust unit tests of the words (levels, compass, maps, π meter, summary).
- `make swift-test`: id → action mapping, frame filtering, layout-change detection, centre maths.
- iOS UI tests (iPad and iPhone): the cockpit's buttons exist with their labels and the current
  level selected, tapping the cruise element switches the level, a mini-map element exists with
  its value, the canvas element still answers as `fractal`.
- Mac `--ui-demo=accessibility`: the fractal view's accessibility tree printed (roles, labels,
  values, frames), AXPress on the cruise element switches the level, the help badge opens its
  popover.
- Mandelbrot fast path unaffected: no kernel or scheduler change; `make interaction-test` draw CPU
  per frame unchanged (the elements cost nothing while no client asks).

Results (measured 2026-10-20, M-series MacBook Pro, release; iOS 26.5 simulators `AB iPad mini`,
`AB iPhone 17e`):

- Mac `--ui-demo=accessibility`: "ui-demo: accessibility OK", 17 children (canvas, compass, two
  maps, Julia panel, cosmic scale, π meter, five cockpit buttons, help badges), AXPress on cruise
  selects it ("current, gear 1, zooming in ×1.08 per second"), window observers 9 attached, 0
  detached, 9 after 20 round trips. The scene sits at 1e-9: at 3000× the mini-map has only one
  level (levels stay 1.5 decades from the view), so its first run asked for a `map-1` that
  correctly was not there.
- Pitfall: the window demos print nothing while the Mac's display sleeps (no display-link
  callbacks, the view never settles); `caffeinate -d -u` keeps it awake for the run.
- iOS `AccessibilityUITests` on both devices: OK after one fix. The compass element was missing on
  the iPad: the display link pauses on a still view, so the compass's visibility read at the
  current time had faded out (hold 2.5 s, fade 0.5 s) while the last frame still showed the rose.
  The elements now take the compass's visibility at the last drawn frame
  (`CompassOverlay::shown_in_last_frame`).
- `TouchControlsUITests` and `FirstUseUITests` on the iPhone: all pass after one test change.
  `testTestsDoNotPersistSettings` read "orbit off" from the canvas's value in a normal launch,
  which is now VoiceOver's summary; it reads the round Orbit toggle's value instead.
- `make interaction-test`: every shown frame equals a fresh render; draw CPU p50 0.17–0.96 ms,
  p95 0.22–3.49 ms per scenario (30 Hz link granted that run). The element work runs at the
  stats cadence and only while an assistive client is on, not per frame; no A/B against main
  was run.

## Review gate

The user turns VoiceOver on (Mac: ⌘F5; iPad: Settings › Accessibility) and moves through the
canvas: are the controls found, are the words right, does a double tap on the cruise icon switch
the level, is the summary short enough?

## Risks

- Overlapping frames on iOS: whether VoiceOver's touch exploration prefers the first element in
  `accessibilityElements` or the smallest is not documented (unverified); the controls come first.
  Swiping reaches every element either way.
- Values change every frame while flying (zoom, rate); they are read when VoiceOver asks, never
  announced, so it does not chatter.

## Open questions

- Should the canvas announce arrivals (a journey stop, a seek result) as VoiceOver announcements?
- Should a mini-map's activation fly (the Mac's ⌥-click) rather than jump?

## Effort breakdown

S overall (about a day).

- Rust descriptions and tests: 0.3 day.
- Swift elements on both platforms, actions, cache: 0.4 day.
- UI tests, Mac demo check, kb: 0.3 day.

# Citations

- [Controls: keyboard, pointer and touch](../decisions/controls.md)
