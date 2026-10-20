---
title: "iOS and iPadOS on the simulator: setup, checks, what broke"
type: platform
status: draft
sources: ["sources/apple-simulator-tooling"]
tags: [ios, ipad, simulator, testing, xctest, ui]
---

# iOS and iPadOS on the simulator: setup, checks, what broke

The first runs of the iPhone/iPad app (2026-10-15, Xcode 26.6, iOS 26.5 simulator runtime,
M1 Pro). Simulator Metal says nothing about speed: these checks are correctness and UI only.
Port background: [iOS and iPadOS port considerations](ios-ipad-considerations.md).

## Setup

- Runtime: `xcodebuild -downloadPlatform iOS` (8.5 GB, iOS 26.5 23F77, ~10 min).
- Simulators: the shared pair `AB iPhone 17e` (iPhone 17e) and `AB iPad mini` (iPad mini A17
  Pro), created on demand by `scripts/sim-common.zsh` (`simctl create`, newest iOS runtime) and
  used by every worktree; see "Low-load rules". `simctl boot` runs them headless: no
  Simulator.app window, nothing on the user's screen.
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

## Low-load rules (2026-10-19)

Why: on 2026-10-18 several agents in parallel worktrees each booted their own simulator pair
(`ABT`, `DR`, `DT`, `GL`, `PG`, `TB`, …) and ran `xcodebuild test` at the same time: 12
simulators, load average ~800 on 10 cores, 32 GB RAM swapping, the Mac unusable. Each pair also
kept 1.5–3.6 GB of simulator data. The rules now, enforced by the scripts:

- **One simulator job at a time on the machine.** `scripts/sim-common.zsh` `sim_lock` takes an
  fcntl lock (`zsystem flock`) on `~/Library/Caches/almondbread/sim.lock` before any simulator
  is booted; a second job prints `waiting for the simulator lock held by <job> in <worktree>, pid
  N, since hh:mm:ss` (from `sim.lock.holder`), polls every 5 s, repeats the message every 5 min
  and gives up after 90 min (`SIM_LOCK_WAIT` seconds). The lock dies with its process, so a
  crashed or killed job never leaves a stale lock. Builds run before the lock, the simulator
  part inside it.
- **One shared pair** for every worktree: `AB iPhone 17e`, `AB iPad mini` (fixed names, created
  on demand, reused). Each job boots one device at a time headless, waits for `bootstatus`, and
  shuts it down by UDID when done (also on failure, an EXIT trap) — never `simctl shutdown all`,
  never a private `simctl create`. UI tests run with `-parallel-testing-enabled NO` (no
  "Clone 1 of …" simulators). Other booted simulators are reported, left alone.
- **Targeted runs for agents**: `make ios-uitest TESTS="TouchControlsUITests/testThrottle"
  DEVICE=ipad` (`TESTS`: comma-separated `Class` or `Class/testMethod`, mapped to
  `-only-testing:Almondbread iOS UITests/…`; `DEVICE=iphone|ipad|both`, default both). The full
  suite on both devices is run once by the driver. `make ios-selftest [DEVICE=…]` likewise.
  `ios-uitest` is `build-for-testing` (unlocked, `ARCHS=arm64`: the generic simulator
  destination otherwise also links x86_64 and fails on the arm64-only Rust library) then
  `test-without-building` against the booted shared device (`-destination id=UDID`).
- **Test builds at low priority**: `ios-selftest` / `ios-uitest` run their `make rust`, the
  simulator build and `build-for-testing` under `nice -n 10`, xcodebuild `-jobs 6`,
  `CARGO_BUILD_JOBS=6` (Make target-specific variables, inherited by prerequisites). `make app`,
  `build`, `run` and `ios-sim-run` keep full speed. Measured 2026-10-19 (M1 Pro 8P+2E, clean
  release build of `almondbread-ffi` for `aarch64-apple-ios-sim`, other agents' builds running):
  normal 84 s, `nice -n 10` 76 s, `CARGO_BUILD_JOBS=6` 78 s, `taskpolicy -b` **673 s** (8×:
  background QoS confines the build to the 2 E-cores and throttles I/O) — so `nice`, not
  `taskpolicy -b`. Six jobs cost a single build nothing measurable (the tail of the crate graph
  is serial) and cap the peak when several agents build at once. Note `nice` does not lower the
  load average: it only lets interactive work go first.
- **`make ios-sim-run [DEVICE=iphone]`** (`scripts/ios-sim-run.sh`): the Debug simulator build in
  the shared iPad (or iPhone) for the user to try by hand. The user's command, so it opens
  Simulator.app (`open -a Simulator --args -CurrentDeviceUDID`). It holds the simulator lock
  while it runs: otherwise a test job would install over the app and shut the simulator down
  under the user; agents' jobs wait and name it as the holder. Foreground with the app's
  output; Ctrl-C (or quitting the app) terminates the app, shuts the simulator down, quits
  Simulator.app if it started it, releases the lock. `SIM_RUN_NO_WINDOW=1` runs it headless
  (checks), `SIM_RUN_ARGS` passes launch arguments. Not for agents.
- Measured 2026-10-19: `make ios-selftest` both devices in sequence ~45 s each after the build;
  two concurrent `make ios-selftest` (iPad, then iPhone 20 s later): the second printed the
  waiting line and got the lock after 15 s, the iPad was shut down before the iPhone booted.
  Targeted `ios-uitest TESTS=TouchControlsUITests/testThrottle DEVICE=ipad`: 67 s wall
  including an incremental `build-for-testing`, 42 s of testing, one simulator booted. The
  machine's load average during these runs (150–300) came from other projects' cargo builds;
  the lock does nothing about those.

Disk: each worktree builds its own `.build/xcode-ios` (simulator app, ~0.4 GB),
`.build/xcode-ios-test` (UI-test DerivedData, ~0.35 GB, more after full builds), `.build/xcode`
and a `target/` (Rust, ~11 GB in the main checkout). A shared DerivedData across worktrees is
deliberately not used (builds of different branches would overwrite each other's products).
Delete `target/`, `.build/` and `build/` of finished worktrees. Old per-agent simulators:
`xcrun simctl list devices`, then `xcrun simctl delete <UDID>` for shut-down ones with an agent
prefix (`ABT`, `DR`, `DT`, `GL`, `PG`, `QA`, `AB2`, `TB` …); keep `AB iPhone 17e`, `AB iPad mini`
and Xcode's default devices. Deleted 2026-10-19: the `ABT`, `DR`, `DT`, `GL` and `PG` pairs/singles
(10 simulators, ~21 GB); the `TB` pair was kept for an agent still running.

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
[Controls: keyboard, pointer and touch](../decisions/controls.md) "Round buttons: the Mac toolbar on touch".)

## Results (2026-10-15)

- Self-test: iPhone 17e `renderer OK (Rust) 1170x2532 drawable`, shown vs fresh render
  0 pixels differ, places 69, thumbnails none missing; iPad mini `1488x2266`, 0 pixels.
  PNGs `build/ios-selftest-ab-iphone-17e.png`, `-ab-ipad-mini.png` (looked at: the home view).
- UI test passes on both after the fixes; screenshots in `build/ios-screens/`
  (`iphone-*`, `ipad-*`: start, after gestures, autopilot, journey, control center,
  landscape; looked at).

## Touch controls tests (2026-10-18)

- `Tests/iOSUITests/TouchControlsUITests.swift` ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Touch"): XCTest drags the
  throttle (`press(forDuration:thenDragTo:withVelocity:thenHoldForDuration:)`, 14 points per
  detent), taps the Rust-drawn cockpit through the real hit test, long-presses for the menu and
  steps the tour card (`--ui-demo=tour-feigenbaum` now starts the tour on iOS too). The fractal
  view's accessibility value grew `level m|c|a|j gear n orbit on|off seek phase assist on|off`
  and, with the launch environment `ALMONDBREAD_UITEST=1` only, ` cockpit id:x,y …` (the cockpit
  cells and help badges in view points), so a test can aim at what Rust drew.
- `UIEditMenuInteraction` on a 390-point iPhone shows two items and a page arrow: XCTest did not
  find the third item. The long-press menu is a SwiftUI popover instead.
- ~~Several agents share the machine: separate simulators per agent (`ABT …`,
  `IOS_DEVICES=…`)~~ — superseded 2026-10-19: one shared pair and a machine-wide lock, see
  "Low-load rules" below. (Settings no longer persist from tests, so sharing is safe.)
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
  [Controls: keyboard, pointer and touch](../decisions/controls.md) "Touch", round 2): the open item below is fixed.

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
| Tapping a place's picture in the control center did nothing visible: it only selected, and the name and Go sit below the 79-tile grid, off screen | high | iOS: a tap goes there (and selects); replaced the same day: a tap selects and puts Go and Fly on the picture, a second tap goes ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Touch", places grid) |
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
XCTest's event record instead ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Touch", the flaky small-twist test).

## Open

- ~~The `--ui-demo` scenes write their settings into the persisted preferences~~ (fixed
  2026-10-18, round 2).
- One iPad run found the control center's state already "shown" at the start of the
  control-center step (not reproduced in the next run).
- No device run yet: speed, thermal behaviour and ProMotion pacing on an iPhone/iPad are
  unmeasured.

# Citations

- [Apple simulator and UI-test tooling (simctl, xcodebuild, XCTest)](../sources/apple-simulator-tooling.md)
