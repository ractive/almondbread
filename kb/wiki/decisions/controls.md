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
| Double-click / Option+double-click / right double-click, two-finger double tap (Smart zoom) | zoom in / out ×2 at the click, a 0.35 s glide; every further click of the stream (click count 3, 4, …) adds ×2 (Mac; switches to pointer mode; Smart zoom always zooms in) |
| Pointer in the edge band while Space is held | pan towards that edge (setting: off / while zooming / always) |
| Drag, pinch, double tap (and each further tap of the stream), two-finger tap | pan, zoom, zoom in ×2 per tap, zoom out (iPhone/iPad) |
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
  1:1), + / − and menu zooms are exponential ease-outs (τ = 60 ms) at the zoom target, summed
  when they overlap; the fixed point holds (tests: drift < 1e-6 px). They stay snappy: a notch
  is a small step (×1.15) the hand repeats, and a key or menu zoom answers a command.
  **Double click / tap** (revised 2026-10-18; user: "feels unnatural, very quick and
  immediate; multiple double clicks in a row don't work") is its own glide: ×2 in log scale
  on a minimum-jerk curve (quintic, rest to rest) over `TAP_ZOOM_SECONDS` = 0.35 s about the
  click. No first-frame jump (after one 120 Hz frame 0.02 % of the step; the τ = 60 ms ease-out
  had 13 %); the tails are slow (under 10 % of the peak rate for the first and last 13 %), so
  the visible motion is ~0.25 s, like the system's double-tap zooms; peak rate 1.875 · ln 2 /
  0.35 = 3.7 e-folds/s. **Chaining:** a further click while one runs adds ×2 (×0.5 out) to the
  target and starts a new quintic from the current offset, rate and acceleration (C²: no
  restart, no stutter), towards the latest click's point; three zooms land at exactly ×8, and
  the scale never turns back (tests at 60 and 120 Hz, clicks 0.08-0.5 s apart:
  `chained_double_taps_accumulate_smoothly`). Mac: every click from the second of a click
  stream zooms (`clickCount >= 2`, left with Option and right zoom out); only the first click
  toggles the autopilot's pause and the second undoes it (`Autopilot::double_click` forgets
  the click), so later ones never toggle. iOS: after a double tap, a touch within 0.4 s and
  60 pt of the last zoom tap goes to a third, single-tap recogniser only (`chainTap`; the
  double- and single-tap recognisers do not receive it), which zooms on its lift: a triple
  tap zooms ×4, a quadruple ×8 (`TouchUITests.testTapStreamsZoom`).
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
has no hover, so a tap selects the picture and puts the name, Go and Fly on it, a second tap
goes there, and a long press opens a menu titled with the name (Go, Fly; "Touch", places grid) ([[platform/swiftui-performance]]). A search field at the top
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
colouring as built") in steps of ~1.5 ms per frame, and shown under the mini-map in plain
words (revision 2026-10-17, user: "analysing …" / `|z50| 0.5362` said nothing): "escapes after
48 213 steps", "settles into a period-34 cycle" ("settles on a fixed point" for period 1),
"still undecided after 100 000 steps", and while it runs "following the orbit: 12 000 steps so
far" (two digits, so the text and the scene change only every ~10 %).

*Dot cloud on the whole-set map* (2026-10-17). In slow-escape regions (the necks) the
interesting part of an orbit comes at step 10^4 to 10^5, far past the 1000-point line. The
probe now runs at least 10^5 steps (`PROBE_MIN_STEPS`; more when the renderer's limit is
higher) and records each iterate's plane value into `OrbitCloud` (`core::orbit`): up to
10^5 points, then every 2nd, 4th, ... (halve the kept points and double the stride, so the
cloud always spans the whole orbit so far within a fixed budget; point k is iterate
1 + k·stride). The map (level 0, also the orbit-only whole-set map) draws the first 50 steps as
the old line, fading fast (near a neck the first steps zigzag across the map and an even web
of lines hid the cloud), and the rest as one-pixel dots: a CPU picture at the panel's pixel
size (`orbit_cloud_picture`: per pixel the visits and the newest visit; newer points whiter and
more opaque, older ones faint and cool, more visits more opaque; every dot has a dark rim, the
empty neighbours, so it reads on bright and dark palettes), uploaded as a BGRA buffer and
alpha-blended by the overlay kernel's glow-image primitive (`info.w = 1`, buffer 7). No
primitive per dot: 10^5 discs binned into the panel's tiles would make every pixel walk
hundreds of primitives. The picture is rebuilt only when the cloud grew by a quarter of a
doubling (~19 %; ~45 rebuilds on the way to 10^5) or the probe finished; a rebuild of 10^5
points on a Retina panel (360×259 px) takes 0.43 ms (measured 2026-10-17, M1 Pro, release).
Interaction test "orbit cloud" (pointer resting on a golden-mean point of the cardioid's edge,
undecided after 10^5 steps, moved half a point every 0.75 s; view 1373×, 2560×1640): draw CPU
p50 0.30 / p95 2.45 ms against 0.25 / 0.35 ms with the orbit off (the p95 is the 1.5 ms probe
step plus rebuilds), 60 fps either way (the view's own render is GPU-bound there), no new
hitches. The main view's orbit is unchanged (its iterates are thousands of view widths away in
such regions).

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

*Revised 2026-10-17 ("Driving levels" below): the pointer no longer steers the autopilot; any user
steering hands over to cruise, which keeps the speed, and the autopilot does not take the wheel
back (the idle / dead-zone re-lock is gone). Q / E step its speed on the pedal ladder. The
steering law below is now the cruise's. Revised again the same day ("Pointer wish" below): the
pointer no longer hands over either; its direction is a coarse wish for the next targets.*

Requested by the user ("an auto-zoom, like an autopilot, so that you can just navigate with
the mouse"). All policy in the core: `crates/almondbread-core/src/autopilot.rs`, owned by the
`Navigator`; the renderer reports feedback and surveys; Swift decodes events and shows the
HUD.

| Input | Action |
| --- | --- |
| Z, menu "Autopilot On/Off", the cockpit's plane icon (the macOS toolbar button was removed 2026-10-17, the iOS round button 2026-10-18) | on (eased start) / off (eased stop); Z key repeats are ignored |
| Esc, menu "Stop Autopilot or Flight" | eased stop; not handled (the system keeps Esc) while nothing flies |
| Pointer offset from the view centre | steers like a joystick (below) |
| Click (iOS: tap) | pause / resume (eased); a double click undoes its first click's toggle and zooms as usual; further clicks of the stream zoom on and never toggle |
| Q / E, mouse wheel notch, trackpad scroll | speed ×0.8 / ×1.25, ×1.1 per notch, `e^(0.004·dy)` per scrolled point; 0.1–4 e-folds/s, default 0.9 (×2.5/s); momentum scrolls are ignored |
| Space, W A S D, drag, pinch | work as before on top of the autopilot (W A S D: keyboard mode, which flies straight) |
| R | stops it dead (reset) |
| iOS finger | a resting finger is the wish, a swipe pans the picture without handing over (since 2026-10-18, "Touch" ▸ "Rest or swipe"; first: a one-finger drag steered instead of panning) |

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
  texel · pixels))` unclamped, its log filtered (τ = 0.5 s down, 1 s up); `fit ≥ 1/10` → 1,
  `fit ≤ 1/16` → 0.3, smoothstep between (revised 2026-10-17, "Steady autopilot rate" below:
  it was 1/4 → 1, 1/10 → 0.3, unfiltered); 0.06 while the view waits for a deep reference
  (frames show only the reprojected history). Down with τ = 0.25 s, back up with τ = 1 s.
  Steering pan is scaled by the throttle too. The same throttle paces fly-to and journey
  flights and forward cruise.
- *safety* (the heading region of the current frame): a GPU `survey` pass over each finished
  level (at most every 0.1 s, only while the autopilot is on) counts per cell of a 16 × 10
  grid the sampled texels (≤ 16 × 16 per cell), interior or unresolved ones, and neighbour
  pairs whose class or integer escape count differ. Band crossings per view width =
  edge rate × level width. Around the heading (Gaussian, σ = 0.15 of the smaller side):
  `detail ≤ 3` → safety 0.3, `≥ 12` → 1, smoothstep between; the hint says "interior" when
  most of that region is interior, else "featureless". At the depth limit the zoom stops.
- *HUD*: "autopilot ×2.46/s · now ×1.3/s" plus the reason ("slower: the renderer is
  behind", until 2026-10-17 "slower: keeping the detail sharp"; "waiting for the deep reference", "slower: heading into the interior",
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
target lock", on by default, persisted; off = the pure joystick; script event `{"lock": false}`;
the env override `ALMONDBREAD_AUTOPILOT_LOCK` and the CLI `--lock` were removed 2026-10-17).
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

### Survey steering (as built, 2026-10-17; default since the user's foreground comparison the same day)

Still an env switch (`ALMONDBREAD_SURVEY_STEER=1`, not yet decided by the user). Its boundary-point pick is also what lane assist uses in cruise ("Driving levels" below).

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

### Steady autopilot rate (2026-10-17)

User report: with the default speed the cockpit's rate readout "varies a lot between ×1.75/s and
×4.x per second" (autopilot near re −1.76866783140550194…, im 0.00164555778036987…, from 1e240,
survey steering on). The readout is the rate spring's value; ×4.x is above the default set speed
(×2.46/s), so the user's set speed was higher (×5 rung: 0.35 · 1.61 e-folds = ×1.75), the
factors are what matters.

**Diagnosis** (`fly --out null --hz 120 --log`, 90 s, 1600 × 1000, `ALMONDBREAD_SURVEY_STEER=1`,
`ALMONDBREAD_TRACE=1` feedback lines; `fly --log` gained the columns `ap_fit`, `ap_capture`,
`ap_waiting_ref`, `ap_survey_steer`; measured 2026-10-17, M1-class MacBook Pro, release, GPU
67–85 % busy with other work). In 3 base runs the throttle alone moved the rate: sd of ln throttle
0.25–0.34, safety 0–0.24 (one run with a 2 s rescue), lock capture ≤ 0.11; hint "Rendering"
30–81 % of the time; rate ×1.32–×2.46 at the default speed. The throttle followed `fit`, and
`fit` reads back the step the scheduler runs at: median fit at the 1/8 preview 0.137, at 1/4
0.238, 3/8 0.347, 1/2 0.45, 7/8 0.81. A gesture frame fills the budget at any step (and the GPU
clocks follow the load, [[synthesis/benchmarks]] "Autopilot throttle and GPU clocks"), so
`fit ≈ step × √(budget / GPU time)`: no measure of headroom. With the old mapping (1/4 → 1, 1/10
→ 0.3) the 1/8 ↔ 1/4 probe toggling (every 0.5–4 s) swung the throttle 0.36 ↔ 0.95, and cheap
regions (5 iterations per texel under BLA, previews at 7/8) ran at full speed between them.
Slowing does not help the renderer: every autopilot frame is a fresh preview at the scheduler's
step, whose cost does not depend on the zoom rate. Smoothing alone (input τ up to 8 s,
simulated on the recorded fits) left the sd at 0.18 e-folds/s: the swings were seconds long.

**Fix.** The throttle now slows only when even the coarsest preview misses the budget: full at
fit 1/10 (1/8 step at 1.56× the budget = 0.78 of the refresh interval, since the gesture budget
is half of it), floor 0.3 at 1/16 (4× the budget: every other refresh). The fit's log is
filtered (τ 0.5 s down, 1 s up) so one slow chunk of another process does not dip the rate; a
lasting overload still slows within about a second (test
`throttle_is_steady_while_frames_fit_and_drops_on_a_real_overload`: 1/8 ↔ 1/4 toggling with
30 ms spikes keeps ≥ 0.97 of the speed at 30/60/120 Hz; fit 1/16 halves the rate within 1.5 s;
30 and 120 Hz within 0.04 e-folds/s mid-transient). Reference waits and the depth limit are
unchanged; safety, capture and rescue untouched. No switch (the task asked for none): needs the
user's foreground check.

**Before / after** (interleaved, frozen binaries, 120 Hz, default speed 0.9 e-folds/s; rate
statistics after 3 s):

| run | rate mean ± sd (e-folds/s) | 0.5 s bins peak-to-peak | p5–p95 | throttle mean | uniform / rescue | late refreshes | zoom in 90 s |
|---|---|---|---|---|---|---|---|
| user point, base ×3 | 0.52 ± 0.18, 0.70 ± 0.33, 0.76 ± 0.15 | 0.62, 2.06, 0.53 | ×1.36–2.43, ×1.36–2.46, ×1.58–2.46 | 0.59, 0.87, 0.86 | 0 / 0, 4.6 % / 2.1 s, 0 / 0 | 36, 129, 53 | 19.6, 26.4, 28.8 decades |
| user point, new ×2 | 0.87 ± 0.085, 0.88 ± 0.064 | 0.49, 0.38 | ×2.00–2.46, ×2.11–2.46 | 1.00, 1.00 | 1.0 % / 0, 0.8 % / 0 | 146, 292 | 32.8, 33.2 decades |
| seahorse 1e2, 60 s, base | 0.65 ± 0.22 | 0.59 | ×1.38–2.46 | 0.73 | 0 / 0 | 8 | 16.1 decades |
| seahorse 1e2, 60 s, new | 0.899 ± 0.002 | 0.015 | ×2.45–2.46 | 1.00 | 0 / 0 | 2 | 22.2 decades |

What still moves the rate at the user point after the fix is real: the safety in short
featureless stretches (≤ 0.6 s uniform, safety down to 0.53) and the lock's capture slow-down
while it turns towards a far aim point after a target or survey-steering hand-off (down to 0.44
for ~1 s). Presented frames per run were the same as the first base run (9437–9536 vs 9557 of
10 440; the other two base runs 10 114 and 10 328); late refreshes rose in the new runs (146,
292 vs 36–129): the dips come from seconds with cheap views at 7/8–1 preview steps (presented
51–100 per second), where the throttle was 1 in the base runs too, and from the deeper views
(1e271 vs 1e258–1e267); not separable from the shared GPU's load in 5 runs. Interaction test
"autopilot" ×4.3 in 2.4 s (2 runs, OK), "autopilot lock" OK.

### Pointer wish (as built, 2026-10-17)

The user asked whether the autopilot could take the coarse direction of the mouse into account,
after a nudge of the mouse had thrown them into cruise. Core: `Autopilot::set_wish`, `Wish`
(`autopilot.rs`), the lock's search and result choice (`autopilot/lock.rs`), the navigator
feeding the pointer (`input.rs`).

- **What counts.** In pointer mode, with the pointer on the canvas and at least `WISH_TRAVEL` =
  24 points of travel since A took the wheel (a pointer resting off-centre when Z was pressed is
  no wish). Past 0.2 of the steering radius (kept down to 0.15: no flicker at the edge) its
  direction is the raw wish. No idle timeout: a pointer left on the right keeps wishing right.
- **Hysteresis.** The raw wish becomes the *settled* wish after 0.3 s; a settled wish changes
  only when the pointer points more than 45° away (or appears / goes) for 0.3 s. Each new
  settled wish has a new *epoch*. A wobbling hand or a brief swing to the other side changes
  nothing (test `wish_settles_with_hysteresis`).
- **Lock search.** Candidates get `+1.5 × agreement` on their pre-score (agreement = the cosine
  between the candidate's direction from the searched view's centre and the wish, faded to 0
  within 0.08 spans: straight ahead is neutral), so the 6 probed candidates are mostly in the
  wished direction. Of the targets that probe aimable, the best one in the wished half
  (agreement > 0.1) is taken; none there: the best anywhere (finding *something* beats the
  direction). The weighting alone was not enough: a well-probing target elsewhere often won on
  probe quality (2 of the 20 view × direction cases of the unit test views, weighting only).
- **Re-target.** When the epoch changes and the current target does not already lie that way
  (agreement ≥ 0.5 keeps it), one pickup search runs from the current view (off-thread as
  always); its best target replaces the current one only if it agrees ≥ 0.25. A search still
  running under the old wish is cancelled, a `next` target chosen under it dropped. Once per
  epoch, so the aim does not jitter (test `a_changed_wish_retargets_once`, 30 / 60 / 120 Hz).
- **Survey steering.** `Survey::boundary_point` weights each detailed cell by
  `exp(2.5 (agreement − 1))`, centres its distance preference 0.25 of the shorter side ahead in
  the wished direction, and takes the best detailed cell in the wished half first; uphill takes
  the highest peak in the wished half. A new epoch re-picks once, from the centre. **While the
  heading region is uniform** the wish is ignored (nearest detail anywhere): with it, the deep
  test path at 1e239, where the first lock target runs out, picked sparse cells in the wished
  half and backed out (rescue) in 6 of 10 runs, never without a wish.
- **Never the pointer itself.** The heading always aims at a found target or boundary point
  (`aim` gets no pointer in A); the view does not follow the pointer as in cruise.
- **HUD.** The lock label gets " · your direction" while a wish is settled ("towards spiral
  M(68,2) · your direction").
- **Hand-over.** The pointer no longer hands over in A (W A S D, a drag, the stick and the pedal
  to zero still do; test `autopilot_hands_over_to_cruise_and_never_takes_the_wheel_back`).
  *Journey legs keep the pointer hand-over* (60 points, 0.35 radius, 0.6 s): a leg is a planned
  flight to one target, there is no choice of targets for a wish to bias, and steering a leg
  by hand is how the user leaves the plan.

**Measured** (headless `almondbread fly --out null`, 1600 × 1000, 40 s, Z at 0.3 s, the pointer
moved to (±350, 0) points at 0.5–1 s and held; machine under heavy load from other sessions,
load 7–24; measured 2026-10-17, M1 Pro, release). "Aims" counts every new lock target and
survey pick by where it lay in the view it was chosen from (a target from the search ahead:
relative to the previous target at its end span), in the left or right half, or straight ahead
(within 0.05 spans):

| Path | Config | runs | uniform % (mean) | rescues | aims in wished / other half (+ ahead) | re-targets |
| --- | --- | --- | --- | --- | --- | --- |
| seahorse 1e3 → 1e18 | no wish | 5 | 0 | 0 | 3 right, 0 left (+18) | 0 |
| seahorse | wish right | 5 | 0 | 0 | 15 / 0 | 0 |
| seahorse | wish left | 5 | 0 | 0 | 21 / 3 | 1 per run |
| user's deep path 1e237 → 1e252 | no wish | 7 | 0.1 | 0 | 29 right, 21 left (+8) | 0 |
| deep | wish, survey steering always with the wish | 10 (aims: runs 3–5) | 7.3 | 6 | 167 / 17 (91 %) | 0–1 |
| deep | wish, soft weight only while uniform | 8 | 8.3 | 5 | 259 / 44 (85 %) | 0–1 |
| deep | **wish, ignored while uniform (as built)** | 8 | 2.8 | 1 | 78 / 49 (61 %) | 0–1 |

Seahorse aims are counted over runs 3–5 only (the halves metric came then); the 3 "other half"
aims with the left wish are the first pickup, made before the wish settled (1.3 s). There the
wish is followed fully and costs nothing: every target lies the wished way, no featureless time.
The deep path is the hard case (all candidates are probe "boundary detail" texels, many rejected;
kb "Survey steering"): with the wish followed everywhere, 91 % of the aims went the wished way but
6 of 10 runs backed out of a featureless region at 1e239, where the first target runs out and
survey steering then took sparse cells in the wished half. Ignoring the wish while the heading
region is uniform brings that to 1 rescue in 8 (the remaining one 1.1 s) but only 61 % of the
aims go the wished way (no wish: 58 % right, 42 % left), because many aims on that path are survey
picks made while uniform. Decision: finding *something* beats the direction; the wish is a
preference, not a constraint. Zoom reached within 1 decade of the no-wish runs (1e251–1e252.8).

## Target marker (as built, 2026-10-17)

The user's extension of the pointer wish: show where the autopilot is heading. Core:
`Navigator::aim_marker` (`AimMarker`: the lock target, or survey steering's pick, while A runs;
the leg's target while a journey flies a leg; the settled wish direction; None in M and C),
`TargetKind::marker_label`; drawn by `crates/almondbread-render/src/marker.rs` into the overlay
scene (like the compass), updated by the host each refresh.

- **Ring.** A light ring (radius 9 pt, 1.5 pt, a dot in the middle) over a dark halo ring:
  reads on bright and dark palettes. It sits on the target's complex point, so it moves with the
  zoom and pan by itself; when the target changes it glides from where it was drawn to the new
  one (0.6 s, smoothstep, from the drawn point as a complex point, so the zoom during the glide
  is right too); it fades in and out over 0.35 s. A target off the canvas is drawn at the edge
  in its direction.
- **Label.** For a known kind, a short label next to the ring for 2.5 s after the target
  changes, then fading: "spiral", "branch point", "minibrot · period 34", "Julia stage · period
  68"; none for boundary detail, survey picks, resurfacing and cross-overs.
- **Wish line.** While a wish is settled, a faint line (white over a dark halo, 45 % alpha) from
  16 points off the centre to 0.22 of the shorter side in the wished direction, so the user
  sees whether the target agrees.
- **Setting.** View ▸ "Autopilot Target Marker", control center Overlays "Autopilot target
  marker", persisted with the overlay settings (`targetMarker`), **on by default** (it is the
  only feedback that the wish does anything, and it is small); the overlay resets put it back
  on. FFI `Explorer.set_target_marker(on)`.
- **Cost.** Two rings, a dot, two segments and a label in the overlay scene; the scene is
  rebuilt every frame while it moves (its key carries the position). Interaction test
  "autopilot": draw CPU p50 0.23 ms, 114 fps at 120 Hz.
- Tests: `marker_follows_the_target` (stays on the target through zoom and pan, glides without
  a jump and arrives after 0.6 s, fades out, nothing while off); the navigator test checks
  `aim_marker` in A (with the wish) and None in C. Screenshot: `--ui-demo=autopilot-wish`
  (synthetic pointer held right; the real mouse is never moved).

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
2.9 s.) *Revised 2026-10-17 ("Journey", smooth legs below):* the landing rides
`v = c · s_left^(2/3)` (speed `(1 − t/T)²`, `T = 0.8 s · √(v / 1.4)` within 0.8–1.5 s) and a
zoom-out-and-in path brakes ahead of its apex onto `k cosh x` (`FlyPath::turn_limit`). **Reference
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
view resolved). `autopilot --center sea-8007 --scale 2.5e-34 --seconds 60`, whose CPU
survey escalates the same way now: uniform 5.0 % → 0 %, rescue 2.6 → 0 s, 23 → 12 target
switches, 1e52.0 → 1e54.4 (the first search: 0 → 10 candidates); lock off: 94 % uniform.
(The switch `ALMONDBREAD_PROBE_ESCALATE=0`, which restored the fixed limit, was removed 2026-10-17.)

## Journey (as built, 2026-10-15)

Requested by the user: an endless, planned, random-but-seeded flight from landmark to
landmark, with targets chosen from the maths rather than by taste. Core:
`crates/almondbread-core/src/journey.rs` (planner and state machine, owned by the
`Navigator`); legs are ordinary fly-to flights ([[sources/van-wijk-nuij-smooth-zooming]]).

| Input | Action |
| --- | --- |
| J, menu "Journey On/Off", the cockpit's sparkles icon, control center "Journey" (the macOS toolbar button was removed 2026-10-17, the iOS round button 2026-10-18) | start with a random seed (1 … 999 999) / resume after manual input / stop |
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

**Revision 2026-10-17: smooth legs.** User report (build c553b78, likely with E raised): "a bit
jumpy … when just zooming in, it gets faster, then it brakes a bit, then accelerates again …
zooms out super fast, comes to a very abrupt stop, then zooms in with high speed and also does
a full stop". Reproduced headless with `fly --log` (journey #4711, 1920 × 1080, 60 Hz, zoom
rate per refresh, measured 2026-10-17, M1 Pro, release). The renderer throttle was not
involved (≥ 0.94 throughout); the motion was, all of it in code older than the driving levels
(cba2d79 showed the same at raised speeds: 728 e-folds/s² at ×1.25⁶, 1448 at ×1.25¹⁰; at the
default speed its #4711 legs, planned from the older catalogue, just happened to be calm):

- *Dead stops.* The landing `v = √(2 a s_left)` through the ω = 4 spring lagged the curve and
  reached the target still moving: every leg ended from 1.4–25 e-folds/s in one frame.
- *Dull pace into the landing.* A dull stretch up to the target (the new needle anchors, Pine
  Compass) flew at 3× until the brake: 10.3 e-folds/s to rest in 0.3 s at the default speed.
- *Pulses.* A one-sample gap in a dull stretch: −10.7 → −8.6 → −10.7 e-folds/s within a second.
- *Turns.* The van Wijk apex at a constant path speed swings the zoom rate from −V to +V in
  ~2/V s: at ×35/s with the long-leg and dull paces −28 → +23 e-folds/s in 0.15 s.

Fix (`flyto.rs`, `journey.rs`): the landing rides `v = c · left^(2/3)` from the frame the spring
would outrun it, in closed form (speed `(1 − t/T)²`, `T = brake_time(v)` = 0.8 s · √(v / 1.4)
within 0.8–1.5 s for the speed v actually flown, so a short leg that never reaches the set speed
brakes as briefly as it flies; the deceleration eases to zero at the target); a path with an
apex brakes ahead of it onto `k cosh x` (x = ρ (s − apex), `k` such that x = −1 … 1 takes
`TURN_TIME` = 1 s, so the swing through zero does not depend on the speed) at `speed / brake_time` (in
path units), riding that line when the spring lags (`FlyPath::turn_limit`, convex, minimised
in closed form); the dull pace bridges gaps of fewer than 3 samples, ramps linearly between
samples and is 1 over the last 3 samples; the pace factors (long legs, dull) raise a leg at most
to 16 e-folds/s (`PACED_MAX`; a raised journey speed itself stays). Before → after, #4711 over
100 s (main f5ff17b → this): default speed, worst zoom acceleration over 0.1 s 41 → 17
e-folds/s² (the landing's first braking from a paced 10.8 e-folds/s), every landing from ≤ 0.2
of the peak; E × 6 (×35/s): 462 → 23; E × 10 (×1000/s): 1069 → 20, peak rate 55 → 16 e-folds/s;
no pulses (main: one per run). The default-speed cross-over to Pine Compass brakes −3.5 → 0 over
0.9 s and is back at +3.4 after 1.8 s (before: ±3.4 in 0.6 s, then a landing from 9.7). Legs take
longer: that cross-over 13.1 → 14.8 s; at E × 10 a short leg that zooms out 1 e-fold first
(from the seahorse view at 1e5) 2.3 → 3.3–5.1 s, so the interaction test's "journey stops" now
runs 7 s (`Scenario.seconds`). Tests:
`cross_overs_turn_and_land_smoothly_at_any_speed` (1.4 / 4 / 16 e-folds/s at 30 / 60 / 120 Hz),
`journey_legs_have_no_pulses_or_dead_stops` (#4711, default and E × 6; fails on c553b78:
lands at 1.44 of 3.09 e-folds/s), `dull_pace_bridges_gaps_ramps_and_lands_at_the_journey_speed`.

*Hand-over and resume* (see "Driving levels"): the ×55/s the user saw in C was the leg's paced
rate taken over as the cruise's set speed (a deep cross-over flies 1.4 × 3.1 × 3 at the default
speed); now the cruise's set speed is the journey speed (the rate eases there). The journey's
and the cruise's speeds were already separate. J (and a choice at a stop) used to halt the
cruise dead and start the leg from rest; now the cruise is released and glides out, and the
next leg starts once it carries the cruise's direction (taking its rate over) or the cruise is
below 0.15 e-folds/s (`journey_resumes_from_cruise_without_a_dead_stop`, both directions).

## Driving levels (as built, 2026-10-17)

The user's design, like a car's gears: four levels, shown in the HUD cockpit as four icons (first built as the letters `M · C · A · J`, replaced by icons the same day, see "Cockpit").
Core: `crates/almondbread-core/src/cruise.rs` (`Level`, `LADDER`, `pedal`, `Cruise`), the
hand-overs in `Navigator` (`level`, `take_wheel`, `pedal`, `cruise_landing`, `cockpit`); the
cockpit is drawn by Rust (`crates/almondbread-render/src/cockpit.rs`). Replaces ⇧Z ("Zoom-out
autopilot" below), its menu item and the iOS long-press.

| Level | What drives | Enters | Leaves |
| --- | --- | --- | --- |
| M manual | the user only | rest | E / Q (C), Z (A), J (J) |
| C cruise | a set speed on one signed axis; the user steers | E / Q from rest or at a journey stop; steering or pedal ≤ 0 in A or J | Esc or pedal to zero (brakes, then M); Z (A); J; R |
| A autopilot | speed and steering (target lock; the pointer is only a direction wish) | Z | W A S D, drag, stick or pedal ≤ 0 (C at the current speed); Z / Esc (eased stop) |
| J journey | planned legs (fly-to flights too) | J, a fly-to | as A for a journey leg; fly-to keeps its old take-over |

**Pedals.** Q / E are one signed speed axis. Each press is one rung of `LADDER`, the cockpit's
gears 1–26 (factors per second 1.1, 1.2, 1.35, 1.5, 1.75, 2, 2.5, 3, 4, 5, 6.5, 8, 10, 13, 16,
20, 27, 35, 45, 60, 80, 100, 150, 250, 400, 1000; ln of them in e-folds/s; 12 rungs from ×1.5
until 2026-10-17, the user asked for finer steps), E towards "in", Q towards "out"; off-ladder
values (the autopilot's 0.9) step to the neighbouring rung. The autopilot's and journey's speed
ranges start at 0.09 e-folds/s, below gear 1 (0.095): with 0.1 a Q at the bottom stepped to
×1.1, was clamped back to 0.1 and never reached zero (found by the hand-over tests).
Holding steps again after 0.4 s and every 0.15 s (0.25 s before the 26 gears; stepped by the navigator per frame; the Mac
key repeats are swallowed, else a repeat would count as a new press). Towards zero the speed
stops *at* zero (detent): repeats never cross, a new press does. In A and J the pedals step that
level's speed on the same ladder (E above the top rung multiplies by 1.25 up to the level's
range); down to zero the cruise takes over at zero and brakes. During a fly-to Q / E set its
speed as before. **Decision on the old Q / E meaning** (Space zoom speed): Q / E with Space held
still set the Space speed; otherwise they are pedals. Scroll and wheel keep their behaviour (pan
/ zoom in M and C; speed in A and J). The gamepad's shoulders are the pedals.

**Cruise motion.** The signed rate follows its target on the autopilot's rate spring (ω 3 /s
speeding up, 4 /s braking): a press eases in over about a second, Esc or pedal-to-zero brakes on
the spring's smooth curve (glide 2/ω of the rate; test `cruise_esc_brakes_on_a_smooth_curve`).
Steering is the autopilot's joystick law (dead zone 0.1 R, heading spring ω 6, zoom about the
heading, pan `−k h`, `k = (0.5 + z⁺)` /s) driven only by the pointer (moved within 3 s, pointer
mode) and W A S D panning; nothing pulls the view (test
`cruise_steers_the_pointed_content_to_the_centre`). Forward, the renderer back-off (throttle)
scales the rate and the depth limit stops it. Integration in ≤ 2 ms substeps; the zoom centre
is the heading weighted by `e^{−z_j}(1 − e^{−dz_j})` per substep and the pan is converted to
the frame's midpoint scale, so the composed frame equals the substeps exactly.

**Zoom-out dynamics.** In reverse `rate = set · (1 + D/3)^{share · ramp(t)}`, D the e-folds
below the fitted home view, `ramp` a smoothstep over 10 s since reversing began (4 s until
2026-10-17): the log of the speed accelerates evenly (no jump on one press). The exponent's
`share` (`cruise::boost_share`) grows linearly with the gear, 0.5 in gear 1 to 1 in gear 26
(user, 2026-10-17: "with lower speed setting, the deep zoom factor should not kick in so
heavily"), so the low gears are deliberately slow. With the full boost (top gear) `D + 3` decays
exponentially, home in `(3/set) ln(1 + D/3)`: logarithmic in the depth; with share s < 1 the
time grows like `D^{1−s}`. Seconds home (cruise unit model, 60 Hz, no steering, no throttle,
`reverse_boost_depends_on_the_gear_and_builds_up_gradually` checks the bold ones; measured
2026-10-17, release):

| Gear | from 1e-10 | 1e-50 | 1e-69 | 1e-300 | 1e-3000 |
| --- | --- | --- | --- | --- | --- |
| 1 (×1.1) | **126** | 337 | 405 | 900 | 2964 |
| 4 (×1.5) | 31 | 73 | 86 | 173 | 496 |
| 6 (×2) | 19 | 41 | **48** | 90 | 234 |
| 10 (×5) | 10 | 18 | 20 | 33 | **69** |
| 16 (×20) | 7 | 11 | 12 | 15 | 23 |
| 22 (×100) | 5 | 8 | 9 | 10 | 11 |
| 26 (×1000) | 4 | 7 | 7 | 8 | **10** |

Throttle applies
in reverse only down to 0.5 (zooming out mostly shows the shrinking history), the rate is capped
at 2000 e-folds/s. Once the path home (`FlyPath` length × ρ) is no longer than 1.25 s of the
current rate (the soft landing's brake curve then matches the rate), or a braking glide would
reach home, or the depth is gone, a fly-to flight with `Flight::soft_landing` takes the last
stretch at that rate and rests exactly on the home framing (centre −0.75 too); level stays C.
Zooming out stops at the whole set; Q from home does nothing. Measured in the navigator tests at
30 Hz, gear 10 (×5/s out) from the seahorse spiral, release build, 2026-10-17 (the 12-rung
ladder's first rung ×1.5 took 17, 29, 42 and 58 s):

| From | Home after | Landing from | Peak rate |
| --- | --- | --- | --- |
| 1e-10 | 9.8 s | 7.3 s | 3 e-folds/s |
| 1e-50 | 19.0 s | 16.5 s | 13 e-folds/s |
| 1e-300 | 33.8 s | 31.3 s | 54 e-folds/s |
| 1e-3000 | 69.9 s | 67.4 s | 287 e-folds/s |

Each zooms out monotonically, moves less than 1.5 e-folds in the first second, eases out over
the last second and rests bit-exactly on home. 30 / 60 / 120 Hz agree to 2e-3 of the zoom and
3 points of the steered centre after about 9 e-folds out in gear 10 (the boost depends on the
depth the zoom changes: first order in the substep length).

**Pitfall: the soft landing stalled short of home** (found with gear 10 from 1e-50, fixed
2026-10-17). `Flight::step_soft` rode its brake curve `v = √(2 a left)` only on frames where the
speed spring would have outrun it; on the next frame it handed back to the spring, which carried
the curve's deceleration `a` as its own rate of change, undershot to a standstill about 1e-4 of
path short of the end, sat there for 1.5 s and then snapped (0.0045 e-folds). Once on the curve
the flight now stays on it (`Flight::soft_curve`), so it lands in the frame the curve ends.

**Hand-overs.** User steering (W A S D / arrows, a drag or touch pan (in A a touch swipe pans without handing over since 2026-10-18, "Touch" ▸ "Rest or swipe"), the gamepad's left stick;
during a journey leg also the pointer deflected past 0.35 of the steering radius for 0.6 s after
moving 60 points since the leg took the wheel: a pointer crossing the canvas to a button does
not count; *in A the pointer no longer hands over since 2026-10-17, it is a direction wish*,
"Pointer wish" above) while the autopilot runs or a journey flies a leg engages the cruise
with the current zoom rate and heading (A: set = the autopilot's set speed; J: set = the
journey speed in the flight's zoom direction, at most the flight's rate, the journey suspended,
J resumes after the cruise glides out or carries on into the leg; revised 2026-10-17, "Journey",
smooth legs). The autopilot **no longer takes the wheel
back**: the idle / dead-zone re-lock is gone because the autopilot is off after the hand-over; Z
re-engages (at the cruise's speed if it went forward). Inside A the pointer never steers the
view; its direction only biases which targets the lock and survey steering choose. Other manual input (Space, pinch, wheel zoom,
rotate) works on top as before. A journey stop counts as rest: E / Q there start a cruise and
the journey stays at its stop (J / Z / X still choose; the cruise glides out, and the next leg
starts when it carries the cruise's direction on or the cruise is at rest). Esc stops the cruise first. Tests: `autopilot_hands_over_to_cruise_and_never_takes_the_wheel_back`,
`steering_or_pedal_to_zero_drops_a_journey_to_cruise`,
`autopilot_recovers_after_cruising_into_a_featureless_region` (cruise into the cardioid, Z: the
rescue backs out).

**Lane assist** (key L, free on the macOS and iOS key maps; control center "Lane assist" in
View; menu; the cockpit's lane icon; persisted `laneAssist`, default off): active only in C, and only
while the user does not steer. Phase 1 (this build): setting, cockpit lane icon (dim off, lit on,
brighter in C, a glow for 0.6 s after a nudge) and the hook `Navigator::assist_nudge(view,
radius, now) -> Option<heading>`. Wired 2026-10-17: the renderer surveys frames while assist
is on in C (`Navigator::wants_survey`); when the survey (at most 0.5 s old) rates the centre
uniform (danger > 0.5 or mostly interior), the nudge is a heading towards the survey's boundary
point (`Survey::boundary_point`, the void-steer work), at most 0.6 of the steering radius; the
cruise's heading spring smooths it. Detail ahead: no nudge. The HUD then says "The Nothing
ahead – steering away". Not yet tried in a foreground window (default off).

**Cockpit.** About 306 points wide at the bottom centre of the canvas (right of the cosmic scale,
left of the iPhone button column), shown with the HUD. Revised 2026-10-17 after the user's
review ("M C A J … I don't think people understand what those mean", and a set-speed notch the bar
overshot during the reverse boost): four icons (steering wheel = manual, speedometer = cruise,
plane = autopilot, sparkles = journey), clickable since 2026-10-17 (manual stops = Esc, cruise starts cruising in = E, autopilot and journey toggle = Z, J), the current one lit and
named ("Cruise"); a gauge with zero in the middle showing only the rate flown (reverse left, log
scale to the top rung); the rate as text ("↓ ×4.2/s") with a small line under it, lit for 1.2 s
after Q / E (user-approved 2026-10-17, replacing "set ×1.5"): in C the gear, always while one is
set ("gear 5" forward, "reverse 3" zooming out, "· boost" while the depth boost adds more than
5 %); in A and J the set speed, "target ×20", only while the rate flown differs from it by more
than 5 % (speeding up, braking, the renderer back-off; hidden again within 3 %, so it does not
flicker). In J the cockpit's set is now the journey speed and its rate the leg's flown path speed
(`Flight::speed` × ρ; before, both were the paced leg speed, so set and rate were always equal
and the set changed every frame). Metrics rebalanced for the longer line ("reverse 26 · boost"
is 72 points at 8 pt): value slot 72, gaps 5, icon pitch 19, padding 7: 306 points wide; the
lane icon; a
(?) badge (help topic `cockpit`: the levels and their keys). A manual zoom (wheel, trackpad,
pinch, double tap, menu) while cruising stops the cruise dead and drops to manual (user request:
zooming by hand must not fight a set speed). Dark translucent plate, haloed text: legible on bright and dark
palettes. FFI `ExplorerStats.cruise` (`CruiseStatus`: level, set, rate, boost, landing, assist,
nudging), `Explorer::set_cockpit(on, inset_right, inset_bottom)`, `set_assist(on)`;
`FRAME_AUTOPILOT` is set in A and C (an iOS finger steers). HUD wording: the uniform-region hints
(interior / featureless / rescue) read "The Nothing ahead – slowing down / turning back".
Screens: `scripts/ui-screenshots.sh` scenes `cruise-reverse` and `autopilot`. Interaction test
"cruise out" replaces "zoom out"; the "autopilot" scenario now checks the hand-over.

## Seek minibrot (as built, 2026-10-17)

Requested by the user: one key that finds the simplest minibrot near the view and flies there,
and again from there the next smaller one. Core: `crates/almondbread-core/src/seek.rs` (`seek`,
the pure bounded search; `Seeker`, its thread and HUD state, owned by the `Navigator`); FFI
`NavKey::SeekMinibrot`, `ExplorerStats.seek` (`SeekStatus`: phase, period, target zoom).

| Input | Action |
| --- | --- |
| N (free on the macOS and iOS key maps), View ▸ "Seek Minibrot (N)", iOS: control center (View) "Seek minibrot" and the long-press menu's "Seek Minibrot Here" (the plane button's menu went 2026-10-18) | search off the main thread ("Seeking a minibrot…"), then a normal fly-to to the minibrot's framing; HUD "Minibrot, period 4640 · 1.0e69× · 42 %", for 6 s after landing without the percentage |
| N again from a minibrot's framing | the next smaller one around it |
| Esc, any other key, click, scroll, any navigation | cancels the search (Esc is then handled); during the flight Q / E / scroll set its speed and the rest takes over as from any fly-to |
| nothing found | "No minibrot found nearby" for 3 s; nothing moves |

No toolbar button: the macOS toolbar already holds eight items, and N plus the menu item are
the discovery path (help: keys list, the controls popover, "Seek a minibrot" on the
autopilot/journeys page).

**Search** (all bounded: 4 s wall clock, a cancel flag, at most 3 discs × 6 retries in the view,
1 + 8 discs × 6 around a minibrot, periods ≤ 200 000; [[techniques/hang-prevention]]):

1. Ball period detection over discs about the view centre of radius 1, 3 and 9 view spans
   (shorter side; radius 1 covers the whole view up to 16:9), lowest period first, Newton for
   the nucleus from the disc centre within 16 radii (as `almondbread locate`). The nucleus is
   refined until its precision is −log2 σ + 96 bits *and* Newton's last step is 2^-48 σ or
   smaller: Newton can stall at half the working precision, and such a "nucleus" (found at
   period 18560 near the needle) gave a size of 1e-83 instead of 1e-70 and failed every test.
   Newton landing on the nucleus of a divisor of p (z_p = 0 there too) shows as a size estimate
   of 0 or above the whole set's and is skipped.
2. **Minibrot, not bulb.** Ball detection mostly returns satellite bulbs (2p, 3p … of a
   minibrot in the disc). With the complex size s (a point w of the whole set maps to
   nucleus + s w), a cardioid is interior at w = −0.6 with a period-p attracting cycle
   (|λ| ≈ 0.84), a satellite (a disc of radius ~0.5 |s|) has its own period-2 bulb there.
   Perturbation about the nucleus's periodic orbit (extended range, rebased at each period and
   when the orbit comes back near 0) for at most 160 periods: the orbit after successive
   periods settles to 1e-4 → minibrot; settles every second period → satellite; escapes →
   neither. The first test tried w = 0.4 beyond the cusp (escapes for a cardioid): it failed
   for the airship at −1.7549, whose cusp, like every embedded minibrot's, sits on the filament
   it hangs from (the real axis).
3. A minibrot whose framing (2.8 |s| on the shorter side) is at most half the view is the
   target. One wider than that, within 2 spans of the centre and under 8 views wide, is the one
   in view: the search goes on to a smaller one around it. Larger ones (the view is in their
   decorations) are skipped.
4. **Around a minibrot of period p:** the copy of the period-3 minibrot on its antenna (tuning:
   period 3p at w = −1.75488, Newton directly, no ball search: from period p + 1 the ball over
   that small disc inflated past the bailout and found nothing), and ball searches from period
   p + 1 in eight discs of radius |s| at |w| = 2.6 (the structure it sits in) for a lower
   period than the best so far, skipping 2p (its period-2 bulb). Only minibrots under 0.6 of
   its size count; the lowest period wins.

**Framing.** Centre at w = −0.55 (inside the cardioid, between cusp and period-2 bulb), the
shorter side 2.8 |s|: the minibrot with its antenna (w = −2 … 0.47 along its axis) fills most of the
view (the first 3.2 |s| left the body at half the window height in the needle flight); the
view's angle is kept.

**Numbers** (measured 2026-10-17, M1 Pro, release, machine under heavy load): home → the
period-3 airship (0.1 ms); N again → the period-7 minibrot at −1.6741; the needle view (scale
4.2e-48 per point) → period 4640, size 1.0e-67, ~7 view radii off (0.02 s); N again → period
13920 (its airship copy, size 2e-69, 0.1 s; before skipping 2p and capping the ring's periods at
the best so far: 0.9 s); seahorse valley at span 1e-6 (inside the period-39 minibrot's
decorations, which is larger than the view) → period 117 (2 ms). Headless
`scripts/flights/seek-needle.json` (1280 × 800, N at 0.5 s and 34 s) ends on the period-13920
framing; rendered with 3e6 iterations its centre is interior.

**Tests.** `seek::tests`: home finds the airship and from its framing a smaller one; bulbs
(period 2, the 1/3 bulb, period 4 at −1.3107) are not minibrots, the cardioid and airship are;
the needle finds 4640 then 13920 (< 2 s); seahorse valley; cancel; the navigator flies N twice
and lands bit-exactly on the framing, the HUD state times out, a pan clears it, far outside the
set "not found" moves nothing, Esc cancels a threaded search and is handled, a threaded search
lands as a flight. Headless: `scripts/flights/seek-needle.json` (`fly` prints the final view;
key `n` / `seek` in scripts).

## Zoom-out autopilot (as built 2026-10-17; removed the same day, replaced by reverse cruise)

Removed with the driving levels (above): ⇧Z, `Navigator::start_zoom_out`, `NavKey::AutopilotOut`,
`FlightStatus.zoom_out`. Kept here for the soft landing, which the reverse cruise reuses.

Requested by the user: the autopilot in reverse, a scenic flight back to the whole set that can be
stopped anywhere, not a quick reset. Core: `Navigator::start_zoom_out` (`crates/almondbread-core/src/input.rs`),
a fly-to flight ([[sources/van-wijk-nuij-smooth-zooming]], `flyto.rs`) with a soft landing.

| Input | Action |
| --- | --- |
| ⇧Z, menu "Zoom Out to the Whole Set (⇧Z)" | on: eased start towards the home view fitted to the window |
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
| at a stop: J (iOS: "Next" under the HUD, or the cockpit's sparkles) | next as planned |
| at a stop: Z (iOS: "Deeper here") | deeper here |
| at a stop: X (iOS: "Elsewhere") | somewhere else |
| at a stop: Esc | the journey ends, the view stays |
| at a stop: pan, zoom, wheel, palette, overlays, help, full screen, click | as without a journey; the journey stays at its stop (not paused) |

Z and X keep their usual meaning when not at a stop (Z the autopilot; X is unbound and not
handled). Bindings: Swift decodes keys in `FractalView.navKey` (macOS and iOS tables) into
`NavKey::{Journey, JourneyAlt, Autopilot, Elsewhere}` (`AutopilotOut` went with ⇧Z); all meaning is in the
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

**iOS.** Since 2026-10-18 three labelled buttons (Next, Deeper here, Elsewhere) appear under the
HUD at a stop (see "Touch", "Round buttons"); before, the journey button turned into ▶ and two
buttons (deeper, elsewhere) appeared next to it (the long-press zoom out went with ⇧Z, 2026-10-17). Not tried on a device or simulator run
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
- **Phase 1, the set grows (to zoom ≈ 3e27 for 30 cm)**: "The whole Mandelbrot set would be 4 km across"
  below 10 km, then the nearest thing in log: "… as big as Earth", "… 3× as big as the Sun";
  at the turn (set/U from 0.8 to 2) "The whole set would be as big as the observable universe".
  Line 2: "your window: 30 cm" (or "your window: 45 cm").
- **Phase 2, an imaginary universe (to ≈ 1e89)**: set/U = U/thing. Reworded 2026-10-17 (user):
  the subject is "The whole Mandelbrot set" ("set" alone is jargon for newcomers) and the
  sentence reads "… would be as big as an imaginary universe", "in which an atom is as big as
  ours" (before: "In the whole set, our universe would be as small as an atom"). Compared with
  familiar things only (Milky Way, solar system, Sun, Earth, Moon, Everest, you, ant, hair, cell,
  virus, atom, proton, Planck length). Galaxy ≈ 1e33, solar system ≈ 1e41, Earth ≈ 1e47, you ≈
  1e54, cell ≈ 1e59, atom ≈ 1e64, proton ≈ 1e69, Planck ≈ 1e89. Between set/U = 2 and a factor 4
  above the galaxy rung no familiar thing is near (only distances between galaxies): "The whole
  set would be bigger than the observable universe". Line 2: "your window: 30 cm".
- **Phase 3, imaginary universes nest (from the Planck rung; reworded 2026-10-17)**: at each
  level the Planck length is as big as the universe below, so one level is
  `P = log10(U / Planck) ≈ 61.7` decades of zoom. The readout counts levels: "…, 16 levels
  deep" (at ≈ 1e1000) and, at the bottom, "1/72,000 of a proton is as big as ours". Replaced
  the atom-ratio nesting (`a = log10(U / atom) ≈ 36.92`, "A universe in a universe … (27
  times)"; the 2026-10-17 off-by-one fix of that count is moot) and `level_decades`, both gone
  from the code. Exact level offsets: see `crates/almondbread-core/src/cosmic.rs` (unverified here).
- **Ruler**: the things larger than the window up to "universe"; then "1,000 × universe",
  phase 2's rungs "Earth = universe" … "Planck = universe" (reworded: the thing is as big as the
  imaginary universe), "googol ×"; then "3 levels" … "52 levels" (phase 3; before "3× nested"
  … "90× nested"). Linear in depth up to one decade past the Planck rung, then compressed
  (slope 1 → 1/8; marks at least 25 points apart, unit test; was: "a level ~69 points", 36.92-based
  and so outdated). A label whose dot is near the marker is nudged off the marker (within 30 %
  of its width) instead of hidden; a dot right under the marker stays unlabelled.
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
- **Controls (2026-10-18, user):** only the tour "π in the Mandelbrot set" (`tours/pi.md`)
  switches the meter on and runs the hunts, through the step keys `pi-meter` and `pi-hunt`
  ("Guided tours"); the tour's end switches it off. Removed: the P key (macOS and iPad keyboards),
  View ▸ "π Meter" (⌥⌘P), "Hunt π at the Neck" / "at the Cusp", the control center's Overlays
  switch with its reading and hunt buttons, the FFI `Explorer.pi_flight`, and its persistence
  (old saved settings with the key decode fine; the key is ignored). Earlier (2026-10-15 to
  -18): P toggled, the View menu had the switch and both hunts, the control center the switch,
  reading and hunt buttons (the iOS way in), persisted, off after Reset all and
  the Overlays ↺. iPhone portrait moves the panel left of the button column (FFI inset); during
  a tour on a wide canvas 352 points more, left of the tour card (`piMeterTourInset`).
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
   colours = escape speed, the "not decided yet" fog (first a dark grey veil, [[techniques/unresolved-fog]]) and what to do about it,
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

## Guided tours (as built, 2026-10-17)

Requested by the user: guided tours, with an engine where a new tour is easy to add, and a first
tour on the Feigenbaum point. Core: `crates/almondbread-core/src/tour.rs` (format, validation,
`TourPlayer`); render: `Host::start_tour` / `tour_go` / `end_tour` (`host.rs`) and
`crates/almondbread-render/src/tour.rs` (probe point and marker on the overlay); FFI:
`crates/almondbread-ffi/src/tour.rs` (`tours()`, `Explorer.start_tour`, `tour_go`, `tour_step`,
`end_tour`); Swift: `Tours.swift` (card, control center section, View menu) and the tour calls in
`ExplorerModel`.

| Input | Action |
| --- | --- |
| View ▸ Tours ▸ (a tour); control center "Tours" ▸ Start; a place's "Tour" button (places a tour names) | starts it at step 1 |
| → or "Next ›" (last step: "Finish") | next step |
| ← or "‹ Back" | previous step |
| Esc or "End tour" | ends the tour |
| anything else (pan, zoom, places, autopilot, journey, overlays) | as without a tour; the card stays |

**Tours are data.** One Markdown file per tour in `crates/almondbread-core/tours/<id>.md`, listed in
`tour::BUNDLED` (one line per file; a unit test fails when a file in `tours/` is not listed, does
not parse, or its id is not its file name). The core reads them (`include_str!`), so every UI gets
the same tours. A header between `---` lines, then one `## Title` per step. The lines right under
a step's heading, up to the first blank line, are its settings (`key: value`); everything after
that blank line, up to the next `## `, is the step's text (Markdown: paragraphs, `**bold**`,
`*italic*`, `- ` bullets; keep it to two or three short paragraphs, the card is 340 points wide).

```text
---
id: feigenbaum
title: The Feigenbaum point
summary: Period doubling into chaos along the spike.
place: feigenbaum-point
---

## Two points
center: -0.6, 0
span: 2.6
orbit: on
probe: -1.1, 0

The orbit now swings between **two points** ...
```

Header keys: `id` (kebab-case, the file name), `title`, `summary` (one line for the list), `place`
(optional: that place's detail gets a "Tour" button).

Step keys (all optional; unknown keys, repeated keys and bad values are errors with their line):

| Key | Value | Effect |
| --- | --- | --- |
| `center` | `re, im` (decimals, any precision) | the view's centre; needs `span` or `zoom` |
| `span` | number | extent across the window's shorter side, like a place's span |
| `zoom` | number (`1e5`, any depth) | magnification of the whole set: span = 3 / zoom |
| `angle` | degrees | the view's turn (with a view) |
| `place` | place id | the place's view and look (no `center`, `span`, `zoom` with it) |
| `move` | `fly` (default) or `jump` | how the step's view is reached (with a view) |
| `look` | look id | the look (palette, lighting, outline) |
| `palette` | palette id (built-in or library) | the palette with its mapping, after the look |
| `interior` | `flat`, `bubbles`, `period`, `angle`, `distance` | interior colouring |
| `iterations` | `auto` or a whole number | the iteration limit (a deliberately low one too) |
| `orbit`, `map`, `cosmic`, `pi-meter`, `compass` | `on` / `off` | orbit overlay, mini-map, cosmic size scale, π meter, always-on compass |
| `orbit-length` | 1 to 1000 | iterates drawn |
| `map-levels` | 1 to 4 | mini-map levels |
| `probe` | `re, im` or `pointer` | the orbit overlay's point c: fixed, or back to the pointer |
| `probe-path` | `re, im -> re, im -> ...` | c moves along the points (f64: shallow paths), each segment in equal time |
| `probe-seconds` | number (default 10) | one pass along the path |
| `probe-motion` | `bounce` (default), `once`, `loop` | back and forth (eased at the turns), once and rest, or start over |
| `marker` | `re, im` or `off` | a gold double ring at a point |
| `marker-label` | text | its label (with `marker`) |
| `pi-hunt` | `neck` or `cusp` | the π meter's guided hunt (the view eases to the valley, the probe slides down the line to ε = 1e-8 / 1e-16 while the meter counts); switches the meter on; its own step only (not carried over, also started again going back to it); no view keys with it |

**Carry-over.** A setting stays until a later step changes it (`Tour::effective`): step n shows
the saved user settings with steps 1..n applied in order, so going back shows exactly what the step
showed before (a look after a palette replaces it, a palette after a look keeps the look's
lighting). A step without a view leaves the view where it is going forward, and returns to the
last view set before it going back. `move` belongs to its own step.

**Playback** (`TourPlayer`). Start saves the image settings, the overlays (orbit, map, cosmic, π
meter, compass) and the view, and stops other motion like a jump (autopilot, cruise, flight,
journey, the π hunt). Each step's settings replace the renderer's (the learned iteration floor
stays) and its view is flown to (the fly-to path) or jumped to; Rust returns the step with the
resulting settings, which the Swift model mirrors without pushing back (as after `go_to_place`).
While a tour runs, the settings persisted to user defaults are the saved ones, so quitting
mid-tour keeps the user's look. The end restores the saved settings and flies back to the saved
view; changes made during the tour are dropped with them. Manual exploring during a step is
allowed; Next continues from the next step's definition.

**Probe point.** The orbit overlay's c normally follows the pointer (`Navigator::zoom_target`).
A tour's `probe` replaces it: `Overlay::tour.probe`, filled by the host every refresh from
`TourPlayer::probe_at(now)`; the existing ring and dots show it, with the whole-set map's dot
cloud and the plain-text fate ("settles into a period-4 cycle"). A path is a closed form of the
time since the step began (`ProbePath::at`: `u = (1 − cos πt/T) / 2` for bounce, so it turns
smoothly; the polyline's segments take equal time, so points placed closer make c move slower
there), the same at any refresh rate (unit test at 30, 60, 120 Hz). While a path moves the host
keeps frames coming; each new c is a new probe (about 1.5 ms of analysis per refresh, as with
the pointer), so the fate line mostly reads "following the orbit" while c moves and settles
when it rests. Without `probe` (or `probe: pointer`) the pointer drives it again; the tour's end
clears both probe and marker.

**Tests.** `tour::tests`: every bundled tour parses, every step resolves to valid settings and a
view, every listed file exists (and the other way round); carry-over; errors with their lines
(unknown key, view without span, bad point, span and zoom, unknown look, palette, place, bad
flag, counts, probe keys without a path, marker label without marker, `move` without a view,
repeated key, missing text, text right under the heading); the probe path's ends, turns and
equal-time segments, the same samples at 30/60/120 Hz, once and loop; steps apply their
settings (look, iterations, overlays, untouched switches kept), back returns to the step's view
and settings, and the end restores the saved settings and view exactly. FFI `tours()` lists the
Feigenbaum tour. `--ui-demo=tour-<id>` captures every step of a tour in a background window
(`window-tour-<id>-<n>.png`, `--ui-demo-tour-wait=n:s` for moving probes) and the restored view
at the end.

### The Feigenbaum tour

`tours/feigenbaum.md`, 9 steps, on the real axis with the period interior colouring (one colour
per period) and the orbit overlay. Every number was computed (Newton on `f^p(0) = 0` for nuclei,
on `f^p(z) = z, (f^p)'(z) = −1` for the period-doubling points, attractors after 4·10⁵
iterations; f64, 2026-10-17), not taken from memory:

| Step | c (probe) | What the orbit does |
| --- | --- | --- |
| Square and add | −0.2 | fixed point −0.1708 (hops around it) |
| Two points | −1.1 | 2-cycle −1.0916, 0.0916 |
| Watch it split | path −0.7 → −1.45, 24 s per pass, bounce | 1, 2, 4, 8, … points, then chaos |
| Period 4 | −1.3107026413 (nucleus) | −1.3107, 0.4072, −1.1449, 0 |
| Ever smaller | −1.3815474844 (period-8 nucleus) | 8 points |
| The Feigenbaum point | −1.4011551890920506, marked | never settles, never escapes |
| Chaos | path −1.4012 → −1.46, 16 s, bounce | a band of dots; windows of period 12 (−1.417), 10 (−1.447), 14 (−1.458) flash by |
| Order returns | −1.7548776662 (period-3 nucleus) | 3-cycle −1.7549, 0, 1.3248 |
| Your turn | the pointer | — |

Period-doubling points on the axis: −0.75, −1.25, −1.3680989394, −1.3940461566, −1.3996312389;
the bulbs between them are 0.5, 0.1181, 0.02595, 0.005585 long, ratios 4.234, 4.552, 4.646 →
δ = 4.669… (src: [[sources/muency-atom-domain]] for the Feigenbaum series of sizes;
the numbers are our computation). Nuclei: period 16 at −1.3969453597, 32 at −1.4002530812.

Checked in the app (2026-10-17, M1 Pro, release, background window, `--ui-demo=tour-feigenbaum
--ui-demo-tour-wait=3:14,7:12`): the overlay's fate line at each step's capture read "settles on
a fixed point", "period-2", "period-8" (step 3, c ≈ −1.378 at 14 s), "period-4", "period-8", "still
undecided after 100 000 steps" (the Feigenbaum point), "following the orbit" (step 7, c ≈ −1.451,
chaotic), "period-3", as the texts say; the end flew back to the start view with the user's look.

### The minibrot-periods tour

`tours/minibrot-periods.md` ("What a minibrot's period means", place `airplane`), 9 steps, user
request 2026-10-18 ("what the periods of the minibrots are, like 'period-2600 minibrot'"): loop
length = period (cardioid 1 at c = −0.2, disc 2 at its nucleus −1), the 1/3 bulb (nucleus
−0.12256 + 0.74486i, marker on its three-branch fork M at −0.10110 + 0.95629i, checked in f64:
`z_4 = z_5`), the Airplane (period 3), then the real chain of `seek` (key N) from the Airplane:
periods 3 → 7 → 16 → 48 → 80 → 128 → 384 → 640 → 896 → 2688 (nuclei by `seek::seek` with a 30 s
budget, a throwaway example, 2026-10-18; the period-7 nucleus −1.674066091474788, period 128 at
−1.668339878897302863730 + 2.31562583820807543e-9 i, period 2688 framed at span 6.3e-16). Probes
only up to period 128 (span 4.8e-11, f64-safe); the 2688 step turns the orbit off and sets
`iterations: 300000` (the auto limit settled at 81 k with the heart mostly fog: `progress`, 7.3 %
unresolved; at 300 k it settles in 11 s at 1600×1000 on the M1 Pro, 4.2 % unresolved), and the last
step sets `iterations: auto` again. Headless checks (`render --inside period --orbit`): orbit fates
fixed point, 2, 3, 3, 7, 128; interior periods of the 2688 view at 500 k: 2688 (heart), 5376 (disc),
8064, …

"Why deeper means longer" (our explanation, inference): a minibrot's size goes like `1/(β λ²)`
with `λ = ∏ 2 z_n` over the loop ([[sources/muency-newton-raphson-zooming]] size estimate), and
`|2z| ≤ 4` inside the escape radius, so a copy `10^-d` small needs `p ≳ d / (2 log₁₀ 4) ≈ 0.83 d`.
The needle places sit right on that bound (period 79 at 1e-91, 255 at 1e-302, 834 at 4e-1001:
0.83–0.87 per digit); the Seahorse minibrot (8007 at 1e-31) and this chain (2688 at 10^15.7) are far
above it.

Checked in the app (2026-10-18, M1 Pro, release, background window, `--ui-demo=tour-minibrot-periods
--ui-demo-tour-wait=7:25,8:10,9:15`): fates "settles on a fixed point", period-2, 3, 3, 7, 128 at
steps 1–6; the gold ring of step 3 sits on the antenna's fork; step 7 shows the period-2688 heart and
its period-5376 disc while refining; the end restored the view and look.

### The π tour

`tours/pi.md` ("π in the Mandelbrot set", place `pi-at-the-neck`), 6 steps, user request
2026-10-18 (the meter's only way in from now on, see "π meter"): the neck marked at −0.75; the
place view with the meter counting the centre (c = −0.75 + 0.02i, N = 158, N·ε = 3.16); `pi-hunt:
neck` (to ε = 1e-8, N = 314 159 266); "Why π?" (the panel's turn circle: one turn from start to
escape at 2ε per step, so N·2ε ≈ 2π, [[techniques/pi-in-the-mandelbrot-set]] "Why π"); `pi-hunt:
cusp` (to ε = 1e-16, N = 314 159 263, one digit per two decades); back to the neck place with the
meter on. Counts in the text are the measured ones on the technique page (315, 31 417, 3 141 593 at
the neck).

New step key `pi-hunt` (core `Step::pi_hunt`, `StepPlan::pi_hunt`; host `apply_tour_plan` stops
any running hunt and starts `Overlay::start_pi_flight`; `end_tour` stops it). Tests: `tour::tests`
`a_pi_hunt_switches_the_meter_on_and_belongs_to_its_own_step` and three new error cases. The
interaction-test scenario "pi hunt" and the `--ui-demo=pi-flight…/pi-cusp…` scenes now start the
tour at its hunt step.

Checked in the app (2026-10-18, M1 Pro, release, background window, `--ui-demo=tour-pi
--ui-demo-tour-wait=3:30,4:5,5:35`): step 2 N = 158, N·ε = 3.1600 (2 digits); step 3 N = 314 159
266, 8 digits in 7.4 s; step 5 N = 314 159 263, N·√ε = 3.14159263, 8 digits; panels left of the
card (at the cusp they cover the probe's ruler, which sits left of the elephant valley: accepted).
`make interaction-test` "pi hunt": counted at the neck, meter off after the tour, 106 fps.

### The limits tour

`tours/limits.md` ("Why some points are hard", place `seahorse-minibrot-1e31`), 9 steps, user
request 2026-10-18: a slow point 0.001 above the neck (orbit overlay: escapes after 3 147 steps;
`N ≈ π/ε`, [[techniques/pi-in-the-mandelbrot-set]]), one 1e-5 above it (*still undecided after 100 000
steps*; it escapes after about 314 000), −0.74 just inside the cardioid (proven after about 1 900
steps), fog in the neck gap at −0.74999996120361811 + 0.00036918279150454967i, span 1.5e-8
([[synthesis/neck-escape-statistics]]: its discs escape after 2e7–7e7), computing further, then
f64's 16 digits at the 1e31 seahorse minibrot, perturbation ([[techniques/perturbation]],
[[techniques/rebasing]]) and skipping ([[techniques/linear-approximation-imagina]]) at the 1e302 needle
minibrot (one sample per pixel: 1.09e9 iterations in 1.97e7 steps, 55×; headless `render`,
2026-10-19, M1 Pro).

**Pitfall: a low `iterations:` shows no fog in the app.** A first version set `iterations: 100`
(then 6) on seahorse valley to show fog; the captures had none. The scheduler extends unresolved
pixels past the limit (×16, `extension_factor`) and then "computes further" up to the cap
(`progress --iterations 6` at seahorse valley: 16 % open at 96 steps after 0.07 s, 805 pixels at
98 304 after 0.5 s). Fog only stays where points need more than the upper limit, so the tour shows
the neck gap instead (54 % open, "computing further · to 200 k steps", in the capture). Same for
the colours tour.

**Pitfall: tour views deeper than about 1e-305.** `every_bundled_tour_parses_and_every_file_is_listed`
checks `viewport.scale.to_f64() > 0`, which underflows below f64's range; a step at `swirl-1e3000`
failed it. The tour ends at `needle-minibrot-1e302` instead (no change to tour.rs, which another
agent was editing).

Checked in the app (2026-10-19, M1 Pro, release, background window, `--ui-demo=tour-limits`): fates
3 147 steps, still undecided after 100 000, settles on a fixed point; the fog view; the 1e31 and
1e302 minibrots; long flights between deep places replaced by `move: jump`.

### The bulbs tour

`tours/bulbs.md` ("Bulbs and spokes", place `third-bulb`), 8 steps, user request 2026-10-18:
period colouring and the orbit overlay; a probe path through the nuclei round the top of the
cardioid (1/2, 3/7, 2/5, 3/8, 1/3, 2/7, 1/4, 1/5, 1/6: periods 2, 7, 5, 8, 3, 7, 4, 5, 6; 48 s per
pass); the antenna's first fork has as many branches as the period (rings on M(5,1) of the 1/4 bulb
and M(6,1) of the 1/5 bulb); the p/q rule; Farey: the biggest bulb between p/q and r/s is
(p + r)/(q + s), period q + s (2/5 between 1/2 and 1/3, 3/8 between 2/5 and 1/3).

Coordinates (`almondbread locate`, 2026-10-19): nuclei 1/2 −1; 3/7 −0.62243629504129359 +
0.42487843647562918i; 2/5 −0.504340175446244 + 0.56276576145298196i; 3/8 −0.35910239011244916 +
0.61735345339882686i; 1/3 −0.12256116687665362 + 0.74486176661974424i; 2/7 0.12119278610590649 +
0.61061169221075421i; 1/4 0.28227139076691388 + 0.5300606175785253i; 1/5 0.37951358801592375 +
0.33493230559749759i; 1/6 0.38900684056977124 + 0.21585065087081911i (each the period q, from guesses
on the cardioid at angle 2πp/q pushed out by `sin(πp/q)/q²`). Forks: `locate --misiurewicz 5,1` near
0.365 + 0.591i gives 0.36636298342276434 + 0.59153377326144523i (1/4), `6,1` near 0.4364 + 0.3417i gives
0.43792424135946278 + 0.34189208433811611i (1/5); the 1/3 bulb's is M(4,1). The fork of the p/q bulb
is M(q+1, 1) in `locate`'s numbering (z_{q+1} = z_{q+2}) for q = 3, 4, 5 (our observation).

Checked in the app (2026-10-19, `--ui-demo=tour-bulbs`): fates period-4, 5, 3, 5, 8 at the fixed
probes; the rings sit on the forks and on the 2/5 and 3/8 bulbs.

### The valleys tour

`tours/valleys.md` ("Seahorse and Elephant valleys", place `seahorse-valley`), 10 steps, user
request 2026-10-18, built from catalogue places ([[synthesis/places]]): seahorse valley, a
seahorse, its tail (a Misiurewicz point at the centre), the two-arm spiral M(24,2) marked at 1e-4
and again at `spiral-1e50` (self-similar: zoom by the cycle's multiplier and turn by its angle),
elephant valley and the herd, the three-way junction M(4,1) and the same at `junction-1e100`.

No orbit probes at Misiurewicz points: a probe at M(4,1) (`render --place three-way-junction
--orbit 0,0`) says "escapes after 137 steps" in f64, because the orbit lands on a repelling fixed
point (|2z| = 1.33) and rounding grows from there; the tour says so in words instead. `move: jump`
on the steps after a deep place (a fly-to up from 1e50 or 1e100 takes long).

Checked in the app (2026-10-19, `--ui-demo=tour-valleys`): the ring on the M(24,2) spiral's centre;
the 1e50 view the same two-armed spiral; the 1e100 fork with three branches (covered in stars, so
the text says that, not "spirals").

### The doubling tour

`tours/doubling.md` ("Patterns that double", place `three-way-junction`), 9 steps, user request
2026-10-18: from the M(4,1) fork (3 arms) towards the period-39 minibrot next to it (nucleus
−0.101095952005097279 + 0.956281332988737429i, `locate --center <M(4,1)> --radius 1e-6`, size
10^−10.41), views centred on the nucleus at spans 2e−5 (the fork at the top, a ring on the
nucleus), 1.5e−6 (6 arms), 1.4e−8 (12), 8e−10 (24 and a lace ring), 1.2e−10 (the minibrot); then why
(near a minibrot one round of its loop acts like a fresh z² + c, and squaring doubles angles; our
simplification of the embedded-Julia mechanism, [[sources/mathr-julia-morphing-posts]]), the
`embedded-julia` place and Julia morphing in deep-zoom videos ([[sources/deep-zoom-videos]]).
Depth fractions of the doublings: [[questions/julia-morph-depth-fractions]].

Checked in the app (2026-10-19, `--ui-demo=tour-doubling`): 6, 12 and 24 arms at steps 3–5, the
minibrot at step 6.

### The colours tour

`tours/colours.md` ("What the colours mean", place `whole-set`), 12 steps, user request
2026-10-18: escape counts with the orbit overlay (c = 0.6 escapes after 8 steps; a path from 0.6
towards the cusp at 0.2501, 283 steps at its end), smooth colouring
([[techniques/smooth-coloring]]), a palette (`grey`) and a look (`ember`) on seahorse valley, fog in
the neck gap (the same view as the limits tour), proven black, then the five interior colourings
on the cardioid and its bulbs ([[techniques/coloring-methods]], [[techniques/interior-detection]]).

**Pitfall: a step that only changes `interior:` may not show in the app until the view moves.**
In the first capture the angle and distance steps still showed the period colouring after 20 s
(the scheduler's history kept the old colouring; the recolour on an interior switch did not
reach the shown frame without a new job). Each interior step now repeats its view with
`move: jump`, which starts a new job; then bubbles, period, angle and distance all showed
(`--ui-demo=tour-colours`, 2026-10-19). Not chased further in the renderer: a candidate bug
for the colour-change path (`plan_frame` / `same_colors`), worth a look.

### The Julia tour

`tours/julia.md` ("Every point has a Julia set", place `cantor-island`), 8 steps, user request
2026-10-19. The app draws no Julia sets, so the tour uses what exists: the orbit overlay at c = −1
(basilica, period 2), the 1/3 nucleus (rabbit, period 3), c = 0.5 + 0.5i (escapes after 9 steps: dust),
then catalogue places whose shapes echo Julia sets: `cantor-island` (a dust-like embedded Julia
set), `two-arm-spiral` (M(24,2); Tan Lei 1990: the set near a Misiurewicz point looks like that c's
Julia set, unverified, no source page yet), `three-way-junction`, `embedded-julia`. A Julia view
(render the filled Julia set of the probe's c next to the picture) would make this tour much
better; not built (user: no Julia renderer for now).

Checked in the app (2026-10-19, `--ui-demo=tour-julia`): fates period-2, period-3, escapes after 9
steps; the places as described. One capture showed the previous step's fate line (taken before
the overlay refreshed); the logged fate was right.

## Touch (as built, 2026-10-18)

The Mac is the reference; the iPhone and iPad app should reach the same features with a finger,
sharing the code. Everything a gesture *means* is the Rust `Navigator`'s (new `InputEvent`s,
unit-tested); Swift only decodes gestures (`FractalUIView`) and draws the SwiftUI controls
(`Sources/AlmondbreadKit/TouchControls.swift`). Checked on the iPhone 17e and iPad mini
simulators only; feel, haptics and thumb reach need a real device.

| Input (touch) | Action | Core |
| --- | --- | --- |
| Tap a cockpit icon / the lane icon | manual (Esc), cruise in (E), autopilot (Z), journey (J), lane assist (L), as a click on the Mac | `cockpit_hit` |
| Throttle lever at the trailing edge: drag the knob (a *position lever* since 2026-10-18, "Position lever" below; was a jog lever, one gear per 14 points) | the knob's tick is the gear: up in, down out, the middle M stops; it stays where it is left; in A and J their speed; during a fly-to its speed | `SetGear { gear }` |
| Throttle: tap a tick / a chevron at either end | that tick's gear / one gear up or down | `SetGear { gear }` |
| Throttle: double tap | manual (Esc: the cruise brakes to rest, A and J stop) | `KeyDown(Stop)` |
| One-finger drag in C | steers like the Mac pointer, while the finger is down (no 3 s idle timeout). **Superseded (device round): a drag pans, the cruise zooms on** | `TouchSteer`, `TouchSteerEnd` |
| One-finger drag in A | the direction wish (target marker and wish line); it **stays after the finger lifts**, a lift inside the dead zone clears it, Z off clears it; never hands over. **Device round: let go once settled, the line goes** | same |
| Long press (0.45 s), released in place, nothing flying (round 2: in A, C and J a held finger steers) | a popover menu at the finger: Seek Minibrot Here, Orbit Here, Fly Here, Copy Coordinates, Go to Copied (when the clipboard has text) | `SeekAt`, `Probe`, `FlyHere`, `coordinates_text_at` |
| Long press, then drag (orbit on) | the orbit probe follows the finger and stays (touch has no hover) | `Probe(Some)` |
| Tap a (?) badge | its help popover; a fingertip within 18 points counts (`help::hit_touch`) | `help_hit` |
| Tour card ‹ Back / Next › / End tour | as ← / → / Esc; regular-size buttons on iOS | — |
| HUD "Continue" | regular-size bordered button on iOS; G on an iPad keyboard | — |

**Reasons.**

- *Throttle as a jog lever, not a slider* (superseded 2026-10-18 by the position lever, below:
  a user on a real iPad read it as "gear up / gear down" and did not see that the drag length
  chose the gear). 26 forward and 26 reverse gears on a 200-point track
  would be 4 points per gear: no detent can be felt and a selection tick every 4 points is a
  buzz. So the knob follows the finger and springs back when it lifts, and every 14 points of
  travel is one detent (one E or Q). The gear set shows as a bar from the zero notch
  (square-root scale, so the low gears get room) and on the knob ("3", "R2", "M"). The finger
  can travel past the track (the whole screen height), so a long drag reaches high gears in
  one go. The zero stop within a drag is the keys' detent (a held Q never crosses zero).
- *A steering finger is not a pointer.* Sent as `Pointer`, a finger became the zoom target and
  the orbit probe, timed out after 3 s in C, and in A the wish vanished 0.3 s after the lift, so
  it only held while a finger covered the picture. `TouchSteer` keeps it out of the pointer
  state; in A the wish is kept like a mouse left on that side.
- *A popover, not the system edit menu,* for the long press: `UIEditMenuInteraction` is a
  horizontal strip that showed two items and a page arrow on the iPhone (seen in the
  screenshot); the SwiftUI popover lists all five with 44-point rows, like the help popovers.
- *Cockpit for touch* (`CockpitOverlay::touch`, FFI `set_cockpit_touch`): every control a
  44 × 44 point cell, icons drawn 1.35× larger; one row (438 points) where it fits (iPad, phone
  on its side), else two rows (253 × 66 points: the icons and lane icon on top, name, gauge and
  rate below). The desktop layout is unchanged. On a phone in portrait a bottom-left map row
  leaves no room beside it, so the touch cockpit sits above the maps (`lift`). A double tap on
  the cockpit or a badge does not zoom, and taps there do not wait for a double tap
  (`shouldReceive` keeps the double-tap and long-press recognisers off them).
- *Layout.* Phone portrait: the throttle on top of the button column at the bottom trailing
  corner (thumb reach). Phone on its side: the buttons in a row at the bottom trailing corner
  (the cockpit keeps left of the five), the throttle at the trailing edge above them. iPad:
  buttons top trailing as before, the throttle at mid height on the trailing edge (where the
  thumbs are when holding it); the π meter and the tour card keep left of it. Shown with the
  HUD, like the cockpit.
- *Haptics* (`Haptics`): a selection tick per gear, a rigid tick at the zero detent and for the
  double-tap stop, a light tap when a flight lands (journey stop, fly-to, minibrot), a success
  when a minibrot search found one. Nothing during continuous motion.
- *iPad menu bar.* The Mac's menu commands (`FractalCommands`) are on iPadOS too: ⇧⌘C / ⇧⌘V,
  ⌘G / ⇧⌘G, ⌘= / ⌘−, ⌘0, ⌘I and the View items (round 2 added the Tours submenu and the Help items).

**Parity with the Mac (2026-10-18).**

| Mac | iPhone / iPad touch | iPad keyboard / pointer |
| --- | --- | --- |
| W A S D, arrows, Space, + / − | drag, pinch, double tap, two-finger tap | same keys |
| E / Q gears, hold for more | throttle lever | E / Q |
| Z, J, Esc | cockpit icons; throttle double tap | Z, J, Esc |
| N seek minibrot | long-press menu (here); control center "Seek minibrot" | N |
| L lane assist | cockpit lane icon, control center | L |
| G continue | HUD button | G |
| C, O, M, P overlays; toolbar Orbit, Mini-map, Size Scale, Fly To | round buttons (C, O, M, Fly to); control center; "Orbit Here" | C, O, M, P |
| R / ⌘0 reset | control center Reset | R, ⌘0 |
| ⌘I, toolbar Control Center | round button (lit while open); ✕ in the pane; phone: swipe the sheet down | ⌘I |
| ⇧⌘C / ⇧⌘V coordinates | long-press Copy / Go to Copied; control center | ⇧⌘C / ⇧⌘V (menu bar) |
| ⌘G / ⇧⌘G fly to | control center Fly to… / Copied; long-press Fly Here | ⌘G / ⇧⌘G |
| Tour ← / → / Esc | card buttons | ← / → / Esc |
| Journey stop J / Z / X | Next / Deeper here / Elsewhere under the HUD | J / Z / X |
| Cockpit click and tooltips | taps; the (?) explains (no hover, no tooltips) | pointer clicks |
| (?) badges | tap (18-point fingertip) | — |
| Orbit probe under the pointer | the last "Orbit Here" point (long-press drag moves it); the view centre otherwise | iPad pointer hover |
| Autopilot wish from the pointer, target marker | a resting finger (a swipe pans, "Rest or swipe"), let go after the lift; marker as on the Mac | pointer hover |
| Mini-map click / ⌥-click | tap jumps; long press flies (round 2) | — |
| Place zoom pill | shown (no hover needed) | — |
| Two-finger scroll setting (pan / zoom) | not applicable (pinch zooms) | trackpad scroll pans |
| Rotation `,` `.` `/`, trackpad rotate | two-finger rotate; compass tap / drag | `,` `.` `/` |
| F full screen | always full screen | — |
| Gamepad | same | — |

**Gaps** (not done): no hover tooltips (the (?) badges and the help say it). The mini-map's fly
and the iPad menu bar's Tours and Help came in round 2 (below).

### Round 2: a first run by a child (2026-10-18 evening)

Goal: the app handed to a child who has never seen it, on an iPad. Branch `ios-touch-2`;
simulators only (`AB2 iPhone 17e`, `AB2 iPad mini`).

| Input (touch) | Action | Core |
| --- | --- | --- |
| A finger at rest (0.2 s) or moving, in A, C or a journey's flight | steers (A: the wish; C: like the pointer; J: takes the wheel once off the dead zone, then the cruise steers). **Device round: A only; in C and J a drag pans. Rest or swipe: a resting finger steers in A, C and J again, a swipe pans** | `finger_steers`, `touch_hold` → `Steer`, `TouchSteer` (replaced by `FingerDown` / `FingerMove` / `FingerUp`) |
| Long press (0.45 s) with nothing flying, or during a fly-to | the long-press menu (as before) | `touch_hold` → `Menu` |
| Long press on a mini-map | flies there (the Mac's ⌥-click), a light haptic | `touch_hold` → `FlyToMap`, `map_click(fly)` |
| Long press on a cockpit icon or (?) | nothing (their taps act) | `touch_hold` → `Nothing` |
| First start | the touch hints (below); any touch or key dismisses them | — |

**Long press vs steering (decision).** In A and C a finger that rested before dragging became
a 0.45 s long press: the drag then only moved the orbit probe and the release opened the menu,
so the plane did not steer — the first thing a child does with an autopilot. Rule now: *where
a finger steers, a finger never opens the menu.* Rust decides (`Navigator::finger_steers`: A
on, cruise engaged, or a journey leg flying; `touch_hold(spot)` adds where the finger rests),
Swift only times the press and asks. Reasons:

- *The menu's actions name a point under a still picture.* While the view flies, the point
  under the finger moves by the time the menu is read (at ×5/s it is 5× deeper a second later);
  "Fly Here" or "Seek Minibrot Here" would act on a stale point. With nothing flying there is
  nothing to confuse. A fly-to the user started (Fly Here, Fly to…) counts as not steering: a
  drag there takes over and pans, the press is still a menu.
- *No second gesture for the menu while flying.* A two-finger long press or a double-tap-hold
  was considered: undiscoverable, and two fingers are pinch/rotate. The menu is one tap away
  (the throttle's double tap or the cockpit's M stops, then hold), and the autopilot button's
  menu keeps seek, lane assist and the pedals.
- *Resting steers after 0.2 s, not 0.45 s* (the recogniser's minimum duration is set per touch
  from the last frame's `FRAME_FINGER_STEERS` bit): a finger put down in A sets the wish almost
  at once; a tap (pause / resume) is shorter, and its tap after a hold is swallowed as before.
- *J:* a journey's flight now steers like C (a drag used to pan and suspend it, the next drag
  steered — two meanings in a row). A touch is deliberate, so it takes the wheel as soon as it
  leaves the dead zone (the Mac's pointer needs 60 points and 0.6 s); inside the dead zone the
  leg flies on. At a stop, and between legs, a drag pans (looking around), as before.
- The frame bit `FRAME_AUTOPILOT` (A or C) became `FRAME_FINGER_STEERS` (A, C or a journey's
  flight).

**First-run touch hints** (`TouchHints.swift`). One layer, the first time the app starts on
iPhone or iPad: "Drag to move", "Pinch to zoom", "Hold for more" in the middle, "Slide to fly in
or out" pointing at the throttle, "Flies by itself" at the plane (anchor preferences, so
the labels follow the layout: phone portrait column, phone on its side, iPad; since 2026-10-18
above the cockpit's plane icon, whose rect Rust reports through `cockpitControls`), "Touch anywhere
to start", a dimmed picture behind. Material capsules like the HUD. It takes no touches: a
window-level recogniser that fails at once sees the first touch anywhere (or a key) and fades
the layer out, and that touch still does what it does (a double tap zooms). Seen once
(`touchHints.seen` in user defaults); Help sheet ▸ "Show Touch Hints" and the iPad's Help menu
bring it back; `--touch-hints` forces it (tests, screenshots). No text walls: three to five
words per label. Checked on iPhone portrait and landscape and iPad screenshots: the middle
group moves up (phone portrait) or left (wide screens) out of the throttle label's way.

**Tour card on a phone** (narrow canvas). At the bottom it covered the map row, the cockpit
above it and part of the picture. Now it sits at the top in the HUD's place (the HUD hides
during a tour there), its text in `.footnote` scrolls within 150 points, and a chevron folds it
to its title and buttons (‹ Back / Next › keep working folded). The view's centre (where tours
put the orbit), the maps and the cockpit stay free. iPad and Mac unchanged.

**Help popovers on iOS.** The cockpit's (?) popover opened *below* its badge at the bottom of a
phone, off the screen with its More… button: SwiftUI on iOS reads `arrowEdge` as the
popover's own edge (the Mac: the anchor's). iOS now passes nil (UIKit picks the side with
room) and the text scrolls beyond 360 points.

**Fly to… on a phone.** The sheet had a 420-point minimum width (a phone is 375–440): fields
and buttons clipped. Now it fits, with `.medium` / `.large` detents, large buttons and a
system Paste button. It no longer reads the clipboard when it opens: on iOS that asked "Allow
paste?" every time (seen in the screenshot). The keyboard words are gone from its iOS text.

**Tests do not persist settings.** `TestLaunch.ephemeral` (window tests, `--ui-demo`, the iOS UI
tests' `ALMONDBREAD_UITEST=1`): no restore, no save, and the HUD, lane assist, scroll, journey
mode, edge panning, target lock and sidebar state are neither read nor written. On iOS
`--ui-demo` and the UI tests were not window tests, so "Orbit Here" turned the orbit and the
mini-map on for the next normal launch.

**iPad menu bar.** View ▸ Tours (every tour, End Tour) and Help ▸ Almondbread Help (⌘?) and
Show Touch Hints.

**Surfaces checked** (screenshots, iPhone 17e and iPad mini, portrait and landscape): control
center sheet at both detents and in landscape, places grid, fly-to sheet, help sheet, cockpit
help popover, long-press menu, journey stop buttons (four 44-point buttons plus the throttle in
the column; the row on a phone on its side), tour card. Fixed what is above, and the control
center's button rows (Copy / Go to copied / Reset, Autopilot / Fly to… / Copied, Journey /
Daily): in the iPad's sidebar next to a portrait canvas their titles wrapped letter by letter
("Co py", "Au-topi-lot"); `FitRow` shows the icons alone when the titles do not fit (the titles
stay the accessibility labels). The rest fitted. One iPad run's control-center tap did not
open the sidebar (the open item in [[platform/ios-simulator]]); the rerun passed.
The HUD's Continue button was not reached in a test (it needs a view at its time limit); it is
a regular-size bordered button.

**Tests.** Rust `a_held_finger_steers_in_a_c_and_j_and_opens_the_menu_in_m`,
`a_finger_takes_the_wheel_from_a_journey_flight`. XCTest `TouchControlsUITests`:
`testHoldSteersInAutopilotAndCruise` (rest 1 s then drag, and rest only: no menu, A stays A,
C moves; M still gets the menu), `testTouchHints` (shown, gone after a double tap that still
zooms, back from Help), `testTourCardCompact` (top, clear of the cockpit, folds, steps folded),
`testMiniMapLongPressFlies`, `testTestsDoNotPersistSettings` (Orbit Here in a test, a normal
launch has the orbit off), `testSurfaces` (screenshots; fly-to buttons hittable; journey stop
buttons ≥ 44 points and hittable in both orientations).

**Needs the device.** Whether 0.2 s is the right rest-to-steer time for small fingers; whether
the hints get read before the first touch dismisses them (a child may tap at once — Help brings
them back); haptics on the map fly; how the folded tour card feels; Allow-paste prompts remain
on "Go to Copied" (user-initiated, once per tap; since the QA pass the system Paste control
beside it avoids them).

**QA pass (2026-10-18, [[platform/ios-simulator]] "QA").** A two-finger twist turns the view
only past 15° (`TouchRotate`: a pinch always twists a little, and a child's pinches turned the
view); a search started from rest asks for frames (`Navigator::wants_frames`); a tap on a
place's picture goes there on iOS (replaced the same day, next paragraph); the long-press menu
and the control center have the system Paste control; touch wording for "click to resume" and
the tour's keys.

**Places grid on touch (user, 2026-10-18, after the QA pass on a real iPad).** "A tap goes
there" left Fly unreachable (Fly sits below the 79-picture grid, off screen). Now, on iOS only
(`PlaceTouchTile` in `ControlCenterView.swift`; the Mac is unchanged): the first tap on a picture
selects it — a 3-point accent border, the name in a dark strip along the bottom edge (the zoom
pill hides), and **Go** (`goToPlace`) and **Fly** (`flyToPlace`) over the image on a material
background, each at least 44 × 44 points. Go or Fly acts; a second tap on the selected picture
outside the buttons (the name strip included) goes there; another picture moves the selection
(one selected tile). The rows under the grid (name, description, Go / Fly / Tour, Random) stay.
VoiceOver: the tile is labelled with the name and has the actions Go and Fly; the buttons are
"Go to …" / "Fly to …" (identifiers `placeTileGo`, `placeTileFly`). **Sizing:** the smallest
tiles are about 100 × 63 points (iPhone 17e portrait in the sheet, 3 columns, the same width at
the medium and large detents; iPad mini portrait sidebar at about its 290-point minimum, 2
columns). Icon and label side by side need about 105 points for both buttons, so a
`ViewThatFits` uses that only on wider tiles and otherwise puts the label under the icon (two
44 × 44 buttons side by side fit; no icon-only fallback needed). Seen in simulator screenshots on
both; the buttons then cover most of the picture, the free margin and the name strip still take
the second tap. XCTest `FirstUseUITests.testPlaceTileGoAndFly` (select moves nothing, one Go,
Go jumps, another tile moves the selection, Fly changes the view over several samples) and
`testControlCenterPlaceAndTour` (select, then a second tap on the picture's corner goes there).

**Tests** (the throttle part superseded by "Position lever" below). Rust: `throttle_steps_the_gears_with_a_detent_at_zero_per_drag`,
`touch_steering_wishes_in_a_and_steers_in_c`, `a_touch_probe_moves_the_orbit_point_only`,
`fly_here_and_seek_here` (core `input`), `touch_cells_are_44_points_in_one_or_two_rows`
(render `cockpit`). XCTest `TouchControlsUITests` (`make ios-uitest`, iPhone and iPad
simulators): throttle (gear 3 after three detents and zooming in; down past zero stops at 0; a
new drag reverses; double tap to manual; in A it steps the speed), cockpit taps through Rust's
hit test 18 points off an icon's centre, the cockpit's (?) popover, the long-press menu (orbit,
a probe drag opens no menu, copy then Go to Copied offered, Fly Here ×8, seek), the tour card,
landscape layout (nothing overlaps). The app reports the cockpit's cells in its accessibility
value only with `ALMONDBREAD_UITEST=1`.

### Device round: an iPad Pro 11" in the hand (2026-10-18)

The first try on a real iPad Pro 11" (M5) found three things the simulators did not.

| Input (touch) | Action now | Core |
| --- | --- | --- |
| Pinch, then one finger lifts first (either one) | the pinch ends there (a zoom fling only if it was still moving); the finger left down does nothing (no pan, no steering, no long press) until every finger is off | `Pinch` / `TouchRotate` set `pinched`; `TouchesLifted` clears it |
| One-finger drag in C | pans 1:1 while the cruise zooms on in its gear; the release flings | `Pan` (as a drag on the Mac) |
| One-finger drag in a journey's flight | takes the wheel (cruise at the journey's speed) and pans; the next drag pans the cruise | `Pan` → `take_wheel` |
| Finger held still in C or a journey's flight | nothing (the drag after it pans; no menu while flying) | `touch_hold` → `Nothing` |
| Finger in A (resting 0.2 s or moving) | the wish, as before, but let go once it settled after the lift: the wish line fades | `TouchSteer`, `TouchSteerEnd` |

*Superseded the same day by "Rest or swipe" below: in C, J and A a resting finger steers and a
swipe pans.*

**1. "After the zoom there's also some panning."** `UIPinchGestureRecognizer` does not end when
one finger lifts: it goes on with the remaining finger, its `location` jumps from the midpoint to
that finger and then follows it. `handlePinch` sent that as the pinch centre's movement (`dx, dy`
of `Pinch`, "two-finger drag pans while pinching"), so the view jumped by half the finger spacing
and then panned with the finger; the pinch's `GestureEnd` (when the last finger lifted) could
fling from those samples. The one-touch pan recogniser could also start on the remaining finger.
Fix, both halves: Swift sends pinch (and twist) events only while two fingers are down and ends
the pinch at the first lift; Rust ignores `Pan` and `TouchSteer` from the moment a pinch or twist
began until the platform says every finger is off (`InputEvent::TouchesLifted`, from a passive
`FingerTracker` recogniser on the view; `FocusLost` clears it too), and `touch_hold` says
`Nothing` meanwhile. A fresh touch afterwards pans as usual. The order of lifting does not
matter: both orders produce the same events. No stale fling: the release measures the pinch's
velocity at the first lift, and `FLING_STALE` drops it if the fingers had stopped. Mac
unchanged (it never sends `Pinch` or `TouchRotate`).

**2. "In cruise mode panning doesn't work, so you can't steer."** Not the 0.2 s hold (a drag
already steered at once in round 2): the finger steered like the Mac's *resting pointer*, a
joystick about the centre — direction and strength from where the finger is, a dead zone of
0.1 steering radii and a quadratic response (`steer_response`). On the iPad (radius about
420 points) a finger 100 points off centre gives a heading of about 30 points, the content then
drifts at 0.6 × 30 ≈ 17 points/s, towards the centre, i.e. *against* a drag outward. A drag of
a few centimetres seemed to do nothing, or the wrong thing. Decision: on touch, a drag in C is
direct manipulation, the same as a drag on the Mac in C: it pans the picture under the finger
while the cruise zooms on (gear unchanged; the release flings). Moving what you want to the
middle is how a finger steers. Hover steering has no touch equivalent; the round-1 mapping of
drag to hover steering was the mistake.
- *Journey's flight:* the same, for consistency (round 2 made J steer "like C"): a drag takes the
  wheel (the Mac's `is_steering(Pan)` path) and pans; the next drag pans the cruise. One meaning.
- *Autopilot stays a wish:* in A the autopilot chooses where to go; the finger only suggests a
  direction (round 1: a touch never hands over, so a child's autopilot keeps flying). A drag
  there still sets the wish at once and a resting finger after 0.2 s.
- *Long press:* `finger_steers` is A only; `touch_hold` gives `Nothing` in C and a journey's
  flight, and Swift then lets the long press not begin at all (`gestureRecognizerShouldBegin`),
  so a rest-then-drag pans and nothing waits for the release. The menu is unchanged in M (and
  during a fly-to the user started).

**3. "A line appears and won't go away" (autopilot).** The target marker's wish line
(`marker.rs`, drawn while `aim_marker().wish` is set). Cause: round 1's `TouchSteerEnd` in A
kept the finger's point as `touch_wish` until the next touch or until A ended, by design ("a
touch has no resting pointer"), so the line stayed for the whole flight. Now the lifted finger's
wish is held only until the autopilot's wish has settled on it (`WISH_SETTLE`, 0.3 s; at most
0.6 s), then let go like a pointer that left the canvas: the wish clears after another 0.3 s and
the line fades (0.35 s). A quick flick still re-targets the lock once towards it; the lock keeps
that target after the wish is gone ("nothing is re-targeted then"). Survey steering's pull towards
the wish ends with it. Cancelled touches send `TouchSteerEnd` as before.

**Tests.** Rust (core `input`): `a_finger_left_after_a_pinch_does_not_pan` (pinch held, one
finger lifted, the other pans and lifts while moving: view unchanged, no fling; a fresh touch
pans; in A no wish from it; a twist counts; `FocusLost` clears), `a_drag_pans_the_cruise_and_it_zooms_on`,
`a_drag_takes_the_wheel_from_a_journey_flight_and_pans`, `a_held_finger_steers_in_a_and_opens_the_menu_in_m`,
`touch_steering_wishes_in_a_and_steers_in_c` (the wish and its line gone 1 s after the lift; a
0.1 s flick settles first). XCTest: the view's accessibility value gained `wish on|off` (the
wish line drawn, `Explorer::wish_line_shown`); `TouchControlsUITests.testFingerInAutopilotAndCruise`
(A: no line 2 s after the lift; C: a drag pans with the finger, also after a 1 s rest, no menu,
gear kept), `testPinchThenDragPans`; `FirstUseUITests.testFirstSession` checks a drag after the
pinches, the line gone after steering the autopilot and a drag in the cruise. XCUITest cannot
lift the fingers of a pinch one at a time, so bug 1 itself is covered by the Rust test and needs
the device. Run 2026-10-18 on `DR iPad Pro 11` (iPad Pro 11-inch M5) and `DR iPhone 17e`: all
pass (screenshots looked at: the wish line right after the lift and gone 2 s later; the cruise
panned under the finger at gear 1 and 3). `testSmallTwistDoesNotTurn` failed once on the iPad
(a 9° synthesised twist turned 9.6°) and passed on four reruns (cause and fix: next paragraph);
the first-session cruise drag measures the distance, as XCUITest pinches had turned that view 85°.

**The flaky small-twist test (2026-10-18): the input, not the dead band.** Reproduced 6 of 10
runs on `AB iPad mini` (`-only-testing`, quiet machine), with the rotation recogniser traced.
XCTest's `rotate(0.15, withVelocity: 0.5)` is not an 8.6° twist: its recorded event paths
(the xcresult's "Synthesized Event" attachment) put the fingers down on a line 18.4° (atan 1/3)
off, jump them onto the start line 10 ms later, then turn in 12° keyframes and drop the last
one, so for 8.6° nothing follows the jump (`rotate(π/3)` likewise delivers 18.4° + 48°, not
60°). UIKit hands the jump over in one `.began` → `.ended` pair as 0.21472 rad (12.3°, the runs
that passed) or 0.42879 rad (24.6°, the runs that failed) from identical synthesised events —
18.4° ∓ 6.1°, UIKit's start hysteresis subtracted or, timing-dependent, added. The dead band
did exactly its job: 24.6° − 15° = the 9.57° seen (`handle`'s `TouchRotate` arm in
`crates/almondbread-core/src/input.rs`; nothing else turns the view on iOS, `twist` was armed).
Ruled out by the trace: a stale `twist` (fresh process, `TouchRotateEnd` sent on `.ended`), a
pointer or pinch path (`Rotate` is sent only by the Mac trackpad), animation overshoot (the
angle is final after the one event). Fix: the test synthesises a real small twist
(`Tests/iOSUITests/TwoFingerTwist.swift`: XCTest's own `XCPointerEventPath` /
`XCSynthesizedEventRecord`, 30 steps over 0.6 s about the view centre); UIKit then reports
about 3.8° after its hysteresis and the view stays put. Rust test
`a_single_twist_event_crossing_the_dead_band_turns_by_the_excess` pins both observed values.
Worth knowing for real fingers: UIKit's own start hysteresis (5–6°, rotation the recogniser
never reports) comes on top of the 15°, so a twist turns the view after about 20°. Checked: the
twist test 10 of 10 on `AB iPad mini` and once on `AB iPhone 17e`.

**Needs the device.** Pinch with one finger lifted early (both orders) — no jump, no pan, no
fling; whether a drag in C feels like steering (the cruise keeps zooming at the screen centre,
so a pan sets where it dives); whether the wish line in A is visible long enough to read.

### Rest or swipe (2026-10-18)

Device feedback on the iPad after the device round: in C (and a journey's flight) the drag now
panned, but the old way of steering, a finger resting on the screen and the flight turning
towards it while it stays down, was gone; the user wants both. In A the finger set the wish as
the direction *from the centre to the finger*, so a swipe to the left on the right half drew a
line pointing right: "random". Decided with the user: **one rule for C, a journey's flight and
A: a finger that rests goes there, a finger that swipes moves the picture.** Manual (nothing
flying, at a journey stop, a fly-to the user started) is unchanged: a drag pans, a long press
opens the menu. The Mac's pointer is unchanged.

| Input (touch), while A, C or a journey's flight flies | Action | Core |
| --- | --- | --- |
| Finger within 10 points of where it went down for 0.25 s | *rests*: steers until it lifts, also when it then slides (a joystick). C: like the Mac's pointer (heading spring, no idle timeout). A: the wish (line from the centre to the finger), let go once settled after the lift. Journey: takes the wheel as soon as it is off the steering dead zone (0.1 radius), then the cruise steers; inside the dead zone the leg flies on | `FINGER_SLOP`, `FINGER_REST`, `Navigator::finger_steer` |
| Finger that leaves the 10 points sooner | *swipes*: pans 1:1, the picture catching up from the touch-down point (the first 10 points included); the release flings. C: the gear stays. Journey: takes the wheel and pans, as before. A: pans **without handing over**, no wish, no line | `Navigator::finger_pan` |
| A tap (lifted before 0.25 s within the slop) | nothing from the finger (the tap pauses / resumes as before) | — |
| A finger lifted after 0.25 s with no frame in between, or whose first move after 0.25 s leaves the slop | it had rested (the rest is checked before the move) | `finger_rest_check` |
| Second finger (pinch, twist, two-finger tap) | the first finger is done: no rest, no swipe, no wish left; the finger left down after a pinch does nothing until every finger is off (as in the device round) | `FingerUp { cancelled: true }`, `pinched`, `TouchesLifted` |
| Finger down on a mini-map / a cockpit icon or (?) badge | a swipe pans (A: without handing over); a rest does nothing (the map's long press flies, the taps act) | `TouchSpot::Map` / `Control` |
| Finger down on the compass | never Rust's: its drag turns the view as before | `TouchSpot::Compass` |

**Events.** Swift sends the single finger raw: `FingerDown { x, y }`, `FingerMove { x, y }` (every
move), `FingerUp { cancelled }`, from the passive `FingerTracker` recogniser (which already
counted fingers for `TouchesLifted`; it never recognises, cancels or delays touches); a second
finger sends `FingerUp { cancelled: true }`. The host fills in where the finger went down
(`Host::touch_spot`: help badge or cockpit, compass, mini-map, picture; the core event carries a
`TouchSpot`). Rust classifies from the event times (`media_time` at arrival) and the frames
(`advance` checks a pending finger; a pending finger counts as animating, so a frame comes even
when nothing else moves). While Rust owns a finger (from `FingerDown` in A, C or a journey's
flight until `TouchesLifted`) it swallows the pan recogniser's `Pan` and `GestureEnd` for it (not
a pinch's `GestureEnd`, which still measures the pinch's fling). Elsewhere `FingerDown` is
ignored (returns false) and the recogniser pans as before. `TouchSteer` / `TouchSteerEnd`, the
long press's `Steer` hold (`TouchHold::Steer`, the 0.2 s `steerHold`) and the frame bit
`FRAME_FINGER_STEERS` are gone; `touch_hold` says `Nothing` on the picture in A, C and J (Rust's
finger), so no long press begins there.

**Why raw, in Rust.** `UIPanGestureRecognizer` starts only after about 10 points and its
translation starts there, so it cannot tell a finger that stayed put from one that has not
reached its threshold yet, and its first 10 points are hidden. The passive recogniser sees every
touch from the first point; the decision (distances, times, what each level does) is policy, so
it is Rust's and unit-tested.

**Thresholds.** 10 points is about UIKit's own pan start distance (what feels like a drag to the
system is one here too; a resting fingertip's tremor stays well inside it). 0.25 s: a deliberate
swipe crosses 10 points in under 0.1 s, and a finger put down to steer acts almost at once
(round 2 used 0.2 s for the long press that steered). A slow drift of 7 points over 0.23 s still
rests (test). *Needs the device:* whether 10 points / 0.25 s suits small fingers.

**A and a swipe: no fight.** A touch swipe in A pans the view directly (not through the Mac's
`Pan` hand-over; a Mac drag still hands over to cruise). It sets `manual_at`, so for
`autopilot::MANUAL_HOLD` (1 s) after the last move, and while the fling runs, the autopilot is
not `free`: the target lock releases its target (`Lock::update` with `free = false` drops target,
next and pending search) and the heading eases to straight ahead. Then the lock picks up a target
anew from the moved view (pickup search, off-thread as always). Nothing pulls the picture back
to the old target: it no longer exists. No code in the autopilot changed. Test
`a_swipe_in_a_pans_and_the_autopilot_re_aims` (seahorse view, synchronous lock: 240 points of
swipe, level stays A, no wish, the lock empty right after the lift, a new target 3 s later, no
rescue).

**Journey.** It has steering (the cruise after a hand-over), so the rule holds there too: a
resting finger takes the wheel once off the dead zone (as round 2 had it), a swipe takes the
wheel and pans (as the device round).

**Tests.** Rust (core `input`): `a_resting_finger_steers_and_a_swipe_pans_in_c` (rest steers
past the idle timeout, rest-then-slide keeps steering and does not drag, swipe catches up 1:1
from the touch-down point, the recogniser's `Pan` ignored, fling, gear kept),
`a_resting_finger_wishes_in_a` (wish and line, let go after the lift, a tap and a rest near the
centre leave none, a rest found only at the lift counts, Z off / on), `a_swipe_in_a_pans_and_the_autopilot_re_aims`,
`a_slow_drift_within_the_slop_rests` (7 points in 0.23 s rests; 12 points in 0.15 s swipes; still
then 50 points at 0.3 s had rested), `a_held_finger_steers_in_flight_and_opens_the_menu_in_m`
(`touch_hold` per level and spot; in M `FingerDown` is not Rust's and the recogniser's drag pans
and flings; compass never, mini-map rest steers nothing; a fly-to is not steering),
`a_resting_finger_takes_the_wheel_from_a_journey_flight_and_steers`,
`a_second_finger_ends_the_resting_or_swiping_finger` (cancelled pending and steering fingers, a
pinch before the cancel, moves and pans ignored until `TouchesLifted`), and the device round's
`a_finger_left_after_a_pinch_does_not_pan` on the new events. XCTest:
`TouchControlsUITests.testRestSteersSwipePans` (A: hold then slide, no menu, A stays, line gone
after the lift; a quick swipe moves the picture, no wish line, A stays. C: a quick swipe pans
with the finger, gear kept; a 1 s hold right of the centre then a slide further right moves the
centre right (a pan would move it left); M: the menu) replaces `testFingerInAutopilotAndCruise`;
`FirstUseUITests.testFirstSession` holds before steering the autopilot and adds a swipe there.
Run 2026-10-18 on `DR iPad Pro 11` (iPad Pro 11-inch M5 simulator, `-only-testing` the two): both
pass. Screenshots looked at: the wish line right after a rested finger lifted in A, none after a
swipe; the A swipe moved the centre by about 960 / 700 points (in the end scale, the autopilot
zooming on), the C swipe 375 / 272, and a 1 s hold right of the centre then a slide further
right moved the cruise's centre 335 points *right* (steering; a pan moves it left).

**Needs the device.** Whether 10 points / 0.25 s separate a rest from a swipe for real (small)
fingers; whether a rest-then-slide feels like a joystick; how the A swipe and the lock's re-aim
after 1 s feel.

### Round buttons: the Mac toolbar on touch (2026-10-18)

User, after trying it on a real iPad: "The toolbar only had the autopilot and journey button. But
those are already handled in the cockpit. I'd like to have the orbit, ruler, minimap button as on
macOS." The round 44-point buttons over the canvas (`ContentView.swift`, `RoundToggle`,
`RoundButton`) are now the macOS toolbar's items, same SF Symbols, in its order: **Orbit**
(`orbitEnabled`), **Mini-map** (`mapEnabled`), **Size scale** (`cosmicScale`, "ruler"), **Fly to**
(the fly-to sheet), **Control center**. A toggle that is on is a circle filled with the accent
colour and a white symbol (off: the HUD's material); accessibility: the name as label, value
"on" / "off", the toggle trait. The Mac is unchanged.

**Where the removed actions went.**

| Was (iOS round buttons) | Now |
| --- | --- |
| Autopilot button (Z) | cockpit plane icon (tap) |
| Journey button (J) | cockpit sparkles icon; control center Journey |
| Journey stop ▶ / ↘ / ⑂ | "Next", "Deeper here", "Elsewhere" under the HUD, only at a stop (`JourneyStopButtons`); the sparkles icon is also J = next there |
| Autopilot menu: Cruise In / Out | throttle lever; cockpit cruise icon (E) |
| Autopilot menu: Stop | cockpit steering wheel (Esc); throttle double tap; control center Stop |
| Autopilot menu: Lane Assist | cockpit lane icon; control center View |
| Autopilot menu: Seek Minibrot | control center View "Seek minibrot" (new, iOS only; works while flying); long-press "Seek Minibrot Here" with nothing flying |

**Decisions.**

- *No menu on the cockpit's plane.* The cockpit is drawn by Rust; a long press there is
  `touch_hold → Nothing` in `input.rs`, and a menu would need a new hold result and its handling in
  `FractalView.swift`. Everything in the old menu except seek already had a cockpit or throttle
  equivalent, so the one gap (seek about the view while flying) went into the control center.
- *Journey stop choices under the HUD, labelled.* Kept in the button stack they would make eight
  items plus the throttle in the phone's portrait column (about 680 points: no room on a 667-point
  phone). Under the HUD they sit next to the line that names the landmark, and words ("Deeper
  here") beat ↘ and ⑂ for a first-time user. A row where it fits, else a column (phone portrait).
  The iOS HUD line at a stop drops "— next · deeper · elsewhere" (the buttons say it).
- *Orbit without a pointer.* `Navigator::probe_point` is the touch probe if one was placed (Orbit
  Here, a long-press drag), else the zoom target, which on touch (no pointer, pointer mode) is
  the view centre; an iPad pointer hovering moves it as on the Mac. So the toggle alone shows the
  orbit of the view centre, or of the last point picked; no code change was needed, only the help
  texts (overlays page, orbit popover) say it.
- *Fit.* Five buttons: phone portrait a column of 5 × 44 + 4 × 10 = 260 points under the throttle
  (its height `canvasHeight − 528`, was − 420 for three buttons; clamped 120 … 220); phone on its
  side a 260-point row (the cockpit inset already reserved five); iPad a top-trailing row, clear of
  a 380-point HUD on an iPad mini in portrait (744 points).
- *Touch hints.* "Flies by itself" now points down at the cockpit's plane icon (its rect from
  `Explorer::cockpit_controls`, read twice a second while the hints show); the middle group stays
  above that label.

**Tests.** `TouchControlsUITests.testOverlayToggles`: each toggle reads "on", the screen outside
the buttons changes (2 × 2-point grid, colour step > 60) and is back after "off"; Fly to opens its
sheet; screenshots `toggle-*`. The other iOS UI tests tap the cockpit's icons instead of the
removed buttons, and `testSurfaces` checks the stop buttons are ≥ 44 points, hittable and clear
of the round buttons in both orientations.

### Closing the control center on touch (2026-10-18)

Device feedback: with the control center open, it was not clear how to close it (the phone's
sheet closes by a swipe down, the iPad's side pane only by the same round button). Approved
design, both: a **✕** (`xmark.circle.fill`, 44-point target, label "Close control center", id
`control-center-close`) at the trailing end of the search bar, which is the pane's top
`safeAreaInset`, so it is pinned and never scrolls away; and the **Control center** round button
drawn like a `RoundToggle` that is on (accent fill, white symbol) while the pane is open, with the
selected trait (value still "shown" / "hidden"). On a phone in portrait the medium-detent sheet
covers the bottom-trailing button column, so there the ✕ is what shows. The Mac is unchanged (no
✕: the toolbar button and ⌘I). Test: `TouchControlsUITests.testControlCenterClose`.

### Position lever (2026-10-18)

User feedback from a real iPad: "It took me a while to understand the gearing. It shifts
according to how much you drag on the gearing bar. I was not checking that and thought it's
just 'gear up' and 'gear down'." The jog lever (the knob springs back, every 14 points of drag
is one gear) hid the gear in the drag length. The user approved a **position lever**: where the
knob stands *is* the gear.

| Touch | Action | Core |
| --- | --- | --- |
| Drag the knob (or anywhere on the lever) | the knob snaps from tick to tick under the finger, a selection haptic per gear (rigid at M); each new tick sets that gear at once; the knob stays where it is left. No detent at M: one drag goes from forward into reverse | `SetGear { gear }` |
| Tap a tick | that gear (after the double-tap wait, about 0.3 s) | `SetGear` |
| Tap a chevron at either end | one gear up / down; two quick taps on a chevron two gears (not the stop) | `SetGear` (the shown gear ± 1) |
| Double tap (not on a chevron) | manual (Esc), as before | `KeyDown(Stop)` |
| VoiceOver swipe up / down | one gear | `SetGear` |

**`SetGear { gear }`** (core `InputEvent`, FFI `InputEvent::SetGear`; `Navigator::set_gear`):
gear ±1…±26 is ± the ladder's rung (`cruise::gear_speed`), 0 stops. Whoever drives takes it as
its set speed: the cruise from rest or in C (`Cruise::set_to`; 0 brakes to rest, then M; reverse
refused at home, like Q); in A the autopilot's speed, in J the journey's; 0 or reverse in A or J
hands over to a cruise at that speed (as the pedals do at zero); a fly-to takes |rung| as its
speed and 0 stops it; the cruise's soft landing stops for 0 or forward. Clamped to ±26. The
relative `Throttle { up }` / `ThrottleEnd` events stay (keys and the Mac do not use them; the
lever no longer does either). The chevrons send the shown gear ± 1, not a pedal step: the
autopilot's default ×2.46/s sits just under rung 7 (×2.5), so E from it lands on gear 7 again
and the knob would not move.

**What the knob shows.** Always the navigator's gear: `CruiseStatus.gear` =
`cruise::signed_gear` of the cockpit's set speed (the rung nearest it, signed; 0 at rest), so it
moves when E / Q, the cockpit, the autopilot button's menu, Stop or a journey change the speed.
In C the cruise's gear. In A and J the gear nearest their set speed (A's default ×2.46/s shows
+7; J's ×4/s default +9); speeds beyond the ladder show +26. Their ranges clamp: the autopilot
stops at 4 e-folds/s (×55/s), so a drag to +26 in A springs back to +20 (the lever sets what
Rust made of it and shows that). A journey stop and the cruise braking show M. During a plain
fly-to the cockpit now reports the fly-to's own speed (before: the journey's, meaningless
there), so the knob shows it. Labels: "M", "+3" (black on a white knob) and "−2" (white on an
orange knob, matching the reverse bar), 17-point bold rounded on a 44 × 26 knob; the bar from
the middle notch to the knob stays. Spoken: "gear 3 in", "gear 2 out", "stopped", prefixed
"autopilot," / "journey,".

**Ticks.** 26 gears a side need 520 points at the 10-point minimum spacing; no lever has that.
So the ticks stand for fewer gears: `cruise::lever_ticks(n)` (FFI `throttle_ticks`) gives n gears
from 1 to 26, tick k at `26^((k−1)/(n−1))` rounded and at least one above the last, so the low
gears (those used most: cruise 1–10, the autopilot's 7) are all there or nearly, and the top
gear is one drag away. The chevrons and the keys reach every gear between; such a gear sits
between its two ticks (log-interpolated). n = ⌊(height/2 − 26 chevron cell − 13 half knob) /
10⌋:

| Lever | Height | Ticks a side | Gears |
| --- | --- | --- | --- |
| iPhone portrait (17e) | 220 | 7 (10.1 pt) | 1 2 3 5 9 15 26 |
| iPhone on its side | 190 | 5 (11.2 pt) | 1 2 5 12 26 |
| iPad (was 240, now up to 360) | 360 | 14 (10.1 pt) | 1–10 12 16 20 26 |
| narrow split view minimum | 120 | 2 | 1 26 |

The iPad lever grew from 240 to at most 360 points (`ContentView`; a phone on its side keeps
240): at mid height on the trailing edge there is room, and all the cruise gears up to 10 get
their own tick. A finger is wider than 10 points, but the knob snaps and every gear clicks, so
the finger is guided by feel and by the big label, not by aiming.

**Words.** First-run hint at the lever: "Up: in · Down: out · M: stop" (was "Slide to fly in or
out"). Help (getting started, cockpit popover) and the control center's touch list describe the
lever as a gear lever that stays where it is left.

**Tests.** Rust `set_gear_puts_the_level_on_that_rung` (core `input`: from rest, across zero
without a detent, keys move the shown gear, 0 brakes to M, clamps, no reverse at home, A: its
speed, clamped to gear 20, reverse hands over; fly-to: its speed, 0 stops),
`lever_gears_and_ticks` (core `cruise`). XCTest `TouchControlsUITests.testThrottle`: a drag to
the third tick and release (gear = that tick's, the knob stays 2 s later, zooms in); chevron up
one gear; two chevrons down; one drag through M into reverse; a tap on the first tick; double
tap to M; in A the knob shows its gear, a chevron steps it and stays A; the autopilot button's
"Cruise In / Faster" moves the knob. The lever reports its ticks, pitch and chevron cell in its
accessibility value with `ALMONDBREAD_UITEST=1` (`ThrottleLeverUI` in the UI tests aims with
them). Passed on the iPhone 17e simulator (portrait and landscape layout).

**Needs the device.** Whether 10-point ticks are easy enough for a child's finger (the knob
snaps, but a finger lifting off can roll one tick); whether the 0.3 s wait for a tap on a tick
feels slow.
