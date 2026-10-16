---
title: "SwiftUI performance: the control center sidebar"
type: platform
status: draft
sources: ["[[sources/apple-swiftui-observation]]"]
tags: [swiftui, macos, ios, ui, performance, control-center]
---

# SwiftUI performance: the control center sidebar

What made the control center (`Sources/AlmondbreadKit/ControlCenterView.swift`, kb:
[[decisions/controls]] "Control center") slow on the Mac, how it was measured, and what
helped. All numbers are our own measurements (measured 2026-10-16, M1-class Mac, Release
Xcode build, background window, other agents loading the machine, so expect ±30 % noise).

## How to measure

`Almondbread --ui-demo=perf` (`UIPerf.swift`) runs a scripted session in a background window
and prints `ui-perf:` lines, then quits:

- body evaluations per view type (`UIPerf.body("name")` counters in the bodies);
- main-thread busy time per phase: run-loop observers, `afterWaiting` to `beforeWaiting` at
  the highest order (after SwiftUI's update and the Core Animation commit), plus the longest
  single iteration (the stutter);
- latency of each scripted keystroke and section toggle: from the action to the end of that
  run-loop iteration;
- thumbnails: how many had their image in their first body, and when the last one showed.

Phases: idle, rendering (autopilot on, `stats` change at 10 Hz), opening Places, each section
alone while rendering, every section toggled with others open and alone, typing "palette" and
"zoom" at 80 ms per key (idle and while rendering). `sample <pid>` during a phase gives the
call tree. The initial section state comes from a volatile defaults suite, so the scene
neither depends on nor changes the user's sidebar.

## Pitfalls found

1. **`ForEach` row closures track observation too.** The form's sections were computed
   properties of one view; the `ControlCenterView` body ran once, but the rows of every open
   section that read `model.stats` (View, Quality, Diagnostics) re-ran at 10 Hz, 30 times in
   3 s. The main thread went from 8-10 % busy (all collapsed) to 43 % while rendering. A
   body counter on the outer view alone shows nothing: count inside the row content as well.
2. **On macOS a grouped `Form` is a non-lazy SwiftUI stack in a scroll view.** The samples
   show `ScrollViewLayoutComputer`, `GroupedFormRowLayout`, `_GroupedFormPickerLayout`
   (measuring `NSPopUpButton`) and `NSHostingView.invalidateSizeConstraintsIfNecessary`, not
   an `NSTableView`. Any change in any row lays out every open row again. Even a small leaf
   row that changes costs ~10 ms per update with one section open, and a section toggle costs
   20-200 ms on its own but 60-450 ms with Places, View, Look, Quality and Diagnostics open.
   The cost grows with how much is open, not with what changed. Neither tooltips (`.help` on
   ~140 grid cells) nor the toggle animation made a measurable difference.
3. **Typing into a field whose text is `@State` of the whole form** re-runs the form body per
   keystroke, and the first character opens every matching section. Before: keystroke median
   25 ms, but the worst keystroke 0.6-0.9 s, and the main thread 81-85 % busy while typing.
4. **Decoding thumbnails on demand shows a loading state.** Each `PlaceThumbnail` decoded its
   640×400 HEIC on first appearance, so the grid filled in over 733 ms. All of them decoded
   took ~71 MB (1 MB of bitmap each).

## Fixes

- **Sections as view structs** (`ControlSectionRows(section:model:)` and one struct per
  section). Their inputs are equal across parent updates, so SwiftUI skips their bodies when
  another section toggles or the query changes. Before, one toggle re-ran the palette grid
  and the Places grid as well.
- **Leaf views for fast reads** (`ViewReadout`, `MotionStatus`, `RotationRow`,
  `JourneyControls`, `CurrentIterations`, `DiagnosticsRows`, `OrbitReadout`, `PiMeterRows`).
  Change-only model properties for what buttons need (`autopilotActive`, and now
  `zoomSpeed`, as in the toolbar fix for tooltips).
- **`panelStats`: the stats for the sidebar at most 4 Hz.** A discrete change (autopilot,
  flight or journey on or off, journey paused, view settled) passes at once, and a trailing
  update makes sure the latest value lands. The HUD keeps the 10 Hz `stats`. Because of
  pitfall 2 the update rate itself is the cost: rendering with View, Look, Quality and
  Diagnostics open went from 43 % to 23 % main-thread busy (37-40 % with the leaves at 10 Hz).
- **Search field owns its text** (`ControlSearchField`); the form filters on a copy of it,
  applied 120 ms after the last keystroke (clearing applies at once). While typing, only the
  field updates, and the form rebuilds once per pause instead of once per key (4-5 form
  bodies instead of 22 for 22 keystrokes). The one rebuild still costs ~0.4 s when "palette"
  opens Look & colour with Places open (pitfall 2). Moving the field out of the
  `safeAreaInset` changed nothing (~15-25 ms per keystroke either way).
- **Thumbnails pre-decoded at launch** (`PlaceThumbnails.preload()` from
  `ExplorerModel.attach`). One utility-QoS serial queue decodes all 71 at display size:
  140 pt × the largest screen scale on the Mac (280 px from the 320 px file at 2×), 120 pt on
  iOS. It uses ImageIO thumbnailing into plain sRGB bitmaps; full bitmaps still avoid the
  iOS simulator HEIF deadlock (kb: [[platform/ios-simulator]]). It takes 0.6-0.75 s in the
  background at launch (3 s on a loaded machine) and uses 13.3 MB instead of ~71 MB. When
  Places first opens, all 71 have their image in their first body and the grid shows complete
  in its first frame (51 ms after the section appears, against 733 ms before).
- **`TileGrid` layout instead of `LazyVGrid`** for the place and palette grids. Its size is
  computed from the width and the count (all tiles have the same shape), so measuring the
  grid never asks its tiles. Inside the non-lazy form the lazy grid re-estimated its tiles on
  every layout pass (`LazyStack.measureEstimates` in the samples). No measurable time change,
  but all 71 thumbnails exist at once (the lazy grid made 45 and added the rest on scroll).

## Before and after (same scene, same machine)

| Phase | Before | After |
| --- | --- | --- |
| rendering, 4 sections open: main busy | 43-44 % | 23 % |
| ditto, row re-evaluations in 3 s | 30 per section | 12-13 per leaf |
| typing 80 ms/key: worst keystroke | 611-891 ms | ~13-25 ms (field only) |
| typing: form rebuilds per 22 keys | 22 | 4-5 (one per pause, ~0.4 s each) |
| typing: main busy | 81-85 % | 43-51 % (incl. the paused rebuilds) |
| Places first open: thumbnails with image at first body | 0 / 45 | 71 / 71 |
| Places first open: last thumbnail visible | 733 ms | 51 ms |
| thumbnail memory | ~71 MB (all decoded) | 13.3 MB |
| section toggle, 5 heavy sections open | median 166 ms, max 388-430 ms | 75-410 ms by section (no clear change) |

## Open

The toggle cost and the ~0.4 s query rebuild come from pitfall 2. Removing it needs a lazy
container (a `List`, or a `ScrollView` + `LazyVStack` styled like the grouped form). That
changes how the sidebar looks, so it is a decision for the user, not part of this fix.
