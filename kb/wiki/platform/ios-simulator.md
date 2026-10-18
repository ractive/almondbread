---
title: "iOS and iPadOS on the simulator: setup, checks, what broke"
type: platform
status: draft
sources: ["[[sources/apple-simulator-tooling]]"]
tags: [ios, ipad, simulator, testing, xctest, ui]
---

# iOS and iPadOS on the simulator: setup, checks, what broke

The first runs of the iPhone/iPad app (2026-10-15, Xcode 26.6, iOS 26.5 simulator runtime,
M1 Pro). Simulator Metal says nothing about speed: these checks are correctness and UI only.
Port background: [[platform/ios-ipad-considerations]].

## Setup

- Runtime: `xcodebuild -downloadPlatform iOS` (8.5 GB, iOS 26.5 23F77, ~10 min).
- Simulators: `xcrun simctl create "AB iPhone 17e" com.apple.CoreSimulator.SimDeviceType.iPhone-17e
  com.apple.CoreSimulator.SimRuntime.iOS-26-5` and `"AB iPad mini"` (iPad mini A17 Pro).
  `simctl boot` runs them headless: no Simulator.app window, nothing on the user's screen.
- `make ios-selftest` (`scripts/ios-selftest.sh`): installs the `make ios-sim` build, launches
  with `SIMCTL_CHILD_ALMONDBREAD_SELFTEST=1` and `--selftest-png=selftest.png` (a relative
  path goes to the app's Documents, fetched through `simctl get_app_container … data`),
  waits for `renderer OK` on `--console-pty`, copies the PNG to `build/ios-selftest-<device>.png`
  and takes a `simctl io … screenshot` of a normal launch.
- `make ios-uitest` (`scripts/ios-uitest.sh`, target `Almondbread iOS UITests`,
  `Tests/iOSUITests/TouchUITests.swift`): XCTest synthesises real UIKit touches — a one-finger
  drag, `pinch(withScale: 3)`, `rotate(π/3)`, double tap, two-finger tap — and taps the
  autopilot, journey and control-center buttons, then rotates the device. The fractal view is
  one accessibility element (`fractal`, trait `allowsDirectInteraction`, so VoiceOver passes
  touches through) whose value is the view: `re im scale angle° autopilot on|off journey on|off`;
  the test asserts each gesture moved, zoomed or turned it. Screenshots go to the result
  bundle and, via `TEST_RUNNER_ALMONDBREAD_UITEST_SHOTS`, to `build/ios-screens/`.
  `xcodebuild test` does not open Simulator.app either.

## What broke, and the fixes

1. **Crash at the first frame**: `-[CAMetalDrawable addPresentedHandler:]: unrecognized
   selector`. The simulator's drawables do not implement presented handlers; the Objective-C
   exception unwinds into Rust ("Rust cannot catch foreign exceptions") and aborts. Found
   with `simctl launch --wait-for-debugger` and lldb on `objc_exception_throw`. Fix
   (`metal/surface.rs`): ask `respondsToSelector:` once; without handlers a frame counts as
   presented at once, without a presentation time.
2. **Control center never appeared, app frozen**: tapping the control-center button set
   `showInspector` (the button's accessibility value said "shown") but nothing presented;
   later XCTest queries timed out. `sample` of the app showed the main thread blocked forever
   in `CMPhotoCodecSessionPoolCreateDecompressionSession` (a semaphore) inside a Core
   Animation commit that lazily decoded a place thumbnail (HEIC), while SwiftUI's
   `prepare-image` threads decoded other thumbnails: a decoder-session-pool deadlock in the
   simulator's HEIF path. Screenshots showed the last frame before the hang, which looked
   like "the sheet does not open". Fix (`ControlCenterView.swift`, `PlaceThumbnails`): every
   thumbnail is decoded on one serial queue and drawn into a plain sRGB bitmap before SwiftUI
   sees it (`Image(decorative:scale:)`), cached. On devices this also removes decode hitches
   while scrolling the grid (inference). The 69-thumbnail grid also makes XCTest's
   accessibility snapshot slow; queries after opening still work.
3. **HUD under the buttons on iPhone**: the HUD (zoom, iterations, GPU, reference, speed)
   ran under the top-trailing autopilot / journey / control-center buttons at 390 pt width.
   Now, on a compact width — and on any canvas narrower than 560 pt, which includes an iPad
   portrait canvas next to the control-center sidebar — the HUD drops the renderer statistics
   (GPU, chunks, size, reference) and wraps at 230 pt, and the buttons stack at the bottom
   trailing corner (thumb reach, clear of the HUD and the bottom-left mini-map). On wider
   iOS canvases the HUD wraps at 380 pt so a journey line does not run under the buttons
   (iPad portrait, seen in the first iPad run).
4. **Status bar over the canvas**: SwiftUI's `statusBarHidden()` on the canvas did not hide
   it; `UIViewControllerBasedStatusBarAppearance = NO` with `UIStatusBarHidden` does.

Gestures needed no fix: drag pans (scale unchanged), pinch ×3 zooms > 1.5×, rotation turns
> 20°, double tap zooms in, two-finger tap zooms out, the autopilot and journey buttons start
and stop them, on both devices. (Those round buttons became the Mac toolbar's items on
2026-10-18; the cockpit's icons start and stop the autopilot and journeys now:
[[decisions/controls]] "Round buttons: the Mac toolbar on touch".)

## Results (2026-10-15)

- Self-test: iPhone 17e `renderer OK (Rust) 1170x2532 drawable`, shown vs fresh render
  0 pixels differ, places 69, thumbnails none missing; iPad mini `1488x2266`, 0 pixels.
  PNGs `build/ios-selftest-ab-iphone-17e.png`, `-ab-ipad-mini.png` (looked at: the home view).
- UI test passes on both after the fixes; screenshots in `build/ios-screens/`
  (`iphone-*`, `ipad-*`: start, after gestures, autopilot, journey, control center,
  landscape; looked at).

## Touch controls tests (2026-10-18)

- `Tests/iOSUITests/TouchControlsUITests.swift` ([[decisions/controls]] "Touch"): XCTest drags the
  throttle (`press(forDuration:thenDragTo:withVelocity:thenHoldForDuration:)`, 14 points per
  detent), taps the Rust-drawn cockpit through the real hit test, long-presses for the menu and
  steps the tour card (`--ui-demo=tour-feigenbaum` now starts the tour on iOS too). The fractal
  view's accessibility value grew `level m|c|a|j gear n orbit on|off seek phase assist on|off`
  and, with the launch environment `ALMONDBREAD_UITEST=1` only, ` cockpit id:x,y …` (the cockpit
  cells and help badges in view points), so a test can aim at what Rust drew.
- `UIEditMenuInteraction` on a 390-point iPhone shows two items and a page arrow: XCTest did not
  find the third item. The long-press menu is a SwiftUI popover instead.
- Several agents share the machine: separate simulators (`ABT iPhone 17e`, `ABT iPad mini`, made
  with `simctl create` like the others) keep one agent's test runs and persisted settings away
  from another's; `IOS_DEVICES="ABT iPhone 17e,ABT iPad mini" make ios-uitest`.
- Persisted settings again: "Orbit Here" in one test leaves the orbit and the mini-map on for
  the next launch (the screenshots then show the touch cockpit lifted above the map row).

## Touch round 2 (2026-10-18 evening)

- Fresh simulators (`AB2 iPhone 17e`, `AB2 iPad mini`) took about 7 minutes to boot the first
  time ("Waiting on Data Migration", locationd migrator); `simctl launch` hangs until then.
  Wait with `simctl bootstatus <name> -b` before installing.
- XCUITest screenshots in landscape come out rotated (the portrait buffer); read them sideways.
- `XCUIApplication.typeKey("m", …)` reaches the app's `pressesBegan` on the simulator (used for
  M, the mini-map).
- The control center's search field is the quickest way to a row in a test (`"lock"` shows the
  View section alone; `"fly"` also lists every place first).
- On iOS, reading `UIPasteboard.general.string` asks "Allow paste?" (seen when the fly-to sheet
  opened); `hasStrings` and SwiftUI's `PasteButton` do not.
- Settings no longer persist from the UI tests or `--ui-demo` (`TestLaunch.ephemeral`;
  [[decisions/controls]] "Touch", round 2): the open item below is fixed.

## QA: a first-time user (2026-10-18, before a child's first try)

`Tests/iOSUITests/FirstUseUITests.swift` plays what a newcomer does, a screenshot per step
(`build/ios-screens/<idiom>-first-<test>-NN-*.png`, all looked at): `testFirstSession` (hints,
dismiss, drag, five pinches, the plane and a steering finger, the plane again, throttle forward
and reverse and its double tap, a journey on and off with its button, every orientation, home
and back while cruising, the cockpit's manual icon ends it), `testControlCenterPlaceAndTour`
(tap a place's picture; the Feigenbaum tour from the Tours section, Next to Finish),
`testLongPressMenuItems` (every item; Go to Copied through the Paste control; the seek must leave
"searching"), `testMashing` (taps, double taps, pinches, twists, buttons twice, three rounds; the
view stays finite, the cockpit stops everything), `testSmallTwistDoesNotTurn`. Simulators
`QA iPad Air 11` (iPad Air 11-inch M4) and `QA iPhone 17e`.

| Found | Severity | Fix |
| --- | --- | --- |
| Long-press "Seek Minibrot Here" stayed "searching" with nothing on screen until the next touch: `SeekAt` (and `Probe`) asked for no frame, so the search was never polled (the round-1 test accepted "searching") | high | `Navigator::wants_frames` (motion or a search); the host's redraw uses it and counts `Probe` as orbit input |
| Tapping a place's picture in the control center did nothing visible: it only selected, and the name and Go sit below the 79-tile grid, off screen | high | iOS: a tap goes there (and selects); replaced the same day: a tap selects and puts Go and Fly on the picture, a second tap goes ([[decisions/controls]] "Touch", places grid) |
| Pinching turned the view (XCUITest: 32° per synthesised pinch on both devices; real fingers twist a little too), the compass and "turned 78°" appeared | medium | touch twist dead band of 15° in Rust (`TouchRotate`, `TouchRotateEnd`); the Mac's trackpad `Rotate` stays 1:1 |
| "Go to Copied" read the clipboard itself: "Allow Paste?" on every tap for text from another app | medium | a system `PasteButton` beside the row (and in the control center's Copy / Paste / Reset row); the row's title keeps the old path |
| Keyboard and mouse words on touch: "click to resume" (paused autopilot or journey), the Tours footer's → ← Esc, the tour's last step ("the pointer", → ← Esc) | low | iOS wording; the tour text names both |
| iPhone on its side: the compass overlapped the throttle's top by a few points | low | the throttle 30 points shorter there |

Seen and fine: hints on both devices; rotations (a frame or two of the old size during the
animation, then correct); home and back resumes the cruise with a correct picture; the tour card
on both; journeys; the long-press menu at both edges; mashing left no stuck state (the cockpit's
manual icon always ends A, C and J); no crash, hang or black frame.

Not fixed, worth knowing: a drag at the start easily moves the set off screen, and pinching in
then shows only empty blue (pinch out, the throttle's reverse, or Reset bring it back — no hint
says so); at a journey stop the journey button (since 2026-10-18 the cockpit's sparkles icon) flies on rather than stopping (stop: cockpit M or
the throttle's double tap); HUD lines and help popovers keep words a child will not know
("branch point M(2,1)", "embedded Julia (period 54)"); XCUITest pinches zoom much less than their
scale (×1.5 for "×3"), so pinch feel needs the device. XCUITest's `rotate(_:withVelocity:)` starts
with an 18.4° finger jump (fingers down 18.4° off the start line, moved onto it 10 ms later) and
drops its last 12° keyframe, and UIKit reads the jump as 12.3° or 24.6° run to run: no use for
angles near a threshold; `Tests/iOSUITests/TwoFingerTwist.swift` draws an exact twist through
XCTest's event record instead ([[decisions/controls]] "Touch", the flaky small-twist test).

## Open

- ~~The `--ui-demo` scenes write their settings into the persisted preferences~~ (fixed
  2026-10-18, round 2).
- One iPad run found the control center's state already "shown" at the start of the
  control-center step (not reproduced in the next run).
- No device run yet: speed, thermal behaviour and ProMotion pacing on an iPhone/iPad are
  unmeasured.
