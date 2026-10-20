---
title: "Apple simulator and UI-test tooling (simctl, xcodebuild, XCTest)"
type: source
authors: ["Apple"]
year: 2026
raw: "web"
status: digested
tags: [ios, simulator, xctest, tooling]
---

# Apple simulator and UI-test tooling (simctl, xcodebuild, XCTest)

**What:** the command-line help of Xcode 26.6's tools as installed (`xcrun simctl help`,
`xcrun simctl launch --help`, `xcodebuild -help`) and the XCTest interface
(`XCUIElement`, `XCUICoordinate`, `XCUIScreen`, `XCUIDevice`). Read 2026-10-15; not saved
(local tool output, Apple documentation at <https://developer.apple.com/documentation/xctest>).

## Facts used

- `xcodebuild -downloadPlatform iOS` installs the current simulator runtime;
  `simctl create <name> <device type> <runtime>` makes a device; `simctl boot` runs it
  without Simulator.app.
- `simctl launch --console-pty` connects the app's stdout; `SIMCTL_CHILD_<VAR>` sets `<VAR>`
  in the launched app's environment; `--wait-for-debugger` stops it before `main`;
  `simctl get_app_container <device> <bundle> data` gives its data container;
  `simctl io <device> screenshot <file>` captures the screen.
- `TEST_RUNNER_<VAR>` passes `<VAR>` to the UI-test runner process; `-resultBundlePath` keeps
  the `.xcresult`; `xcrun xcresulttool get test-results tests` and `export attachments` read it.
- XCTest synthesises touches: `XCUICoordinate.press(forDuration:thenDragTo:…)`,
  `XCUIElement.pinch(withScale:velocity:)`, `rotate(_:withVelocity:)`, `doubleTap()`,
  `twoFingerTap()`; `XCUIDevice.shared.orientation` rotates the device. Element queries need
  an accessibility snapshot of the app, which times out when the main thread is blocked.

Used by [iOS and iPadOS on the simulator: setup, checks, what broke](../platform/ios-simulator.md).
