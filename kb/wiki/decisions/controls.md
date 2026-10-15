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
Sections in order: Places, View & navigation, Look & colour, Lighting & outline, Interior,
Overlays, Iterations & quality, Performance & diagnostics, Controls. A search field at the top
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
target can be recaptured while it is within about `R (1 + k/z)` of the centre. Straight
ahead (h* = 0) when the pointer is off the canvas or over the HUD, in keyboard mode, or idle
for 3 s; edge panning is off while the autopilot runs. Rate and heading are integrated in
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

**Assist** (inspector toggle, off by default, persisted). The survey's detail-weighted
centroid around the heading (σ = 0.35 of the smaller side; score = min(detail / 12, 1) ·
(1 − interior / 2)) leans the heading target towards boundary detail by 0.6 of its offset,
at most 0.4 R, fading out as the pointer deflects past the dead zone (gone at a response of
0.2): it never steers against the user.

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

**Reset (2026-10-15, user request).** "Reset all…" at the bottom of the control center asks
"Reset settings" or "Reset settings and view" and puts every per-window setting (look,
palette and mapping, lighting and outline, interior, overlays, iterations and quality, edge
panning, autopilot assist; with the view: the whole set) back to the defaults
(`Explorer.defaultSettings()`). Each section header has a small ↺ that resets that section
alone. Both are undoable (Edit ▸ Undo, ⌘Z: the previous settings record and view go on the
window's undo manager). The Settings window's reset of app-wide preferences stays separate.
Places, autopilot and fly-to are in the control center since the integration (Places: grid by
category, Go / Fly / Random; View & navigation: Autopilot, Fly to…, Copied, assist).

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

## Cosmic size scale (as built, 2026-10-15)

A panel at the bottom left (above the mini-map when that sits there too) that compares the zoom
with real things: a log ruler with coloured dots for things sliding past a gold marker, one
sentence and two smaller lines. Drawn in Rust/Metal into the overlay scene
(`crates/almondbread-render/src/cosmic.rs`); the table, the sentences and the mappings live in
`crates/almondbread-core/src/cosmic.rs`, every size with its source page. Off by default.

- **Controls**: C toggles it (macOS key and iPad/iPhone hardware keyboard; free before: WASD,
  arrows, Q/E, Z, J, O, M, R, `,` `.` `/`, Space, `+`/`-`, Esc), toolbar toggle "Size Scale"
  (ruler icon, with a tooltip), menu item "Cosmic Size Scale", control center "Overlays":
  switch plus a "Compare" picker. Persisted with the other overlay settings; "Reset all" and
  the overlays ↺ turn it off. iOS: the control center row (no extra button: the bottom-right
  stack is full on iPhone). CLI: `render out.png --scale-overlay 1 [--scale-anchor
  screen|cosmos|universe|milky-way|sun|earth|human|cell|atom] [--screen-cm 30]`.
- **Depth**: `d = log10(2.25 / view width)` in the plane (the set spans re −2 … 1/4).
- **Screen anchor (default, physical)**: the view is the window on the screen. macOS reads the
  display's physical width (`CGDisplayScreenSize`, rejected outside 0.1 … 1 mm per point); iOS
  assumes 160 points per inch on iPhone and 132 on iPad (rule of thumb, unverified); otherwise
  30 cm. The whole set is `L · 10^d`: "The whole set ≈ 1.6× the Milky Way". When it passes the
  observable universe (`d ≈ 27.5` for 30 cm) the readout turns around without a jump: the set
  *is* the universe and the view is `U · 10^-d`, down to the Planck length at `d ≈ 61.7`.
- **Set = X**: the view is `X · 10^-d` ("This view ≈ a hydrogen atom, if the whole set were
  Earth").
- **Light and probes**: a third line gives light's crossing time ("light crosses it in 165
  thousand years", "longer than humans have existed", "≈ the Planck time") and, for views
  below 10 µm, the probe energy `E = hc/λ` with its band (visible, UV, X-rays, gamma, more than
  the LHC's 13.6 TeV). Band edges are textbook ranges (unverified).
- **Small side**: cell, virus, DNA, atom, nucleus, proton, "limit probed" (~1e-19 m), then a
  red band "unexplored" down to the Planck length; inside it the sentence counts Planck lengths
  and the GUT scale (~2e-32 m) is marked hypothetical; the Planck mark reads "Planck
  (strings?)": strings are hypothetical.
- **Past the Planck length**: the zoom factor (as the HUD shows it) passes big numbers: Planck
  times since the Big Bang (8e60), atoms in the universe (1e80), a googol, Shannon's chess
  games (1e120), Planck volumes in the universe (8e184); then universe-to-Planck ladders
  (61.7 decades each; 48.6 at 1e3000), drawn as repeated faint rungs. The ruler compresses past
  the Planck length (slope 1 → 1/12 over 15 decades).
- **Relative anchor "Zoom range as cosmos (metaphor)"**: zoom 1 … 1e200 (one constant) stands
  for the universe … the Planck length and for today … the Planck epoch. The 25 things sit on
  one grid evenly spaced in zoom decades: 8.3 decades a step, about 12 s at the default zoom
  speed (×4.95/s), the whole cosmos in under 5 minutes. The epochs sit on the same grid where
  the view is about the thermal wavelength `hc/kT` of their temperature (CMB at a cell,
  nucleosynthesis at an atom, quark-hadron at a nucleus, electroweak just below a proton,
  inflation at the GUT scale, Planck at Planck; today, the Sun and the first stars spread over
  the astronomical part; src: [[sources/cosmic-history-timeline]]), so size and time do not
  drift apart. Log size and log time are interpolated between grid points. Sizes are shown
  above the ruler, epochs below; the panel names the next thing and says it is a metaphor.
  Past 1e200 it counts "cosmos lengths" (2.5 at 1e500) with the things repeated as faint rungs.
  - Rejected first: spacing in `log10(1 + zoom decades)` over 1e3000. Steps grew from 1 s to
    20 min ("from hair to blood cell it takes ages", user, 2026-10-15).
  - Rejected before that: a linear map over 1e3000 (48.6 zoom decades per physical decade).
- **Picker**: "Your screen", then "The set is …" (universe, Milky Way, Sun, Earth, you, a
  cell, an atom), then "Zoom range as cosmos (metaphor)", in groups. A caption under the picker
  explains the chosen anchor ("Your window is fixed: how big would the whole set be?"), and the
  options carry the same text as tooltips.
- **How big is big?** (removed 2026-10-15): past the Planck length a gold "How big is big? →"
  link above the panel opened the gear panel, placed from `Explorer.cosmicHint()`. Link, panel
  and `cosmicHint()` are gone with the panel ([[synthesis/how-big-is-big]]).
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
[[sources/cosmic-history-timeline]]. Idea: [[ideas]] "Cosmic size scale".

## π meter (as built, 2026-10-15)

A maths moment ([[techniques/pi-in-the-mandelbrot-set]]), off by default, drawn in Rust/Metal
like the mini-map (`crates/almondbread-render/src/pi_meter.rs`, maths in
`crates/almondbread-core/src/pi_moment.rs`).

- **When it shows:** the view's centre lies on the neck's line `-3/4 + iε` (or `-3/4 - iε`) or
  the cusp's line `1/4 + ε`, within 20 % of ε sideways, ε ≤ 0.1. Elsewhere a small hint panel
  says where to go.
- **Panel (bottom right):** `N·ε =` (or `N·√ε =`) with the digits that agree with π in gold and
  the rest dimmed, π below it aligned digit for digit, N and ε, "k digits of π" or the progress
  of the running count, and a log-log plot of |N·ε − π| against ε (ε falls to the right) that
  gains a point each time a count finishes, with the expected ~ε (~√ε) trend dashed. On the
  canvas: a dashed line of approach from the parabolic point (ringed, labelled) to the centre
  (gold ring), since the image there is mostly unresolved.
- **Counting:** a background thread per count (compensated f64 in shifted coordinates); the
  frame only reads the result, so frames cost a scene rebuild when the text changes. A running
  count is kept while the view goes deeper (it becomes a plot point) and dropped when the view
  leaves the point or zooms out past it by more than a decade. **Depth limit 10⁹ iterations**
  (ε ≥ 3.1e-9 neck, ε ≥ 1e-17 cusp; about 13 s); deeper, the panel says so. The image's
  iteration limit is not raised (measurements on the technique page).
- **Controls:** P toggles (macOS and iPad keyboards); toolbar toggle "π Meter" (SF Symbol
  `pi`) with a tooltip; View menu "π Meter" (⌥⌘P), "Fly into the Neck (π)", "Fly into the Cusp
  (π)"; control center Overlays: switch, reading (product, N, digits, progress) and the two
  flight buttons (the iOS way in). Persisted with the other settings, off after Reset all and
  the Overlays ↺. iPhone portrait moves the panel left of the button column (FFI inset).
- **Guided flight:** jumps to the place "π at the Neck" (ε = 0.02) or "π at the Cusp"
  (ε = 2e-4), turns the meter on and zooms exponentially about the parabolic point to
  ε = 1e-8 or 1e-16, every view framed like the place (about 10 s / 18 s at the default zoom
  speed; Q / E change the speed, Esc or any move ends it). Driven by the meter in
  `Overlay::prepare`, not the fly-to path (that pans first). FFI: `set_pi_meter`, `pi_meter`,
  `pi_flight`.
- **Headless:** `almondbread render out.png --pi-overlay 1 [--ppp 0.5]` waits for the count and
  fills the plot with the counts every half decade from ε = 0.1 down to the view's ε.
