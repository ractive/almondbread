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
and stop them, on both devices.

## Results (2026-10-15)

- Self-test: iPhone 17e `renderer OK (Rust) 1170x2532 drawable`, shown vs fresh render
  0 pixels differ, places 69, thumbnails none missing; iPad mini `1488x2266`, 0 pixels.
  PNGs `build/ios-selftest-ab-iphone-17e.png`, `-ab-ipad-mini.png` (looked at: the home view).
- UI test passes on both after the fixes; screenshots in `build/ios-screens/`
  (`iphone-*`, `ipad-*`: start, after gestures, autopilot, journey, control center,
  landscape; looked at).

## Open

- The `--ui-demo` scenes write their settings (orbit, map, interior colouring) into the
  persisted preferences, so a later normal launch on the same simulator starts with them.
- One iPad run found the control center's state already "shown" at the start of the
  control-center step (not reproduced in the next run).
- No device run yet: speed, thermal behaviour and ProMotion pacing on an iPhone/iPad are
  unmeasured.
