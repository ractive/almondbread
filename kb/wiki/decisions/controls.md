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

## Revision FL (2026-10-14): motion feel and gamepad, as built

All in the core `Navigator` (`crates/almondbread-core/src/input.rs`); platforms forward raw
events. Measured in [[synthesis/benchmarks]] FL and judged on the review videos.

- **Eased velocities.** Held keys, edge panning and the gamepad set a *target* velocity (pan
  in points/s, zoom in e-folds/s). The actual velocity follows on a critically damped spring,
  integrated in closed form per frame together with the exact distance travelled, so motion
  is identical at 30, 60 and 120 Hz (tests: 1e-9 for pan alone; pan coupled with zoom is
  applied at the step's midpoint scale, second order). Speeding up ω = 30 /s (half the key
  speed after 56 ms, 90 % after 130 ms), slowing down ω = 18 /s (a released key glides on
  2/ω = 0.11 s worth of its speed: 4.4 % of the view width at the key speed). The zoom rate
  uses ω = 25 / 18 /s: releasing Space zooms on by 0.18 e-folds (×1.19) at the default speed.
- **Direct manipulation stays 1:1.** Drag, trackpad scroll and pinch move the view exactly
  with the hand. On release (`GestureEnd`: mouse up after a drag, magnify phase ended, touch
  pan/pinch ended) the view flings on with the velocity of the last 0.1 s, decaying with
  τ = 0.33 s (pan) / 0.25 s (zoom); no fling if the hand rested more than 60 ms before
  release. The macOS trackpad sends its own momentum-phase scrolls after the fingers lift;
  those arrive as ordinary pans, so no second fling is invented (no `GestureEnd` for
  scrolling). A new touch, drag or direction key stops a fling.
- **Animated zoom steps.** Wheel notches (new event `WheelNotch`; precise scroll-zoom stays
  1:1), double click/tap, + / − and menu zooms are exponential ease-outs (τ = 60 ms) at the
  zoom target, summed when they overlap; the fixed point holds (tests: drift < 1e-6 px).
- **Gamepad.** GameController framework on macOS and iOS (`Gamepad.swift`) sends raw state
  (`Gamepad { lx, ly, rx, ry, lt, rt, boost }`) on every change; Rust applies a radial stick
  dead zone of 0.15, trigger dead zone 0.06 and the response curve `0.25 m + 0.75 m³` of the
  rescaled deflection. Left stick pans like W A S D, right trigger / left trigger zoom in /
  out, right stick up/down zooms too, A boosts both ×2.5, the shoulders are Q / E, Y resets.
  Deflection switches to keyboard mode (crosshair target). A disconnect sends a centred
  state, so motion eases out. Window tests never read controllers.
- **Interaction-test changes.** Edge pan expects the eased start (−2/ω of a second) and a
  glide of 2/ω_decel of speed after the pointer leaves (measured 53.0 pt vs 54.2 expected);
  the keyboard-glide fixed point is measured after the tapped key's pan has glided out.

## Revision 2026-10-14: control center, orbit overlay and mini-map, as built

Requested by the user ("show the c value as a dot plus the first n iterations as lines",
interior colourings, "a settings menu or a control center", then a mini-map "where you
actually are", with nested levels and click-to-jump). Measured in [[synthesis/benchmarks]]
("Overlays").

**Control center** (`Sources/AlmondbreadKit/ControlCenterView.swift`, replaces the inspector).
Sections in order: Places, View & navigation, Journey & cinematic effects, Look & colour,
Lighting & outline, Interior, Overlays, Iterations & quality, Controls. The sidebar holds only
what can be set (user, 2026-10-16): no live values. The HUD shows zoom, centre and speed; the
renderer statistics (iterations, GPU, render size, reference, kernel, state; the former
"Performance & diagnostics" section) are the HUD's developer details, off by default
(Overlays, View menu, Settings). On the Mac the sections are a lazy stack styled like a
grouped form; Places thumbnails show only their zoom, in a small dark pill at the bottom right,
with the name as the tooltip (user, 2026-10-16: the caption strip cut names to "The Wh…"); iOS
has no hover, so a tap selects and names the place under the grid and a long press opens a
menu titled with the name (Go, Fly) ([[platform/swiftui-performance]]). A search field at the top
filters sections by title and keywords ("dense", "orbit", "aa", ...); headers collapse
(remembered). A trailing sidebar on the Mac and iPad, so nothing covers the image; a sheet on
iPhone (medium/large detents, translucent material). Toggled by the toolbar button and ⌘I
("Show Control Center"); the toolbar also has Orbit and Mini-map toggles. App-wide preferences
are in the macOS Settings window (⌘,): restore colours and overlays at launch, HUD, zoom units
(×-magnification or 10^n), edge panning, game controllers, reset to defaults. Per-window
settings (look, colour, interior, quality, overlays) persist in user defaults as one JSON
record written half a second after the last change and restored at launch (window tests
start from the defaults). Places is a placeholder for the places catalog being built
separately (`PlacesSection`, `PlacesProvider`); the palette picker is a swatch grid separate
from looks, and `Explorer.setCustomPalette(stops)` takes any palette as stops for a palette
library.

**Orbit overlay.** c is the zoom target (the pointer in pointer mode, the crosshair in
keyboard mode or during autopilot), drawn as a ring and dot; `z_1 ... z_n` (n = 1 ... 1000,
default 50; O toggles) as dots joined by segments, warm to cool and fading along the orbit,
each over a dark halo so it reads on any palette. The orbit is computed in Rust
(`core::orbit`): shallow views iterate c in f64-mantissa floatexp; deep views perturb
against the renderer's reference (`z = Z_m + δ` with rebasing), so iterates that come back
near c (after a period, where the reference rebases to m = 1) land exactly. The fate of c is
classified like the kernels classify pixels (derivative test, atom domain, period by
closure among divisors of q with `|λ| < 1`, see [[techniques/coloring-methods]] "Interior
colouring as built") in steps of ~1.5 ms per frame, and shown under the mini-map: "escapes at
n 37", "period 3 |λ| 0.412 arg +0.250", "unresolved at n ...", with `|z_n|`.

**Drawn in Rust/Metal, not SwiftUI** (`crates/almondbread-render/src/overlay.rs`, kernel
`overlay`). The scene is vector primitives in drawable pixels (segments, discs, rings, rounded
rects, map images, glyphs of a 5×7 bitmap font with box-filtered coverage), binned on the CPU
into 16×16 tiles (segments are walked, so a long line touches a band of tiles); a thread per
pixel blends its tile's primitives in order by analytic coverage (1-pixel ramp of the signed
distance): anti-aliased and crisp at any backing scale. After a compose pass only the tiles
with primitives run, reading the new history; when only the overlay changed (the pointer moved
over a resting view) a full pass copies the history to the drawable with the overlay. The
history never contains the overlay, so reprojection, shown-frame checks and exports are
unaffected; with the overlays off nothing is encoded. Unlike the crosshair (a Core Animation
layer, M2.5) the overlay changes the drawn frame, which is the point: a Windows or Linux UI
gets it for free.

**Mini-map** (M toggles; size, corner and levels in the control center). The whole set
(level 0) plus up to three *breadcrumb* levels at multiples of 1000× (the deepest
`levels − 1` that are still at least 30× shallower than the view), in a row from the chosen
corner, each in a translucent rounded panel with its magnification as a label. Each level
marks the next level's area (the deepest marks the view) as a rectangle, switching to a
crosshair once the area is under ~6 points. A level keeps its centre until the view leaves the
middle 60 % of its panel; then it is re-centred and rendered again. Each level is rendered by
a small renderer of its own (64 K live slots, 2 ms chunks, one in flight, 4 AA samples, no
interior analysis) at the panel's pixel size; renderers of dropped levels are reused (making
one compiles pipelines). They are stepped every refresh while the main view rests and every 8th during a gesture or the autopilot (so the maps fill in during a long flight).
The orbit overlay draws the orbit at full-set scale on level 0.

**Click to jump.** A click (tap on iOS) on a panel maps the pixel to c with the level's
precision and jumps there keeping the current zoom. When the map pixel is more than 4 view
pixels the clicked point is underdetermined; landing on an arbitrary point of the pixel usually
shows smooth exterior. Then, on a thread, ball period detection finds the lowest-period
minibrot touching the pixel, Newton finds its nucleus at the view's precision, and the jump
goes to the minibrot's cusp, nucleus + size/4 along its orientation (`minibrot_size_complex`),
a boundary point with structure at every depth; the note under the map says what was done
("snapped to the period-p minibrot's cusp near the click"). No minibrot found: the pixel's
centre. Option-click flies there instead (the fly-to path: out, across, in, landing at the current zoom).

## Autopilot (as built, 2026-10-14)

Requested by the user ("an auto-zoom, like an autopilot, so that you can just navigate with
the mouse"). All policy in the core: `crates/almondbread-core/src/autopilot.rs`, owned by the
`Navigator`; the renderer reports feedback and surveys; Swift decodes events and shows the
HUD.

| Input | Action |
| --- | --- |
| Z, menu "Autopilot On/Off", toolbar ✈ (iOS: overlay button) | on (eased start) / off (eased stop); Z key repeats are ignored |
| Esc, menu "Stop Autopilot or Flight" | eased stop; not handled (the system keeps Esc) while nothing flies |
| Pointer offset from the view centre | steers like a joystick (below) |
| Click (iOS: tap) | pause / resume (eased); a double click undoes its first click's toggle and zooms as usual |
| Q / E, mouse wheel notch, trackpad scroll | speed ×0.8 / ×1.25, ×1.1 per notch, `e^(0.004·dy)` per scrolled point; 0.1–4 e-folds/s, default 0.9 (×2.5/s); momentum scrolls are ignored |
| Space, W A S D, drag, pinch | work as before on top of the autopilot (W A S D: keyboard mode, which flies straight) |
| R | stops it dead (reset) |
| iOS finger | while on, a one-finger drag steers instead of panning; lifting it flies straight |

**Steering.** The pointer's offset `p` (view points from the centre; steering radius R = half
the canvas's smaller side) gives a heading target `h* = p̂ · R · f(|p|/R)` with a dead zone of
0.1 R and `f = 0.35 m + 0.65 m²` of the rescaled deflection. The heading `h` follows `h*` on
a critically damped spring (ω = 6 /s, 90 % in 0.65 s), so a shaky hand does not shake the
view. Each frame zooms about `h` at the autopilot rate z and pans the content by `−k·h` with
`k = (0.5 + 1.0 · z) /s`: the content under the pointer drifts to the middle while the view
dives into it, and the focus of expansion sits at `h (1 + k/z)`, beyond the pointer, which
reads as turning towards it. k grows with z so that turning keeps pace with the dive: a
target can be recaptured while it is within about `R (1 + k/z)` of the centre. When the
pointer is off the canvas or over the HUD, in keyboard mode, or idle for 3 s, the target lock
aims (below; straight ahead, h* = 0, with the lock off); edge panning is off while the
autopilot runs. Rate and heading are integrated in
fixed 2 ms substeps (the pan rate depends on the zoom rate), so 30, 60 and 120 Hz agree to
1e-3 of the distance steered (test `autopilot_is_frame_rate_independent`); the rate spring
alone is exact.

**Speed.** The zoom rate follows `speed · throttle · safety` on a spring (ω = 3 /s up: 50 %
after 0.56 s; 4 /s down: a stop glides on 2/ω = 0.5 s of rate, 0.45 e-folds at the default).

- *throttle* (how the renderer keeps up; `RenderFeedback` from the scheduler every frame):
  the gesture-preview resolution the frame budget affords, `fit = √(budget / (cost per
  texel · pixels))` unclamped; `fit ≥ 1/4` → 1, `fit ≤ 1/10` → 0.3, smoothstep between;
  0.06 while the view waits for a deep reference (frames show only the reprojected history).
  Down with τ = 0.25 s, back up with τ = 1 s. Steering pan is scaled by the throttle too.
- *safety* (the heading region of the current frame): a GPU `survey` pass over each finished
  level (at most every 0.1 s, only while the autopilot is on) counts per cell of a 16 × 10
  grid the sampled texels (≤ 16 × 16 per cell), interior or unresolved ones, and neighbour
  pairs whose class or integer escape count differ. Band crossings per view width =
  edge rate × level width. Around the heading (Gaussian, σ = 0.15 of the smaller side):
  `detail ≤ 3` → safety 0.3, `≥ 12` → 1, smoothstep between; the hint says "interior" when
  most of that region is interior, else "featureless". At the depth limit the zoom stops.
- *HUD*: "autopilot ×2.46/s · now ×1.3/s" plus the reason ("slower: keeping the detail
  sharp", "waiting for the deep reference", "slower: heading into the interior",
  "slower: featureless ahead", "paused · click to resume").

**Assist** (2026-10-14, removed 2026-10-15). The survey's detail-weighted centroid around
the heading leaned the heading target towards boundary detail while the pointer rested. The
user reported that plain Z and Z with assist both end in featureless regions; replaced by the
target lock below.

### Target lock and rescue (as built, 2026-10-15)

**Why the assist failed.** Zooming continuously towards almost any point ends in smooth
exterior or interior: the boundary has no thickness, so a generic point sits a finite
distance from it, and once the view is smaller than that distance only one side shows. A
centroid follower is a feedback loop on the last frame's detail: the centroid moves with every
frame and is measured late, so it lags, and as the detail thins it has nothing left to lean
towards. Only points *on* the boundary stay busy at every depth: Misiurewicz points (spiral
centres and branch points, where the view is asymptotically self-similar) and minibrots (a dive
towards one passes its embedded Julia stages and ends with the minibrot and its decorations;
see "Journey" for the maths).

**Target lock** (`crates/almondbread-core/src/autopilot/lock.rs`; control center "Autopilot
target lock", on by default, persisted; off = the pure joystick; `ALMONDBREAD_AUTOPILOT_LOCK=0|1`
overrides the setting in the app for A/B tests; `fly --lock 0|1`, script event `{"lock": false}`).
While the user does not steer (pointer in the dead zone, off the canvas or idle for 3 s,
keyboard mode, and no pan, drag, fling or held key for 1 s), the autopilot aims at a target:

- *Search* (on a thread, `autopilot-lock`; synchronous only in tests and the `autopilot` CLI):
  the journey's finder over the view (ball period detection and Newton for nuclei with extra
  bits at depth, minibrot framings and their Julia stages, orbit closest return and Newton for
  Misiurewicz points, boundary texels of a 40 × 40 probe), candidates within 0.45 spans of
  the centre, pre-scored by distance (−1.2 per span) and kind (spirals and Julia stages +0.15,
  branch points +0.1, boundary texels −0.5), the best 6 probed (40 × 40 CPU probe of the end
  framing) and kept when the probe is *aimable*: interior ≤ 0.75 (centre third ≤ 0.9),
  ≥ 3 band crossings per width (centre third ≥ 1). Looser than the journey's landing test:
  sparse filaments are dull to land on but fine to dive through. Searches take 0.01–0.3 s
  (measured 2026-10-15, M1 Pro, release; deeper and denser views more).
- *Ahead*: each target has an end span (its framing). As soon as it is locked, a search runs
  from the view centred on it at that end span, continuing its maths: an approach or a Julia
  stage adds the same minibrot's next stages (nucleus again by Newton at the new precision,
  pre-score +0.6, so a dive follows one minibrot through its Julia stages to the minibrot
  instead of hopping to a deeper one every leg); a Misiurewicz point adds itself 4–7 e-folds
  deeper, refined by Newton at that depth's precision (−0.25: the fallback; other targets win
  when they probe well). The next target takes over 0.7 e-folds before the current end span.
  Pickup searches (nothing locked) snap the view to a grid (span to 1/8 e-fold, centre to
  1/128 span) so views a frame apart search the same view: 30, 60 and 120 Hz flights pick the
  same targets (test `autopilot_lock_is_frame_rate_independent`).
- *Aiming*: when the target changes, the aim point glides from where the heading pointed to
  the new target (0.8 s, smoothstep). The heading target for an aim point p is `g p` with
  `g = (z + λ)/(z + k)`, λ = 1.5 /s, so the point moves at `z (p − h) − k h = −λ p` (zoom about
  h at rate z, pan −k h); the heading spring runs at ω = 20 /s while the lock aims. The first
  version aimed `h = p` through the pointer spring (ω = 6): at 4 e-folds/s it oscillated
  (the zoom amplifies the spring's lag at rate z) and lost the target within seconds
  (`almondbread autopilot` at the 1e-50 start: target offsets swinging ±450 points, then
  lost; with the gain law it holds within a point). A far aim point (beyond 0.5 R) slows the
  zoom, to 0.3 at 1.2 R, so the turn can catch it.
- *Pointer priority*: any deflection past the dead zone, or manual input, releases the lock at
  once (the pointer's heading alone, ω = 6); letting go picks up again with a search from the
  current view.

**Rescue.** With the lock on and the user not steering, a heading region that stays uniform
(survey danger > 0.5; without a fresh survey: the last search found the view uniform) for 1 s
with no target, or 2.5 s with one (a target that leads nowhere), eases the zoom into reverse at
0.6 × speed (0.5–1.5 e-folds/s, through the rate spring; × (1 + t/2 s) after t s of it, at most
3 e-folds/s, so a deep interior start does not take half a minute) and keeps searching the widening view
(every 0.6 s or 0.5 e-folds); the first target found ends it and the dive resumes. It also ends
after 0.6 s of detail in the survey's heading region (target or not), and when the whole set
is in view (span ≥ 4). Ended that way without a target, the lock aims at the survey's busiest
cell (band crossings × (1 − interior), Gaussian preference for the centre, σ = 0.35 side)
for 2 e-folds and picks again when that is spent: a chain of short legs towards detail the
GPU shows, for views the CPU search cannot resolve. The dropped target is remembered (the last 4; none within 2 of its end
span is taken again): without that, the sea-8007 interior start dived into the minibrot,
backed out, found its framing again and dived in again. *Limit*: the CPU probes stop at
30 000–60 000 iterations, so views near minibrots of period ≳ 10 000 (sea-8007, the
period-32412 spiral) look like interior to the search: few targets there, and without a fresh
GPU survey (levels that never finish) a spurious rescue; the survey-detail exit bounds it. A deflected pointer ends it at once: steering into a
void on purpose is allowed (the safety still slows down there). HUD: "Autopilot · towards spiral
M(68,2)" and "backing out: nothing ahead".

**Tuning on review flights** (`scripts/flights/autopilot-seahorse.json`, `almondbread fly`
with a scripted pilot that keeps a target under the pointer, clamped to the window):

- The first version panned at a fixed k = 0.8 /s. At ×6/s the pilot lost the seahorse
  target after a 2 s look-away and never caught it again (the target escapes once it is
  beyond `R (1 + k/z)`, about 1.4 R at z = 1.8). k now grows with z.
- A pilot with 0.3 s lag and gain 2.5 oscillated with the stronger turning (two lags in the
  loop: the hand and the heading spring); with 0.15 s lag it holds the target within a few
  points of the centre down to 1e-30. A real hand adapts; the numbers are for the script.
- Looking away for 2 s at ×6/s still loses a target: it is ~36× further out when you look
  back. That is the physics of a dive, not a bug; slowing down (Q, scroll) or pausing
  (click) is the remedy.
- The first heading straight into the main cardioid from the home view slowed to 0.3 with
  the "interior" hint, as designed.

### Survey steering (as built, 2026-10-17; opt-in)

User report: at deep zoom (around 1e247 at re −1.76866783140550194…, im 0.00164555778036987…)
Z "often flies into the void"; asked for a simple, fast "where is even something" steering,
not a smarter "what is interesting" search.

**Diagnosis** (`fly --out null --log`, `ALMONDBREAD_LOCK_TRACE=1`, 40 s from 1e240 at that
point, 1600 × 1000; measured 2026-10-17, M1 Pro, release, shared machine). The lock's maths
finds nothing there: every candidate of every search is a probe "boundary detail" texel (no
nucleus or Misiurewicz point resolves), and most probe to detail 0–1 at their end framing
(gain 3–6 e-folds: a texel of the 40 × 40 probe is on the boundary only to 1/40 of the span,
so 4 e-folds deeper it is off it). In the failing runs a detail target was followed to its end
span, the search ahead and the pickup found nothing, and the lock kept aiming at the spent
point: detail in the heading region fell from ~20 to ~2 within a second, safety 0.3, a
rescue after 2.5 s (zoom reversal 1e248.2 → 1e247.2), then a survey pick. So: not too slow
and not wrong in place, but the target runs out (probe precision of the fallback target) and
nothing steers while the lock has no usable target. Baseline counts over 5 + 5 interleaved
runs below; typical bad run: 4.3 s uniform, 1 rescue, 3–6 s without a target.

**Steering.** `ALMONDBREAD_SURVEY_STEER=1` (off by default until compared in the foreground):
whenever the user does not steer and no lock target is followed (none, search pending, a
target past its end span with no next one, or a target whose heading region has been uniform
for 0.4 s, which is then dropped into the tabu list), the heading aims at a point from the GPU
survey:

- The survey kernel also returns per cell the escaped sample with the highest escape count and
  its texel (second `uint4` per cell; negligible cost, it already reads those texels). The
  escape count rises towards the set, so this is the cell's sample nearest the boundary, at
  survey resolution (16 × 16 samples per cell).
- Pick: among cells with ≥ `DETAIL_HI` band crossings and escaped samples, the best of
  detail (capped at 4 × `DETAIL_HI`) × escaped share × Gaussian of the distance from the
  current aim (σ = 0.25 of the shorter side); none detailed: the highest peak in view
  (uphill in escape count). The pick is stored as a complex point using the surveyed view
  (`Survey::viewport`), so the zoom and pan since the survey do not shift it.
- Hold: the pick is kept while each new survey finds the region around it non-uniform (the
  heading test: danger ≤ 0.5, interior ≤ 0.5); then the next pick is made near it. The first
  version re-picked the nearest detailed cell on every survey: near the detail threshold it
  hopped between cells 10 times a second, oscillated in a sparse region and ended in a rescue
  (one run 20.6 % uniform, 2 rescues).
- The aim point eases towards the pick (τ = 0.3 s, exact for any frame rate between surveys)
  and is aimed like a lock target (gain law, ω = 20, capture slow-down). HUD label "towards
  the nearest detail". A lock target found later takes over at once; a deflected pointer wins
  as always; the rescue waits the "locked" 2.5 s while survey steering aims. It replaces the
  post-rescue "detail in view" pick while on.

**Measured** (interleaved base/survey runs, same frozen binary, loads 5–110):

| Path | Config | uniform s (mean) | rescues (total) | zoom gained (mean decades) |
| --- | --- | --- | --- | --- |
| user point, 90 s, 5 runs | off | 9.3, 14.2, 0, 4.6, 5.3 (6.7) | 6 | 25.4 |
| user point, 90 s, 5 runs | on | 0.9, 0.3, 4.3, 0.4, 0.8 (1.3) | 1 | 26.8 |
| user point, 40 s, 5 runs | off | 4.3, 2.0, 0.6, 0, 0 (1.4) | 1 | 13.8 |
| user point, 40 s, 5 runs | on | 0, 0.4, 0, 0, 1.3 (0.3) | 0 | 13.1 |

Time without a lock target or survey aim: 2.4–4.9 s per run → 0. Seahorse medallion (1e8),
needle minibrot 1e91, junction 1e100, starburst 1e112: 0 % uniform and no rescue with and
without (4 runs per path and config, 3 of them with the first version; the lock always had a
target there, survey steering never engaged), the
same zoom reached within run-to-run noise. Headless `almondbread autopilot` (CPU survey of 16
× 16 cells of 5 × 5 texels every 0.25 s) at the user point: uniform 15.6 % → 10.6 %, still one
rescue: the coarse CPU survey is a much weaker guide than the GPU one. Unit tests:
`boundary_point_prefers_detail_near_the_heading_else_uphill`,
`survey_steering_brings_the_boundary_point_to_the_centre_at_any_frame_rate` (30/60/120 Hz
agree within 3 points and 0.02 e-folds), `lock_target_and_pointer_win_over_survey_steering`.
Foreground check: `open --env ALMONDBREAD_SURVEY_STEER=1 <Almondbread.app>`.

## Fly to coordinates (as built, 2026-10-14)

Requested by the user ("give it a coordinate and it automatically zooms in there; control the
speed with Q and E"). Core: `crates/almondbread-core/src/flyto.rs` (path and flight),
`Navigator::fly_to`; FFI `Explorer.fly_to(re, im, scale)` / `fly_to_coordinates(text)`.

| Input | Action |
| --- | --- |
| ⌘G "Fly to Coordinates…" | sheet with re, im, scale (decimal strings, any depth); pasting the copied-coordinates text into a field fills all three; prefilled from the clipboard |
| ⇧⌘G "Fly to Copied Coordinates" | flies to the clipboard's view (⇧⌘V still jumps) |
| Q / E, wheel notch, trackpad scroll | flight speed ×0.8 / ×1.25 live, 0.1–16 e-folds/s at the zoom ends, default 2 |
| Esc | eased stop on the path (ω = 4 /s), the view stays there |
| any navigation (keys, Space, drag, pinch, wheel zoom, double click, gamepad, Z) | takes over: the flight's velocity is handed to the pan and zoom springs, which glide out |
| Go to coordinates, R | cancel the flight |

**Path** (src: [[sources/van-wijk-nuij-smooth-zooming]]). Van Wijk & Nuij's optimal
zoom-and-pan path with ρ = √2 in (u, w) space (u along the line to the target, w the view
width): zoom out as far as the distance needs, pan, zoom in; at its ends a pure zoom at
`d ln w/ds = ±ρ`. Deep views need three changes, all ours:

1. **Units and logs.** Everything is in units of a power of two near max(w0, w1, d); widths
   are kept as `ln w`, and `ln cosh`, `ln sinh` are evaluated without overflow.
2. **Head and tail.** f64 cannot hold w1/w0 = 1e-3000. The path is a head (pure zoom out
   about the start centre until w = 1e-6 · d), the van Wijk part between widths f64 holds,
   and a tail (pure zoom in about the target from 1e-6 · min(w, d) to w1). Head and tail zoom
   at exactly the van Wijk end rate ρ per unit of s, and the van Wijk part's centre moves
   ~1e-6 view widths per unit at its ends, so the joins are seamless.
3. **Exact ends, no drift.** Every frame's view is computed from the path parameter, not
   integrated. The travelled distance `u(s) = w0/ρ² · sinh(ρs) / cosh(ρs + r0)` and the
   remaining offset `d − u(s) = w0 cosh r0 / ρ² · sinh(ρ(S − s)) / (cosh r1 · cosh(ρs + r0))`
   are both cancellation-free; the centre is the start centre plus u (first half) or the
   target minus the remaining offset (second half), in arbitrary precision, so the f64
   offset is always small next to the view. At s = S the view is the target, bit for bit.
   Targets within 1e-100 of a view width of the centre line zoom straight.

**Motion.** The path parameter's speed follows `speed / ρ · throttle` (the autopilot's
renderer back-off) on a spring (ω = 3 /s up, 4 /s down), capped by `√(2 a s_left)` to
brake at a constant deceleration onto the target, snapping within 1e-4 units;
`a = max(2 units/s², set speed / 0.75 s)`. (A fixed 2 units/s² made fast flights brake for
most of a short path: the interaction test's 15 e-folds at 12 e-folds/s had not landed after
2.9 s.) **Reference
prefetch:** when the target is inside the current view and deeper, a needed big reference is
requested as a jump to the target would request it (nucleus search around the target, the
target's precision and scale), and it then serves most of the rest of the dive
([[techniques/rebasing]]): 2 references for home → sea-8007, 6 for home → 1e-300. The first
version asked for a plain centre reference at the target (nucleus search off, kernel picked
at the current depth); the settled frame at sea-8007 then showed smooth wrong fans around the
minibrot, with 35.8 % of escape counts different from a fresh render, while the class check
of `verify --history flyto` passed. The session now also fails when more than 1 % of escape
counts differ from a fresh render (it is 0 % now).

**Tests.** Unit tests: path starts on the start view (< 1e-6 pt) and ends exactly on the
target, continuity over 4000 steps (zoom and centre steps bounded), the target moves
monotonically towards the centre once on screen (home → sea-8007), deep → far deep
(1e-300 at c = i → sea-8007 zooms out to ~1e-3 and back), nearby targets, zoom-out, pure
zoom, 30/60/120 Hz flights agree to 2e-3 and land exactly, Esc glides 2/ω of speed, the
navigator lands with `viewport == target`, and a pan hands the motion over without a jump.
`verify --history flyto` (in `make regress`) flies through the interactive scheduler and
diffs the landed frame against the CPU oracle and a fresh render.

**Stalls (2026-10-16, "a fly-to sometimes pauses mid-flight").** Measured per refresh with
`fly --log` on eight flights (home to sea-8007, junction-1e300, needle-minibrot-1e302 and
spiral-1e1000 at the default speed and at ×9 Q, junction-1e300 to spiral-1e1000): the motion
model never stops (fly-to's own `waiting_reference` throttle of 0.06 never engaged: the target
reference, requested once the view passes 1e-12, came in 26 ms as a progressive prefix; the
renderer back-off slows it to 0.3 of the speed at most, where previews are at 1/8). Two things
did freeze the picture: (1) frames of 165–310 ms CPU where the flight first needed a new kernel
variant (BLA, LA, the deep kernel) with a cold Metal shader cache, fixed by the pipeline
warm-up ([[platform/metal-compute]] "Pipeline warm-up"); a frame that long also made the
scheduler see a jump (zoom ×20 between keys) and reset its learned state; (2) refreshes with
nothing presented for up to 250 ms while two colorize passes waited on a GPU that other
processes kept 83–100 % busy (our own chunks were ≤ 32 ms): load, not the app.

**Reset (2026-10-15, user request).** "Reset all…" at the bottom of the control center asks
"Reset settings" or "Reset settings and view" and puts every per-window setting (look,
palette and mapping, lighting and outline, interior, overlays, iterations and quality, edge
panning, autopilot target lock; with the view: the whole set) back to the defaults
(`Explorer.defaultSettings()`). Each section header has a small ↺ that resets that section
alone. Both are undoable (Edit ▸ Undo, ⌘Z: the previous settings record and view go on the
window's undo manager). The Settings window's reset of app-wide preferences stays separate.
Places, autopilot and fly-to are in the control center since the integration (Places: grid by
category, Go / Fly / Random; View & navigation: Autopilot, Fly to…, Copied, target lock).

**Deep high-period minibrots (2026-10-16).** At sea-8007 (period 8007, 3.2e-31 across) the
lock found nothing: its 40 × 40 CPU probe ran at the CPU cap of 30 000 iterations, where every
texel near that minibrot is unresolved (the app's auto limit settles at ~118 000), so the view
read as interior, the finder (which needs boundary texels) returned 0 candidates, and the
autopilot backed out (rescue) or dived into the minibrot's interior. The view probe now
escalates (`journey::probe_escalating`: 4× while more than half the probe never escaped and
the step lets more escape, to 240 000 or 0.5 s), and candidate probes start at the view's
limit. Newton at periods 8007 and 16014 was never the problem (both nuclei were found once the
view resolved). `autopilot --center sea-8007 --scale 2.5e-34 --lock 1 --seconds 60`, whose CPU
survey escalates the same way now: uniform 5.0 % → 0 %, rescue 2.6 → 0 s, 23 → 12 target
switches, 1e52.0 → 1e54.4 (the first search: 0 → 10 candidates); lock off: 94 % uniform.
`ALMONDBREAD_PROBE_ESCALATE=0` restores the fixed limit.

## Journey (as built, 2026-10-15)

Requested by the user: an endless, planned, random-but-seeded flight from landmark to
landmark, with targets chosen from the maths rather than by taste. Core:
`crates/almondbread-core/src/journey.rs` (planner and state machine, owned by the
`Navigator`); legs are ordinary fly-to flights ([[sources/van-wijk-nuij-smooth-zooming]]).

| Input | Action |
| --- | --- |
| J, menu "Journey On/Off", toolbar ✦, control center "Journey" (iOS: overlay button) | start with a random seed (1 … 999 999) / resume after manual input / stop |
| control center seed field, "Daily" | start a given seed; the daily journey's seed is the date (yyyymmdd), the same for everyone |
| Q / E, wheel notch, trackpad scroll | journey speed ×0.8 / ×1.25 (0.1–16 e-folds/s at the zoom ends, default 1.4) |
| any navigation (keys, drag, pinch, wheel zoom, rotate, gamepad, Z) | takes over: the leg's motion glides out in the springs (as for fly-to); the HUD says "paused · J or click to resume" |
| click / tap | pause (eased) and resume |
| Esc, R | stop |

**Legs.** Each leg flies the van Wijk–Nuij path to the target at the journey speed (with the
renderer back-off of fly-to), then rests for `3 s · √(1.4 / speed)` (1–6 s) while the target
refines and anti-aliases, then flies on. The next target is planned on a thread from the
current target as soon as the leg starts (0.01–0.3 s per plan to 1e40, measured 2026-10-15,
M1 Pro, release: `almondbread journey --seed 4711 --legs 18`), so it is ready at landing;
during the rest the scheduler computes the next target's reference in the background
(`Navigator::prefetch_target`; a reference that does not serve the current view is kept as
the *spare* until a view needs it).

**Targets from the maths** (per plan; all randomness from one SplitMix64 stream):

1. A 40 × 40 CPU probe of the view's square (side = the view's shorter-side span), computed
   with the oracle (f64, or perturbation at depth). Boundary texels (neighbours whose class or
   integer escape count differ), weighted by how many texels around them escape well after the
   fastest one, give 10 seeded sample points.
2. At each point: ball period detection over a disc of 0.7–2.5 probe texels, Newton for the
   nucleus at 128 bits beyond the view's precision (deep minibrots in view are 2^-40 … 2^-100 of
   it; at the view's precision Newton failed for every period above ~1000), the complex size
   estimate ([[techniques/reference-selection]]). Candidates: the **minibrot** framed at
   6–18 σ; **embedded Julia stages** at t = 1/2, 3/4, 7/8 of the way from the view to σ in log
   scale and at 2 σ^(3/4) (Heiland-Allen's size estimate,
   [[sources/muency-newton-raphson-zooming]]; the stage order — Julia-like figure, nested
   preimages, minibrot — after [[sources/kisaka-julia-sets-in-mandelbrot]]), aimed 3–18 % of
   the view off the nucleus so the shapes around it double; an **approach** centred on a
   minibrot too deep for one leg.
3. **Misiurewicz points** (first four samples): the orbit of the sample shadows a repelling
   cycle; the (k, p) with the smallest relative `|z_{k+p} − z_k|` over periods 1–12 and the
   sample's atom period (and twice it: deep spirals live in embedded copies whose cycles have
   that period) seeds `find_misiurewicz` at k − 1, k, k + 1 (k ≥ 2: a preperiod of 1 is a
   nucleus). The cycle multiplier `Π 2 z_i` labels it a **spiral** (complex) or a **branch
   point** (real, a dendrite).
4. A **boundary detail** zoom (3–6 e-folds into the sample) as a fallback (never twice in a
   row), and **resurfacing** to a shallow catalogue place ([[synthesis/places]]) when nothing
   else is found. (Until the variety revision below: resurfacing after 6 legs or below 1e40.)

**Scoring with survey-style statistics.** Each candidate's framing is probed (40 × 40) and
measured like the GPU survey kernel measures a frame (interior share, whole and middle third;
band crossings per width) plus two measures added while tuning: the *filament* share (exterior
texels escaping at least 8 iterations after the fastest: how much of the view is near the set)
and the *roughness* (median escape-count step between neighbours). Rejected: interior (> 60 %,
or > 75 % in the middle), featureless (< 3 crossings per width, filament < 10 %, calm cells
> 80 %), noise (roughness > 400: colours change several times per screen pixel). Score:
structure (crossings; filament share rising from 0.15 to 0.45, falling above 0.75), a little
interior for minibrots and stages, minus calm and roughness, plus variety: the same kind family
as the last target −0.45 (twice in a row −0.95 more), a family absent from the last four +0.2,
detail zooms −0.6, a gain away from the leg's seeded preferred gain (4–10 e-folds) −0.06 per
e-fold, the same direction as the last leg −0.15, straight down onto the last centre −0.6, a
place already visited −10, plus a seeded 0–0.25. Up to 14 candidates are probed per plan. The
probes run on the CPU rather than reading the GPU survey of the shown frame: the survey depends
on frame timing; the CPU probe makes a seed give the same journey on every machine.

**Tuning on journey #4711** (looked at as thumbnails of every target and as review videos).
The first scoring (crossings only) flew into dendrite country: thin filaments in wide smooth
gradients at every leg, because integer escape counts change between neighbours even where the
colouring looks smooth. Rewarding the filament share and rejecting "noise" by crossings
(> 0.8 of the width) then rejected exactly the rich views (crossings near 40 of 40), so the
journey only zoomed into boundary details; replacing the noise rule by roughness turned every
view into a dense carpet without shapes. The filament band (0.35–0.75) gives decorated
structures with smooth space around them. At depth, minibrot Newton failed until it ran at
128 extra bits, and every minibrot in view was 25+ e-folds deeper than a leg; the approach and
the 7/8 stage made minibrots and Julia stages appear at every depth ("towards a period-87
minibrot" followed by "embedded Julia (period 87)"). From home, seeds 4711, 7 and 20261015
plan spirals, minibrots, stages, branch points and approaches in alternation, with a
resurfacing after 6 legs (9 in the first review video: one region for its whole 85 s, so 6).

**Variety: dives, cross-overs, novelty (revision 2026-10-15).** The tuned planner still showed
one family of shapes for a minute (journey #4711: six legs of dendritic star filigree under
M(113,10), 12 of 17 stills alike). The reason is the maths, not the scoring: deep in one
region every view is decorated with the same local Julia set, so diving within a region repeats
its shapes however the targets are picked. The journey now plans *across regions*
(`journey.rs`, `journey/regions.rs`, `journey/look.rs`):

- **Regions.** `region_of(c)` (mirror images are one region): the needle (re < −1.4),
  seahorse valley, elephant valley; else the internal angle of `μ = 1 − √(1 − 4c)` (the
  cardioid is `c = μ/2 − μ²/4`) snapped to the nearest p/q (q ≤ 9), *valley* when
  `|μ| < 1 + 0.5/q²`, *limb* otherwise; the period-2 bulb the same way about −1. Families:
  valley (spirals at depth), limb (dendrites, stars), needle.
- **Dives.** A journey is a sequence of dives: a **cross-over** leg to an anchor in another
  region, then 1–2 legs within it in a seeded style: *scenic* (one leg, gains 2.5–5 e-folds,
  cross over beyond 1e10), *medium* (1–2 legs, 4–8, to 1e22), *deep* (2 legs, 11–17, to 1e40;
  never twice in a row, not the first dive). Within a dive, candidates also come from a view
  2–4 e-folds wider (targets beside the current one: a short cross-over in the region). A
  journey started away from home first dives two legs where the user is.
- **Anchors** (cross-over targets): the catalogue's places from 1e0.7 to 1e16 (curated
  framings), and per plan four seeded p/q bulbs (30 % on the period-2 bulb) giving the hub of
  the limb (Misiurewicz M(q+1, 1), Newton from 2.2 bulb radii beyond the nucleus; the bulb
  radius is about `sin(πp/q)/q²`) and the valleys either side of the attachment point
  `e^{iθ}/2 − e^{2iθ}/4`. The bulb itself framed with its antenna was dropped: a dark disc
  beside the cardioid in every review. Pre-score: not this region or the last three (hard),
  −0.5 for one of the last eight, −0.7 for the same family as the last dive, −0.8 for a limb
  on a deep dive, a seeded 0–0.6; after the probe −2 per unit of interior share above 12 %
  (catalogue framings beside the cardioid were the same black wedge every time).
- **Look descriptor** (`Look`, 17 components from the 40 × 40 probe plus two short orbits):
  distance spectrum (quantiles of `log10 |∇ν|`, ν = n + s the smooth count), box-counting
  dimension of the structure mask (|∇ν| ≥ 2), interior, filament, calm and crossings, weak
  orientation measures, colour statistics of the default palette, and the **spiral pitch**:
  the cycle the orbit of a point near the centre shadows (closest return over periods 1–16),
  its multiplier λ = Π 2 z_i, the turn per e-fold `|arg λ| / ln |λ|` (arg folded to
  [0, π/2]). Calibrated on 30 landings of #4711 sorted by eye into four families (dendrite
  carpets, bulbs, thin stars and medallions, spirals): image orientation measures do not
  resolve seahorse spirals 3–4 texels wide (separation of family means ≤ 0.5, also at 96 × 96
  texels); the pitch separates spirals from dendrite carpets (1.2–1.3), the distance spectrum
  and dimension separate thin stars from everything denser (2–3.4); colour is nearly random
  between views of one shape. Weighted distance: within a family 0.24 / 0.45 / 0.77 (10th /
  50th / 90th percentile), across families 0.45 / 0.73 / 1.13.
- **Novelty.** Score + 1.2 · min(d, 0.8), d the distance to the most similar of the last six
  landings (stretched 12 % per leg of age). Rejected: closer than 0.22 to one of the last three
  (near-duplicate); for a dive's second leg closer than 0.33 (the dive goes on only while it
  shows something new, else the journey crosses over).
- **Long legs fly faster.** A leg whose path covers more than 14 e-folds flies only 20 % of the
  excess at the journey speed (`leg_pace`: speed × e / (14 + 0.2 (e − 14))): a cross-over from
  1e40 (~92 e-folds) takes ~21 s instead of ~66 s at the default speed.
- **Colours per dive** (cinematic effects on only): every cross-over moves a palette phase on
  by 0.15–0.4 cycles, eased over the leg's flight (`Navigator::journey_palette_phase`, added
  to the palette offset in `color_params`); a new journey continues from the current phase.

Tests: `journeys_cross_regions_and_do_not_repeat_looks` (seeds 4711, 7, 20261015, 16 legs: ≥ 5
regions, at most 3 legs per region in a row, no landing within 0.22 of the one before),
`novelty_rejects_a_repeat_and_rewards_a_new_look`, `long_legs_fly_faster`,
`regions_of_known_places`, `bulb_anchors_find_their_nuclei_and_hubs`. Tools:
`almondbread journey --describe 1` prints each landing's region, dive and look with its
distances to the last legs; `--save f` writes the targets, `--views f` describes fixed views
(to compare descriptor variants on one set). Results in [[synthesis/benchmarks]] (PJ,
variety).

**Determinism.** A plan depends only on the start view (centre, shorter-side span) and the
chain (seed stream, history), and every leg plans from the previous target, so a journey is
reproducible target for target from the same start; a view near the fitted home view plans
from the canonical whole set (centre −0.75, span 3), so "journey #4711 from home" is the same
in every window. Manual input breaks the chain: resuming plans from where the user left the
view (the seed stream continues). Tests: `journeys_are_reproducible_by_seed`,
`targets_are_on_the_boundary_and_not_interior` (minibrot nuclei are periodic to the view's
precision; every target has boundary structure in its middle and < 60 % interior),
`journey_state_machine_flies_dwells_and_flies_on`, `journey_flies_legs_and_manual_input_takes_over`.

**HUD and tools.** "Journey #4711 · leg 5 · towards a period-208 minibrot · 1.0e20× ·
flying|resting|planning" and "journey speed ×4.06/s". `almondbread journey --seed n
[--legs k] [--thumbs dir]` prints a journey's plan (and renders each target); `fly` scripts
take `{"journey": 4711}` (`scripts/flights/journey-4711*.json`).

**Revision 2026-10-16: deeper dives, faster through monotony.** User reports: "a slow,
boring stretch along the needle" and "dives that stay too shallow". Criteria used: the median
dive (cross-over plus its legs, deepest landing) at 1e10 or deeper across seeds, and the
longest flight stretch whose look does not change below ~2 s at the default speed.

- *Depth.* With gains of 2.5–5 / 4–8 / 11–17 e-folds (scenic / medium / deep), one or two
  medium legs, and minibrots searched only where the lowest-period atom touches a disc of
  0.7–2.5 probe texels (shallow ones), five of six dives of journeys #7, #4711 and #20261016
  ended above 1e10 (median dive 1e4.5–1e7.2). Now: gains 4–8 / 7–12 / 13–19, medium dives
  always two legs, style odds 25 / 40 / 35 % (deep still never twice in a row), legs shallower
  than the style's lower gain cost 0.12 per e-fold, and every other sample of a dive's
  period search uses a disc smaller by `e^-(mean gain − 6)` (deeper minibrots). Six seeds × 16
  legs: median dive 1e8.4–1e16.9 (median of medians 1e11.1), deepest 1e17.7–1e26.2. Played
  for 150 s: median zoom 1e3.1 → 1e7.5 (#2) and 1e3.3 → 1e8.9 (#20261015); time deeper than
  1e10 32 → 50 s and 0 → 68 s.
- *Monotony.* A dive into a self-similar spot (Pine Compass on the needle, re −1.4304) shows
  the same picture for 19 e-folds; my first dullness test (no structure: interior, calm, few
  crossings) did not flag it, because the needle's views are full of escape bands. The look
  descriptor does: its distance per e-fold of zoom was 0.05–0.15 there against 0.2–0.5 in
  varied dives. Each plan now probes its leg's path at 32 equal steps of the path parameter
  (`journey::path_profile`, on the planning thread); a sample is dull when its probe has no
  structure or its look changed less than 0.15 per e-fold over the last 2 e-folds; runs of 4
  or more dull samples fly at 3× (`DULL_PACE`, through the navigator's speed spring; braking
  onto the target is unchanged). Seed #20261015, 16 legs: flight 162 → 143 s at the default
  speed, the longest dull stretch 5.9 → 2.0 s (the Pine Compass tail); seed #2's cross-over
  out of the needle 7.5 → 2.5 s. `almondbread journey --path 1|2` prints each leg's profile.
- *Black mid-flight frames* were not reproduced: five journeys played headless (seed 4711 for
  85 s, seed 20261015 with the cinematic effects for 3 min, seed 7 from the needle for 3 min,
  seeds 2 and 20261015 for 150 s before and after the change), 62 700 frames at 60 fps through ffmpeg `signalstats`: no frame with its 90th-percentile luma below
  20 and no flash (mean luma below 35 % of the ten frames around it).

## Zoom-out autopilot (as built, 2026-10-17)

Requested by the user: the autopilot in reverse, a scenic flight back to the whole set that can be
stopped anywhere, not a quick reset. Core: `Navigator::start_zoom_out` (`crates/almondbread-core/src/input.rs`),
a fly-to flight ([[sources/van-wijk-nuij-smooth-zooming]], `flyto.rs`) with a soft landing.

| Input | Action |
| --- | --- |
| ⇧Z, menu "Zoom Out to the Whole Set (⇧Z)" (iOS: long press on the plane button) | on: eased start towards the home view fitted to the window |
| Q / E, wheel notch, trackpad scroll | the autopilot's speed (shared with Z: 0.1–4 e-folds/s, default 0.9) |
| Esc, Z, ⇧Z | eased stop where it is (Z does not start the autopilot) |
| any navigation (pan, drag, pinch, wheel zoom, keys) | takes over as from a fly-to (the motion glides out in the springs) |
| R, J | reset / journey as usual (the zoom out ends) |

**Path and pace.** The van Wijk–Nuij path from the view to the home view: from any depth mostly
the head (a pure zoom out about the view's centre, `ln w` only, so 1e-3000 does not overflow), then
the van Wijk part drifts the centre to −0.75 while it widens. The path parameter runs at the
autopilot speed / ρ, so the zoom rate is the autopilot speed in e-folds/s at the ends (measured in
the test: the rate halfway is within 2 % of the set speed from 1e-300 and 1e-3000), throttled by the
renderer back-off like every flight. Bounded: one closed-form path evaluation per frame, no loop over
the depth. At the default speed 1e-3000 → home takes 6908 e-folds / 0.9 ≈ 2.1 h, at 4 e-folds/s
≈ 29 min (E); the user asked for a constant-speed flight, so it is not faster at depth.

**Soft landing** (`Flight::soft_landing`, `SOFT_LAND_TIME` = 2.5 s). The fly-to landing brakes at
`max(2, v/0.75 s)` on `v = √(2 a s_left)` through the ω = 4 /s spring; the spring lags the curve, so
a flight reaches the end still moving and snaps (the first version: 2.3 → 0.75 e-folds/s in the last
frame at 4 e-folds/s). The soft landing decelerates at `v / 2.5 s` and, once the spring would outrun
the curve, rides it in closed form (`√s_left` falls at `√(a/2)` per second), so the last e-fold or so
eases out evenly to rest, identically at 30, 60 and 120 Hz. Resizing during the zoom out replans the
path to the new home view.

**HUD.** "Zooming out to the whole set · 37 % · (back-off hint)" and "zoom-out speed ×2.46/s";
FFI `FlightStatus.zoom_out`.

**Tests.** `zoom_out_autopilot_lands_softly_at_home_from_any_depth` (1e-300, 1e-3000 at 4
e-folds/s: monotone, lands bit-exactly on the home view, flight time = e-folds / speed + < 6 s,
the last second decelerates monotonically from below 0.8 × speed),
`zoom_out_autopilot_is_frame_rate_independent_and_stops_on_request` (30/60/120 Hz agree to 1e-3
e-folds after 6 s; Q/E; Esc, Z, ⇧Z glide 0.05–1 e-fold and stop on the way; a pan takes over; R).
Interaction test scenario "zoom out" (⇧Z from 1e5: 1.28 e-folds in 2.4 s, Esc stops; 102 fps
presented at 120 Hz, frame vs fresh render 0.000 %; measured 2026-10-17, release).

## Journey with stops (as built, 2026-10-17)

Requested by the user: a journey mode that stops at each landmark until you choose how to go on.
The continuous journey (J by default) is unchanged. Core: `journey.rs` (`Phase::Stopped`,
`Choice`, `plan_with`, `Journey::choose`), `Navigator::journey_choose`.

| Input | Action |
| --- | --- |
| control center "Journey: Continuous / Stop at each find" (persisted, default continuous; ↺ of the section resets it) | the mode J and the Journey button start |
| J / ⇧J (not at a stop) | journey in the set mode / in the other mode; resume and stop as before |
| at a stop: J (iOS: the journey button, ▶) | next as planned |
| at a stop: Z (iOS: ↘ button) | deeper here |
| at a stop: X (iOS: ⑂ button) | somewhere else |
| at a stop: Esc | the journey ends, the view stays |
| at a stop: pan, zoom, wheel, palette, overlays, help, full screen, click | as without a journey; the journey stays at its stop (not paused) |

Z and X keep their usual meaning when not at a stop (Z the autopilot; X is unbound and not
handled). Bindings: Swift decodes keys in `FractalView.navKey` (macOS and iOS tables) into
`NavKey::{Journey, JourneyAlt, Autopilot, AutopilotOut, Elsewhere}`; all meaning is in the
navigator.

**At a stop** the leg has landed exactly on the landmark and nothing moves: the navigator asks for
no frames (`Journey::wants_frames`), so the picture finishes rendering (compute further included),
and the planned next leg is already waiting (planned while the leg flew). HUD: "Minibrot, period 34 ·
1.2e8× — J next · Z deeper here · X somewhere else" (`Target::title`; the zoom is the shown view's)
and "planning" while a choice is planned; the next leg's line carries a note ("nothing deeper here").

**Choices** (`Choice`, `plan_with`):

- *J, next*: the plan already made from the landmark, exactly the continuous journey's next target
  (test: the stop journey's second landmark equals the continuous one's).
- *Z, deeper*: a plan from the landmark that skips the cross-over and the style's depth cap, keeps
  only candidates of this view at least 1 e-fold deeper (`DEEPER_MIN_GAIN`; no wider view, a
  boundary detail is allowed after a detail) and relaxes the novelty threshold to the duplicate
  rule (0.22; within a dive it is otherwise 0.33). If nothing qualifies, it returns the plain plan
  (J behaviour) with the note "nothing deeper here". It counts as one of the dive's legs.
- *X, elsewhere*: a plan from the landmark that crosses over now (new style, another region).
- *Moved away*: the next leg is planned from the landmark while its centre is within half a view
  span of the view's centre and its zoom within 2 e-folds (`NEAR_SPANS`, `NEAR_EFOLDS`); beyond,
  from the current view, as a resume after manual input does. The flight always starts from the
  current view.

**Determinism.** The choices are user input. Every plan is a function of (start view, chain,
choice), and the chain only advances when a leg starts, so a stop journey is reproducible from its
seed, the start view and the sequence of choices; pressing only J reproduces the continuous journey
target for target. Moving far away and then choosing plans from the view: from there on the journey
depends on where the user went (as after a resume). CLI: `almondbread journey --seed n --choices
jzx...` plays a choice per stop (from the second leg). Seed 4711, `--choices xzzzx`: Triple Spiral →
over to Elephant Medallion → embedded Julia (period 125) 1e11.6 → towards a period-237 minibrot
1e15.6 → embedded Julia (period 237) 1e18.2 → over to Sunflower Island; `zzzzz` stays in the 1/3
valley to 1e16.6 (deep-dive cap 1e12 of the medium style ignored).

**Tests.** `journey_stops_at_each_find_and_takes_choices` (30/60/120 Hz: the same landmarks, exactly
on them, no frames at a stop; pan, wheel and click keep the stop; J = the continuous next target; X
crosses into another region; Z gains ≥ 1 e-fold or notes "nothing deeper here" and stays within a
view of the landmark; a view moved far away is planned from; Esc ends where it is; X not ours
otherwise). Interaction test scenario "journey stops" (J at E × 10 from 1e5: stopped at "Minibrot,
period 68", X flew on to "over to Pine Compass"; frame vs fresh render 0.000 %; measured
2026-10-17, release). `scripts/ui-screenshots.sh` scenes `journey-stop` and `zoom-out`.

**iOS.** The journey button turns into ▶ at a stop and two buttons (deeper, elsewhere) appear next
to it; a long press on the plane button offers the zoom out. Not tried on a device or simulator run
(no simulator runtime installed; the target builds).

## View rotation (as built, 2026-10-15)

`Viewport.angle` (radians, the camera's counter-clockwise turn: a view point (dx, dy) is
`center + scale · e^(i·angle) · (dx + i·dy)`) is carried end to end:

- **Kernel**: `IterateParams.rot = (cos, sin)` turns each texel's offset before the scale;
  `(1, 0)` keeps the plain products bit for bit. The DE derivative stays `dz/dc · texel`, so
  lighting is fixed to the fractal and turns with the image, as a rolled camera sees a lit
  relief; this also keeps reprojected history pixels consistent with new samples.
- **Reprojection**: `ComposeParams.hist_rot` maps output pixels into the history through
  scale, rotation and translation (`Renderer::reproject`); tile priorities test a tile's four
  corners against the turned history footprint.
- **CPU oracle** (`cpu_render`, `cpu_render_deep_with` take the angle), `verify --angle deg`,
  `verify --history rotate` (zoom while turning `--spin` radians per e-fold about the
  pointer), the fresh renders of the shown-frame checks (`displayed_frame_mismatch`,
  `verify_session`).
- **Navigator**: pans, zooms and every motion are in screen axes (`Viewport::rotate` inside
  `pan_by_points`, `zoom_by`, `point_at_offset`; `offset_of` for the inverse). Keys `,` / `.`
  turn at 45°/s (eased, ω = 20 / 14 /s; 30, 60 and 120 Hz agree to 1e-6 rad), `/` turns
  upright (τ = 0.12 s, exactly 0 at the end); trackpad rotation (macOS `rotate(with:)`) and the
  iOS rotation recogniser turn 1:1 about the zoom target. A flight turns from its start angle
  to the target's the short way round, in proportion to the path parameter, landing exactly.
- **Overlays**: orbit points are turned into screen axes; the mini-maps stay north-up and mark
  the view with a turned rectangle.
- **Coordinates**: the copied text gains `angle <degrees>` for a rotated view; paste restores
  it; fly-to keeps the current angle when the text has none.

Regress (in `make regress`): `--history rotate` at the neck (0.35 rad per e-fold, then a
step), a rotated (40°) zoom at 1e-305, and a flight from home to sea-8007 that turns to 60°;
all match a fresh render with 0 % class mismatch and 0 % escape-count differences.

## Cinematic effects (as built, 2026-10-15)

`crates/almondbread-core/src/cinematic.rs`, part of the per-window render settings
(persisted; reset by the ↺ of "Journey & cinematic effects"), off by default: one switch and
four strengths in the control center, menu "Cinematic Effects".

- **Spin**: automatic motion turns the view with the zoom, 4° per e-fold by default (±15 in the
  control center): the autopilot about its dive point, flights and journey legs by
  `spin · ρ · path length` spread over the path (the landing angle is where the spin ends, so a
  flight still lands exactly). Manual zooming never spins. At the default journey speed that is
  5.6°/s.
- **Hue drift**: the palette turns in OKLab hue (lightness and chroma kept; Ottosson's
  matrices, in the kernel's `paletteAt`; src: [[sources/ottosson-oklab]]) by 1° per e-fold
  of depth. The first default, 2.5°, turned the aurora look by 115° over the 46 e-folds of an
  85 s journey: its peach and teal went brown and pink, muddy in the review video; 1° keeps the
  look recognisable and still shifts it noticeably over a dive.
- **Phase drift**: the palette phase moves 0.01 cycles per e-fold of depth.
- **Cycling in flight**: 0.02 palette cycles per second while the autopilot or a flight moves.

Hue and phase are functions of the view's depth, not of time: a view always has the same
colours, and at rest nothing changes. The history keeps its colours across a zoom (the hue,
like the phase, is left out of the "same colours" test) and is replaced within a few frames.
Tests: `flights_turn_and_land_exactly`, `autopilot_spins_with_the_zoom_only_when_enabled`, the
`cinematic` unit test; `fly` scripts take `"cinematic": true` or the strengths.

## Cosmic size scale (as built 2026-10-15, one mode since 2026-10-17)

A panel at the bottom left (above the mini-map when that sits there too) that compares the zoom
with real things: a log ruler with coloured dots sliding past a gold marker, a sentence (wrapped
onto a second line when it is long) and one explanation line. Drawn in Rust/Metal into the
overlay scene (`crates/almondbread-render/src/cosmic.rs`); the table, the sentences and the
marks live in `crates/almondbread-core/src/cosmic.rs`, every size with its source page. Off by
default.

- **Controls**: C toggles it (macOS key and iPad/iPhone hardware keyboard), toolbar toggle
  "Size Scale" (ruler icon, with a tooltip), menu item "Cosmic Size Scale", control center
  "Overlays": a switch with the caption "Your window stays 30 cm (or its real size): how big
  would the whole set be?". No picker: there is one mode. Persisted with the other overlay
  settings; "Reset all" and the overlays ↺ turn it off. CLI: `render out.png --scale-overlay 1
  [--screen-cm 30]`.
- **The concept (agreed with the user 2026-10-17)**: your window stays the window, 30 cm or its
  measured width, at every depth; only the whole set grows. macOS reads the display's physical
  width (`CGDisplayScreenSize`, rejected outside 0.1 … 1 mm per point); iOS assumes 160 points
  per inch on iPhone and 132 on iPad (rule of thumb, unverified). Depth `d = log10(2.25 / view
  width)` in the plane, so the whole set is `L · 10^d` metres. Everything is computed from
  `n = d − d_U` (log10 of set over universe, `U` = 8.7e26 m), so nothing overflows to the depth
  limit (~1e3010). Ratios only, never counts of universes; no physics jargon (no light times, no
  GeV).
- **Phase 1, the set grows (to zoom ≈ 3e27 for 30 cm)**: "The whole set would be 4 km across"
  below 10 km, then the nearest thing in log: "… as big as Earth", "… 3× as big as the Sun";
  at the turn (set/U from 0.8 to 2) "The whole set would be as big as the observable universe".
  Line 2: "your window: 30 cm" (or "your window: 45 cm").
- **Phase 2, the universe shrinks inside the set (to ≈ 1e89)**: set/U = U/thing, so the
  universe would be as small as a thing of size `U · 10^-n`: "In the whole set, our universe
  would be as small as an atom", "… only 2.7× as big as the Milky Way", "… as small as 40% of a
  cell". Compared with familiar things only (Milky Way, solar system, Sun, Earth, Moon, Everest,
  you, ant, hair, cell, virus, atom, proton, Planck length). Galaxy ≈ 1e33, solar system ≈ 1e41,
  Earth ≈ 1e47, you ≈ 1e54, cell ≈ 1e59, atom ≈ 1e64, proton ≈ 1e69, Planck ≈ 1e89. Between
  set/U = 2 and a factor 4 above the galaxy rung no familiar thing is near (only distances
  between galaxies): "The whole set would be bigger than the observable universe". Line 2:
  "your window: 30 cm".
- **Phase 3, nested universes (from the Planck rung)**: `k = floor(n / a) + 1` universes with
  `a = log10(U / atom) ≈ 36.92` decades (set/U = (U/atom)^(k-1): imagine every atom were a
  universe). At `n = a` our universe is an atom of the set, so the set is already a universe of
  universes (k = 2, the "universe = atom" rung, still read in phase 2's words). Phase 3 starts
  at the Planck rung (`n ≈ 61.7`, zoom ≈ 1e89, k = 2) and k counts on from there (k = 3 at zoom
  ≈ 1e101, 27 at ≈ 1e987–1e1024, 81 at ≈ 1e2981–1e3018). Fixed 2026-10-17: the count was
  `max(2, floor(n / a))`, one universe short (the user asked whether the maths was right). Line 1: "A universe in a universe", "A universe
  in a universe in a universe", "A universe in a universe in a universe in a universe", from
  k = 5 "A universe in a universe in … (27 times)" (at 1e1000). Line 2: "imagine every atom were a
  universe, and every atom in it too".
- **Ruler**: the things larger than the window up to "universe"; then "1,000 × universe",
  phase 2's rungs "universe = galaxy", "= solar system", "= Earth", "= you", "= cell", "= atom",
  "= proton", "= Planck"; "googol ×" (set/U = 1e100); then "3× nested" … "90× nested" where
  k starts. "1 million ×" is not marked: it falls on "universe = galaxy". Linear in depth up
  to one decade past the Planck rung, then compressed (slope 1 → 1/8 over 15 decades; a level is
  then ~69 points). A unit test keeps every pair of marks past the universe at least 25 points
  apart. A label whose dot is near the marker is nudged off the marker (within 30 % of its width)
  instead of hidden; a dot right under the marker stays unlabelled.
- **Rejected variants**:
  - Built 2026-10-15: the screen anchor turned around at the universe (the set *was* the
    universe and the view shrank to the Planck length at zoom ≈ 1e62: "This view ≈ 1.8× the
    Planck length / if the whole set were the observable universe / light: 1e-43 s · to see it:
    4e19 GeV, unexplored"). The silent flip of what is fixed was "very confusing" (user,
    2026-10-16).
  - Built 2026-10-16: the set kept growing in counts of universes ("The whole set would be
    5e972 × the observable universe", ruler marks every 100 decades). Replaced by ratios to
    familiar things and nested universes (user, 2026-10-17).
  - Other modes, removed 2026-10-17: "The set is X" (universe, Milky Way, Sun, Earth, you,
    cell, atom: the view `X · 10^-d` down to the Planck length, then big-number milestones and
    universe-to-Planck ladders) and "Zoom range as cosmos (metaphor)" (zoom 1 … 1e200 as the
    universe … the Planck length and today … the Planck epoch, epochs from
    [[sources/cosmic-history-timeline]]; its pacing was tuned twice, see [[log]]). One mode is
    easier to understand; the picker is gone.
  - Dropped with them: the third line (light's crossing time, probe energy and band, "GUT
    scale?" mark, the red "unexplored" band).
- **How big is big?** (removed 2026-10-15): past the Planck length a gold "How big is big? →"
  link above the panel opened the gear panel ([[synthesis/how-big-is-big]]).
- **Readability**: the backing is 86 % opaque black, and the labels and lines have a 2.4 to
  2.6 px dark outline. Labels go above or below the ruler, whichever row is free, at least
  9 pt from the next label and never far from their dot; a label that does not fit is hidden.
- **Highlight**: crossing a mark flashes it gold with a growing ring and "passing X" above the
  panel for 1.6 s (no flash on jumps of 3 decades or more).
- **Cost** (measured 2026-10-15, M-series MacBook Pro, release, interaction test "cosmic
  zoom" vs "1e5 zoom", 1280x820 pt window): draw CPU p50 0.37 vs 0.14 ms (+0.23 ms: scene
  build ~0.01 ms, the rest binning and uploading the overlay scene every frame while the
  zoom changes). Off: no overlay work at all, the shown frame matched a fresh render.

Sources: [[sources/nasa-planetary-fact-sheets]], [[sources/iau-astronomical-units]],
[[sources/codata-2022-constants]], [[sources/cosmic-scale-references]],
[[sources/cosmic-history-timeline]] (removed metaphor). Idea: [[ideas]] "Cosmic size scale".

## π meter (as built, 2026-10-15)

A maths moment ([[techniques/pi-in-the-mandelbrot-set]]), off by default, drawn in Rust/Metal
like the mini-map (`crates/almondbread-render/src/pi_meter.rs`, maths in
`crates/almondbread-core/src/pi_moment.rs`).

- **When it shows:** the probe is on (below), or the view's centre lies on the neck's line
  `-3/4 + iε` (or `-3/4 - iε`) or the cusp's line `1/4 + ε`, within 20 % of ε sideways,
  ε ≤ 0.1. Elsewhere a small hint panel says where to go.
- **Probe (updated 2026-10-15):** a glowing marker on the line that the meter counts instead of
  the view centre, so the view can stay on the valley while the counted point goes far below a
  pixel from the pinch point. It slides on a log ruler (one tick per decade from 0.1, two at
  the cusp, "ε, log scale"), its true position as text (`probe c = −3/4 + 1.00e-8·i`). Set by
  the π hunt; kept while its pinch point is in view (it wins over a view centred on the line);
  put away when the pinch point leaves the view.
- **Panel (bottom right):** `N·ε =` (or `N·√ε =`) with the digits that agree with π in gold and
  the rest dimmed, π below it aligned digit for digit, N and ε, "k digits of π" or the progress
  of the running count, and a log-log plot of |N·ε − π| against ε (ε falls to the right) that
  gains a point each time a count finishes, with the expected ~ε (~√ε) trend dashed. On the
  canvas without a probe: a dashed line of approach from the parabolic point (ringed,
  labelled) to the centre (gold ring), since the image there is mostly unresolved.
- **Orbit trail inset (2026-10-15):** above the panel (434 points wide; left of it when the
  view is too low, hidden when neither fits; clear of maps in the right corners and of the
  maps and cosmic scale at the bottom left). Left: the counted orbit (the neck's gate in the
  z-plane at ±4√ε with the fixed point and 2-cycle ringed; the cusp's `(z − 1/2)/√ε` against
  `√ε·n` with π/2 and π marked). Right: the turn circle (the orbit in the turning coordinate
  goes round once: "2ε per step: N ≈ π/ε"). A glowing density picture coloured by progress
  n/N (blue, cyan, gold, rose), with the newest iterate marked while the count runs, so the
  orbit visibly crawls through the gate on long counts. Why these plots: the technique page,
  "The orbit".
- **Counting:** a background thread per count (compensated f64 in shifted coordinates), which
  also accumulates the trail (every k-th iterate, at most 4e6 per count) and publishes its
  picture every 0.12 s; the frame only reads results and uploads a new picture as a new shared
  buffer (a glow primitive in the overlay kernel adds it), so frames cost a scene rebuild when
  the text or picture changes (about 0.25 ms p50, 0.4 ms p95 a frame during the hunt; the
  technique page has the measurements). A running
  count is kept while the view goes deeper (it becomes a plot point) and dropped when the view
  leaves the point or zooms out past it by more than a decade. **Depth limit 10⁹ iterations**
  (ε ≥ 3.1e-9 neck, ε ≥ 1e-17 cusp; about 13 s); deeper, the panel says so. The image's
  iteration limit is not raised (measurements on the technique page).
- **Controls:** P toggles (macOS and iPad keyboards); toolbar toggle "π Meter" (SF Symbol
  `pi`) with a tooltip; View menu "π Meter" (⌥⌘P), "Hunt π at the Neck", "Hunt π at the Cusp"
  (renamed from "Fly into the …": the view no longer flies into the point); control center
  Overlays: switch, reading (product, N, digits, progress) and the two hunt buttons (the iOS
  way in). Persisted with the other settings, off after Reset all and
  the Overlays ↺. iPhone portrait moves the panel left of the button column (FFI inset).
- **π hunt (guided flight, reworked 2026-10-15):** takes the place's look; the navigator's
  fly-to eases the view to seahorse valley (neck) or elephant valley (cusp) with the pinch
  point in view; the probe then slides from ε = 0.1 to 1e-8 (neck) or 1e-16 (cusp) at the
  zoom speed in e-folds of the expected count (about 10 s at 1.6/s; Q / E change it) while the
  view zooms gently (×1.4 neck, ×2 cusp) and the meter counts and draws the trail; it holds
  until the last count is done plus 2.5 s. Esc ends it, and a move during the slide ends it;
  the probe stays. Before, the flight zoomed the view into the pinch point, where the image is
  black ("not really exciting"). Driven by `PiMeter::flight_view` from `Overlay::prepare`
  (`Overlay::start_pi_flight`). FFI: `set_pi_meter`, `pi_meter`, `pi_flight`.
- **Headless:** `almondbread render out.png --pi-overlay 1 [--ppp 0.5]` waits for the count and
  fills the plot with the counts every half decade from ε = 0.1 down to the view's ε;
  `--pi-probe neck,1e-5` puts the probe there in the flight's view at that moment (with its
  trail). `almondbread fly --script scripts/flights/pi-neck.json --pi-flight neck` records
  the hunt as the app shows it (`pi-cusp.json`, `--pi-flight cusp`); ui-demo scenes
  `pi-flight`, `pi-flight-late`, `pi-cusp`, `pi-cusp-late`; interaction-test scenario
  "pi hunt".

## Compass (as built, 2026-10-16)

The cinematic spin, the rotate keys and the trackpad turn the image, so a compass shows which
way the plane's north (+im) is, after Apple Maps' compass. Drawn in Rust/Metal into the
overlay scene (`crates/almondbread-render/src/compass.rs`; a new overlay primitive, a filled
triangle, kind 7); policy and maths in `crates/almondbread-core/src/compass.rs`; motion in the
`Navigator` (`turn_north`, `turn_view`). Swift only routes presses.

- **Look:** a 22 pt rose (dark translucent disc, thin light rim, ticks at E/S/W and the
  diagonals), a kite needle with a red north half and a pale south half (lit and shaded
  sides), an upright "N" (overlay SDF font) beyond the north tip. On screen north points at
  `π/2 − angle` (counter-clockwise from +x, y up).
- **Placement:** top right of the canvas, 14 pt inside the safe area (below the toolbar, left of
  the control center on macOS: the canvas runs under the sidebar and the safe-area insets
  exclude it), left of mini-map panels when those sit top right; iPad and wide iPhone 56 pt
  lower (the top-right button row; FFI `set_compass(always, inset_top)`). The HUD (top left),
  the mini-map and cosmic scale (bottom left) and the π meter (bottom right) keep their
  corners. Mini-map panels now also keep clear of the safe-area insets (top-right maps used to
  sit under the toolbar and the control center).
- **Visibility:** while the view is turned by more than 0.5°, during a drag or a turn to north,
  then 2.5 s more and a 0.5 s fade (frames are requested only for the fade). Setting
  "Always show compass" (control center Overlays, View menu "Always Show Compass"; persisted,
  off after Reset all and the Overlays ↺). Hidden, it adds no primitive, no scene and no work.
- **Click / tap:** eases the view back to north about the centre in 0.4 s (cubic ease in-out).
  It is applied as the ratio of what is left of the ease, so motion added meanwhile (the
  spin) is absorbed and the turn ends at exactly 0. A double click is two presses on the
  compass: the second does not restart the turn and never reaches the canvas (no double-click
  zoom, no autopilot pause). iOS: tap and double tap turn to north.
- **Drag:** the needle follows the pointer around the compass centre 1:1 (the view's angle
  falls by the pointer's angle change, the short way round); a press becomes a drag beyond
  4 pt (a click otherwise). iOS: a one-finger pan that starts on the compass.
- **Autopilot, flights, journeys with spin:** a turn to north (or a drag) does not stop them.
  The autopilot keeps spinning about its dive point from the straightened angle. A flight is
  turned along (`Flight::shift_angle`: start, end and landing angles shift by the same
  amount), so it keeps spinning from north and lands turned by as much (a non-spinning
  fly-to that turns to a target angle lands shifted too: the user turned the flight). The
  journey's next leg starts from the current angle. Turning by hand (rotate keys, `/`,
  trackpad rotation) ends a turn to north; `/` keeps its faster τ = 0.12 s ease and, unlike
  the compass, cancels a flight.
- **Hit test in Rust:** `Host::compass_press / compass_drag / compass_release` (FFI
  `compass_press`, `compass_drag`, `compass_release`, `compass_center`), like the mini-map's
  `map_click`; a hit counts within the rose plus 5 pt, only while it is clearly visible
  (opacity > 0.25).
- **Cost:** while shown and turning (the spin) the scene is rebuilt every frame: build and
  binning of the compass alone 18 µs at 2560x1640 (measured 2026-10-16, M1 Pro,
  release); interaction test "compass" (cinematic autopilot with the compass shown, a drag and
  a click) draw CPU p50 0.33 ms, the same as "cinematic autopilot" without it (0.33 ms).
- **Tools:** `almondbread render out.png --angle 30 --compass 1 [--insets top,right]`;
  ui-demo scenes `compass`, `compass-maps`; interaction-test scenario "compass" (drag a
  quarter turn: −88.7°; click at −36.7°: 0.66° 0.6 s later with the spin going on).
- **Tests:** `almondbread_core::compass` (ease reaches exactly 0 and is the same at 30, 60,
  120, 240 Hz; motion added meanwhile is absorbed; drag maths across the ±180° seam; hit test;
  visibility and fade; Navigator: turn about the centre, frame-rate independence, drag ends a
  turn, autopilot and flight with spin), `almondbread_render::compass` (layout, click versus
  drag, nothing drawn while hidden).

## Full screen (macOS, as built, 2026-10-16)

Requested by the user: a clean view for watching flights. AppKit's own full screen; the
policy is Swift's (it is window chrome, not navigation), the Esc decision uses the
`Navigator`'s answer to `Stop`. iOS and iPadOS: nothing new (the canvas is already full screen).

| Input | Action |
| --- | --- |
| F (canvas key, like O / M / P) | full screen on / off; key repeats are ignored |
| ⌃⌘F, View ▸ Enter Full Screen, the green button | the same, AppKit's standard path |
| Esc in full screen | stops the autopilot, a flight, a journey or a π hunt if one runs (eased); only an Esc that stopped nothing leaves full screen |

- **Esc.** In full screen the key monitor swallows every Esc, so AppKit never sees one; the
  key-down goes to Rust as `Stop`, and only when that is not handled (nothing moving) does the
  window leave full screen. A held Esc's repeats never leave: the press that stopped the
  autopilot must not also exit while the stop eases out. A second, separate Esc during the
  0.5 s ease-out does leave (Rust no longer reports motion to stop). Outside full screen Esc is
  unchanged: handled only while something moves, otherwise the system's. Text fields keep Esc.
- **Clean view.** The control center closes as the window starts entering full screen and comes
  back as it was when it starts leaving (opening or closing it in full screen does not count;
  `FullScreenSidebar`, driven by `ExplorerModel.fullScreen` from the window's will-enter /
  will-exit notifications, so ⌃⌘F and the green button behave like F). The toolbar hides and
  slides in when the pointer reaches the top edge (`windowToolbarFullScreenVisibility(.onHover)`,
  macOS 15+; macOS 14 keeps AppKit's always-visible toolbar). The HUD and the overlays keep
  their settings. No HUD auto-fade: it would need an input timestamp observed by SwiftUI or a
  timer, and the HUD can already be switched off (View ▸ Status Overlay).
- **Insets.** The HUD (SwiftUI, inside the safe area) and the Rust overlays (compass, mini-maps,
  pointer canvas, edge band: the `Insets` event) read the same safe area. Besides `layout()`,
  the insets are sent again after each full-screen transition and whenever the window's
  `contentLayoutRect` changes (KVO), so a hidden toolbar gives the overlays the top edge. While
  the toolbar is slid in on hover it overlays the canvas (AppKit's standard behaviour; not
  verifiable without entering full screen in a test, which window tests never do).
- **Menu.** AppKit adds "Enter Full Screen" (⌃⌘F) to the View menu only for a menu bar it
  shows; the canvas makes sure the View menu has an item with the `toggleFullScreen:` action
  (AppKit's own check before it adds one, so there is never a duplicate) and AppKit titles it
  Enter / Exit. The window is marked a primary full-screen window (a background test window,
  an accessory app, reports `fullScreenNone`).
- **Test.** Interaction-test scenario "esc full screen" replaces the window's full-screen state
  and toggle with a probe (window tests never enter full screen): Z, then Esc stops the
  autopilot and stays; 5 Esc repeats stay; Esc 1.2 s later (nothing moving) leaves; F enters,
  an F repeat does nothing, F leaves; outside full screen an idle Esc passes to the system; the
  control center's close-and-restore. All OK (2026-10-16, M1-class Mac, release).

## Render progress (as built, 2026-10-17)

User decision (2026-10-17): while a settled view computes for long (a jump, after zooming
stops, a high manual limit such as 10 M with automatic iterations off) the app shows how far
it is, honestly.

- **The line.** A thin (2 pt) light-blue line along the top edge of the canvas, like a browser
  loading bar, drawn by Rust into the overlay scene (`crates/almondbread-render/src/progress.rs`,
  `draw_line`) just below the toolbar: it uses the navigator's canvas insets, the same the
  compass and HUD use (full screen with the toolbar shown or hidden included). A faint dark
  track under it keeps it readable on bright images; a brighter head over its last 48 pt.
- **When.** It appears once the settled view has computed for 0.5 s, and only if the estimate
  says more than 0.6 s is left (or after 1.5 s regardless; unknown stages, reference orbit,
  tables, seeds, count as long). It fades in over 0.2 s; when done it runs to the end
  (0.3 s), then fades out (0.45 s). Input (gesture frames, flights, the autopilot) hides it
  within 0.15 s. Gesture frames never show it: shallow zooms show nothing.
- **Honest fraction.** Time-based: `elapsed / (elapsed + remaining)`, never moving backwards and
  closing at most half of the remaining gap per second (one optimistic estimate cannot throw
  it ahead, where it would stall). The remaining time is the current level's plus predictions
  for the stages after it:
  - *Level*: pixels left (`total − admitted + live`) over their resolve rate in a window of
    0.35 × the level's age (0.75–20 s); once every pixel is admitted also the youngest live
    pixel's steps to the limit over its recent speed (`liveMinN`, a new chunk counter: one
    `simd_min` and one `atomic_fetch_min` per threadgroup). The step bound is an upper bound
    (pixels speed up as others finish); the estimate is `sqrt(min(by_pixels, by_steps) ×
    by_steps)`: the resolve rate is right while most pixels finish, the step bound when the
    last slow ones decide.
  - *Preview*: the full level as the preview's time × the texel ratio.
  - *Extension passes*: up to their GPU budget (2 s) × wall per GPU second, at most 4× the full
    level.
  - *AA samples*: each costs the full level's time × the adaptive list's share (0.5 until it
    is built, then the last sample's time); they run until their GPU time passes the AA budget.
  - Reference orbit, tables and the M5 seed passes are unknown: the line creeps towards 20 %
    (25 s time constant) and claims nothing more.
  The bias is pessimistic, so the line speeds up towards the end instead of racing to 95 %
  and stalling.
- **HUD text** (`HUDProgressLine`, a leaf view reading the change-only `progressText`), e.g.
  "refining · 2.1 M of 10 M steps · 3 % of pixels left · ~40 s". Stage words: reference
  orbit, preparing, seeding, preview, refining, resolving (extension), anti-aliasing (with
  "sample n of m"). Steps show only once every pixel is admitted (before, the youngest pixel
  is a fresh one); pixels left is monotonic. The ETA shows after the line has shown 3 s, the
  level's pixels have run 8 s (or less than 16 s are left), and the raw finish time held
  within 30 % over 3 s; it is then smoothed (time constant 0.1 × the remaining time, 2–8 s)
  and rounded coarser as it grows (~7 s, ~40 s, ~4 min). It disappears when done.
- **FFI.** `ExplorerStats.progress: RenderProgressInfo` (visible, fraction, stage, text, ETA,
  steps reached / limit, pixels left, and `unresolved` / `unresolved_of`: what the last full or
  extension level left unresolved, kept after the view settled, for a later "raise the limit?"
  hint).
- **Measuring.** `almondbread progress --location L | --place P | --center re,im --scale s
  [--iterations N] [--zoom-seconds s]` jumps (or zooms, then stops) through the real
  per-refresh scheduler headless and prints the line, the ETA and the counters against t / T
  (T: when the view settled). `--ui-demo=progress` / `progress-late` capture the window
  mid-render.

Measured (2026-10-17, M1-class Mac, release, 2560×1600 px headless, other agents sharing the
GPU):

| case | settled | line vs t / T | ETA |
| --- | ---: | --- | --- |
| neck −0.74999996120361811 + 0.00036918279150454967i, 1e-11/pt, manual 2 M | 179–236 s (seeds 10–25 s, full ~105 s, one AA sample ~70 s at the 2 M limit) | within −10 / +6 points; 54 % at 57 %, 73 % at 76 %, 89 % at 96 % | first ~2 min at t = 23 s (true 2.6 min), then within ~25 % |
| seahorse −0.743643887037151 + 0.13182590420533i, 2e-9/pt, manual 10 M (1316 pixels unresolved) | 7.8 s | within ±10 points (16 % at 13 %, 51 % at 45 %, 85 % at 91 %) | none (too short to be stable) |
| spiral-1e1000 (jump) | 1.2–2.3 s | shown for the last ~0.6–1 s | none |
| shallow zooms (2 s gesture at 1e-5 … 1e-12 per px, 5 places) | 0.3–0.9 s | never shown, except −1.25 + 0.02i at 1e-6 (1.3–1.8 s: 940 k interior pixels in extension passes) | — |

Early versions failed in instructive ways: an AA "between samples" moment recorded as a sample
duration predicted 0.1 s samples and threw the line from 55 % to 90 %, where it stalled for a
minute; an ETA computed in the level's first seconds after its seeds said "~50 min" for a
5-minute view (now gated on the pixels' age, and not smoothed before it shows).

Findings for the scheduler (not changed here): at a 2 M manual limit the first AA sample
lists 70 % of the view at the nominal limit and costs ~70 s, as much as the full level, before
the 1.5 GPU-second AA budget is checked; and shallow views in the period-2 bulb spend their
extension passes on ~1 M interior pixels.

## Help (as built, 2026-10-17)

Agreed with the user (2026-10-17): help in three layers, written for curious
non-mathematicians (what you see and what to do, no jargon).

1. **(?) popovers** next to a feature: a title, 3–6 short lines and "More…", which opens the
   matching help page.
2. **One help window**, "Almondbread Help" (macOS: Help ▸ Almondbread Help, ⌘?, a SwiftUI
   `Window` scene with a sidebar of pages; iOS: a sheet with a page menu, opened by "More…").
   Pages: Getting started & controls; What am I seeing? (the rule, black = proven inside,
   colours = escape speed, the dark grey "not decided yet" veil and what to do about it,
   automatic vs manual iterations and the upper limit, the progress line); Autopilot,
   journeys & places; Overlays (cosmic scale, π meter, mini-map, orbit, compass, status);
   Deep zoom & precision (briefly); Tips.
3. **The texts are Markdown files** bundled with AlmondbreadKit
   (`Sources/AlmondbreadKit/Resources/Help/<n>-<page>.md`, the title is the `# ` heading;
   `popovers.md` holds one `## <topic-id>` section per (?)), rendered by a small SwiftUI
   renderer (`HelpMarkdownView` in `Help.swift`: headings, bullets, paragraphs; inline
   emphasis through `AttributedString` markdown). The same files can become a website.

**Where the (?) are.**

- *Control center* section headers that need explaining: Iterations & quality, Look & colour,
  Interior, Overlays, Controls. `HelpButton` is a static view with its own `@State` popover:
  it reads nothing from the model, so the sidebar's change-only rule holds
  ([[platform/swiftui-performance]]).
- *Overlays drawn by Rust*: the cosmic scale panel, the π meter (panel and hint panel) and the
  mini-map row (the whole-set panel's top-right corner; "orbit" when only the orbit overlay
  shows that map). A 7 pt dark disc with a light rim and a "?" glyph, 5 pt inside the panel's
  top-right corner (`crates/almondbread-render/src/help.rs`); the cosmic ruler and the π hint
  panel keep 19 pt free for it.

**Design: hit-testing in Rust, the popover in SwiftUI.** The panels lay their badge out while
the overlay scene is built (`Overlay::help`, cleared when nothing is drawn), so the badge is
exactly where it was drawn. The platform asks `Explorer.help_hit(x, y)` on every press before
the compass and the maps (view points from the centre, y up, like the compass): a hit returns
the topic id, centre and radius, and the press (its drag, release and double click) goes
nowhere else. Swift sets the model's `canvasHelp` (topic and the badge's rect in canvas
coordinates), the only observed property involved, and a `.popover(item:attachmentAnchor:)` on
the canvas anchors at that rect (above badges in the lower half, below in the upper; iOS
`.presentationCompactAdaptation(.popover)` keeps it a popover on iPhone). `Overlay::click`
also ignores presses on a badge, so another UI cannot jump the map from it.
Rejected: SwiftUI buttons positioned from panel rects that Rust reports. That needs a
change-only feed of rects every time a panel moves or resizes (the cosmic panel's height
follows its wrapped sentence, the π panel moves with the sidebar), and the button would trail
the Metal frame by a frame; the synchronous hit test needs neither, and other platforms get
the badge for free. Only the circle plus 2.5 pt takes presses (unit test `help::tests`), so
the panels and the canvas keep every other click.

**Checks.** `--ui-demo=help-canvas` (the cosmic scale's popover pressed on its badge),
`help-control` (the Iterations & quality popover) and `help-window` (the help window, a
window of its own behind all others) write `build/screens/help-*.png`, the window plus its
popover windows composited (`UIDemo.captureWithPopovers`).
