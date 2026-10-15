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
