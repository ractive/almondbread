---
title: "Controls: keyboard, pointer and touch"
type: decision
date: 2026-10-14
status: accepted
tags: [ui, input, interaction]
---

# Controls: keyboard, pointer and touch

## Context

Deep zooms are explored by moving continuously, so held keys must feel the same at
every depth and stop the moment a key is released. Users also keep the inspector open
while navigating.

## Decision

| Input | Action |
| --- | --- |
| Hold W A S D or arrow keys | pan continuously, 0.4 view widths per second, diagonals normalised |
| Hold Space / Shift+Space | zoom in / out at the zoom target (pointer or keyboard crosshair, see the revision below) |
| Q / E | zoom speed ×0.8 / ×1.25, from 0.1 to 8 e-folds per second (default 1.6, ≈×5 per second); shown in the HUD, highlighted on change |
| + / − | zoom step ×1.5 at the zoom target |
| R, Cmd+0 | reset view |
| Two-finger scroll, drag | pan (Mac) |
| Pinch, Cmd/Option-scroll, wheel | zoom at the zoom target (Mac) |
| Double-click / Option+double-click | zoom in / out ×2 at the click (Mac; switches to pointer mode) |
| Pointer in the edge band while Space is held | pan towards that edge (setting: off / while zooming / always) |
| Drag, pinch, double tap, two-finger tap | pan, zoom, zoom in, zoom out (iPhone/iPad) |
| iPad hardware keyboard | the same keys as the Mac |

## Reasons and rules

- Speeds are in screen units (view widths, e-folds), so motion feels the same at any
  depth.
- Motion is integrated per display frame from the set of held keys, not from key repeat.
- On the Mac, keys come from a window-local event monitor, so they work while an inspector
  control has focus; text fields keep their keys. Handled keys are swallowed (no system
  beep). Losing key-window status, or focus on iPad, releases every key, because the
  key-up may never arrive.
- Continuous zoom stops at the precision limit without drifting the centre; the HUD then
  shows a prominent warning. Measured in [[synthesis/benchmarks]] (interaction section).

## Consequences

The interaction test drives the same key paths with synthetic events. Gesture frames
render a dynamic-resolution preview ([[techniques/frame-reuse-and-progressive]]).

## Revision 2026-10-14: pointer and keyboard modes, crosshair zoom target

Requested by the user; **implemented in M2.5** ([[roadmap]]); as built in the
next section, measured in [[synthesis/benchmarks]] (M2.5).

**Zoom target.** Every zoom (held Space, Shift+Space, pinch, Magic Mouse
or trackpad scroll-zoom, wheel, double-click, + / −) zooms exactly
towards one point, the *zoom target*, shown as a crosshair. The complex
coordinate under the target stays fixed on screen while zooming.

**Two modes, switched by the last input used:**

| | Pointer mode | Keyboard mode |
| --- | --- | --- |
| Entered by | moving the mouse or trackpad pointer over the canvas (small threshold, so a nudge of the desk does not switch) | W A S D or arrow keys (Q / E and Space alone do not switch) |
| Crosshair | the system cursor becomes a crosshair over the canvas (standard arrow over the inspector, HUD and title bar) | the system cursor is hidden; an overlay crosshair glides to the view centre with a critically damped spring (about 200–300 ms, no overshoot, frame-rate independent), and the zoom follows the crosshair while it glides |
| Zoom target | the pointer position | the overlay crosshair (the centre once arrived) |
| Panning | drag / two-finger scroll moves the image under the pointer | W A S D / arrows move the image; the crosshair stays centred |

Arrows and W A S D are equivalent everywhere.

**Edge panning (pointer mode).** While a zoom is running (Space held)
and the pointer is within an edge band of the canvas (about 8 % of the
smaller dimension), the view also pans towards that edge, with speed
rising smoothly from 0 at the inner border of the band to the WASD pan
speed at the edge, diagonal in corners. Off when no zoom is running, so
reaching for the inspector, menus or Dock never moves the image. A
setting offers off / while zooming (default) / always. When the pointer
leaves the window, edge panning stops.

**Touch (iPhone/iPad).** Unchanged: the pinch centroid is the zoom
target. With an iPad trackpad or mouse, pointer mode applies (iPadOS
pointer interactions), so the crosshair behaviour carries over.

**Testability.** Mode switches, the spring and edge panning are driven
per display frame from the model, so the background interaction test can
script them (synthetic pointer moves and key events) and assert the
fixed-point property: the complex coordinate under the zoom target
drifts by less than one pixel per second of zooming.

## As built (M2.5)

All policy is in the Rust core's `Navigator` (`crates/almondbread-core/src/input.rs`);
the platform views decode events and present what each `frame` call returns.

- **Mode switching.** Pointer mode is the default. A W A S D / arrow key-down enters
  keyboard mode; Q / E, Space, Shift, + / − and R do not. Pointer travel of more than
  6 points from where the pointer was when keyboard mode began switches back (leaving
  and re-entering the canvas measures from the re-entry point, so a window switch is not
  travel). A double-click, a touch pinch or a drag is pointer use and switches too; a
  two-finger trackpad scroll (pan) and a trackpad pinch do not move the pointer and
  keep the mode.
- **Zoom target.** Keyboard mode: the crosshair. Pointer mode: the pointer while it is
  over the canvas (the view minus the window chrome's safe-area insets and minus the
  HUD), else the view centre; gestures that report a position (pinch, wheel, scroll
  zoom) use that position in pointer mode and the crosshair in keyboard mode. Menu
  zoom (⌘= / ⌘−, ×2) uses the zoom target. `Viewport::zoom_by` keeps the point fixed
  exactly; the interaction test measures 0 px drift over 2.5 s of zooming.
- **Glide.** On entering keyboard mode the crosshair starts at the previous zoom
  target with zero velocity and follows a critically damped spring to the centre,
  integrated in closed form per frame (`x(t) = (x0 + (v0 + ωx0) t) e^(−ωt)`), so it is
  exactly frame-rate independent and, starting at rest, never crosses the centre.
  ω = 25 /s: 10 % left after 156 ms, 1 % after 265 ms; it snaps onto the centre below
  0.02 pt. The zoom target is the crosshair's current position every frame.
- **Edge panning.** Band = 8 % of the canvas's smaller side; per edge the strength is
  `smoothstep(1 − d / band)` (0 at the inner border, 1 at the edge, zero slope at
  both ends), x and y combined and normalised to at most 1 in corners; speed = strength
  × the key pan speed (0.4 view widths per second). Active only in pointer mode with the
  pointer on the canvas; the setting (inspector, Navigation, persisted in user defaults)
  is off / while Space is held (default) / always.
- **Starting motion from idle.** The first frame after an event that starts motion
  integrates from the event's time. Before M2.5 it integrated up to the 0.1 s cap from
  the last frame before the display link paused: every key press from rest jumped by
  0.1 s of motion (4 % of the view width for a pan key).
- **Presentation (macOS).** Pointer mode: `NSCursor.crosshair` through cursor rects over
  the canvas (safe area minus the HUD), so the arrow stays over the title bar, toolbar,
  HUD and inspector. Keyboard mode: `NSCursor.setHiddenUntilMouseMoves(true)` on entry
  and again on each sub-threshold pointer move, and an overlay crosshair. Window tests
  never touch the cursor.
- **Overlay crosshair: a Core Animation layer, not Metal.** A sublayer of the
  `CAMetalLayer` (two `CAShapeLayer`s: 1.25 pt white arms with a gap and a centre dot
  over a 3.2 pt black halo at 50 % with a soft shadow path; readable on black, white,
  grey and saturated palettes; positioned on the pixel grid, actions disabled), moved
  after each `frame` call (its result bits say keyboard mode; `crosshairX/Y()` give the
  position). Drawing it
  into the Metal frame was rejected: a glide while the image is idle would need a full
  colorize and present per refresh, the crosshair would end up in the shown-frame
  checks and the self-test, and the layer reaches the screen in the same display
  commit as the call that placed it (the image can trail by the frame in flight, which
  is invisible for a marker the image moves under). Cost: a few µs per frame.
- **iPad.** `UIHoverGestureRecognizer` feeds pointer positions (trackpad, mouse);
  `UIPointerInteraction` shows a crosshair-shaped pointer in pointer mode and hides it
  in keyboard mode (the HUD region keeps the system pointer). Touch is unchanged: the
  pinch centre is the target.
- **Tests.** Rust unit tests (mode switching, glide convergence and no overshoot,
  frame-rate independence at 30 / 60 / 120 Hz against the closed form, the fixed point
  for every zoom input in both modes, the edge-pan ramp, corners, insets and setting,
  motion from idle). `--interaction-test` scenarios `pointer space`, `keyboard glide`
  and `edge pan` ([[synthesis/benchmarks]], M2.5).
