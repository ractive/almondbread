---
title: "SwiftUI performance: the control center sidebar"
type: platform
status: draft
sources: ["[[sources/apple-swiftui-observation]]"]
tags: [swiftui, macos, ios, ui, performance, control-center]
---

# SwiftUI performance: the control center sidebar

What made the control center (`Sources/AlmondbreadKit/ControlCenterView.swift`, kb:
[Controls: keyboard, pointer and touch](../decisions/controls.md) "Control center") slow on the Mac, how it was measured, and what
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

## Fixes (first pass)

- **Sections as view structs** (`ControlSectionRows(section:model:)` and one struct per
  section). Their inputs are equal across parent updates, so SwiftUI skips their bodies when
  another section toggles or the query changes. Before, one toggle re-ran the palette grid
  and the Places grid as well.
- **Leaf views for fast reads** (`ViewReadout`, `MotionStatus`, `RotationRow`,
  `JourneyControls`, `CurrentIterations`, `DiagnosticsRows`, `OrbitReadout`, `PiMeterRows`).
  Change-only model properties for what buttons need (`autopilotActive`, and now
  `zoomSpeed`, as in the toolbar fix for tooltips).
- **`panelStats`: the stats for the sidebar at most 4 Hz** (removed in the second pass). A discrete change (autopilot,
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
  iOS simulator HEIF deadlock (kb: [iOS and iPadOS on the simulator: setup, checks, what broke](ios-simulator.md)). It takes 0.6-0.75 s in the
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
| section toggle, 5 heavy sections open | median 166 ms, max 388-430 ms | 75-410 ms by section (no clear change; fixed in the second pass below) |

## Resolved: no live values, a lazy container (second pass)

The first pass left the toggle cost and the ~0.4 s query rebuild (pitfall 2) and 23 % busy
while rendering. The user decided that **the sidebar holds only what can be set**, and that a
lazy container may replace the form if it looks close to it.

- **Live values out of the sidebar.** `ViewReadout` (centre, zoom, scale, zoom speed), the
  rotation angle, the autopilot / flight speed line, the journey's leg / phase / speed lines,
  `CurrentIterations`, the Diagnostics section, `OrbitReadout`, `MapNote` and the π meter
  readouts are gone. `panelStats` had no readers left and was deleted. Where they went: the
  HUD (`HUDView`, `ContentView.swift`) shows zoom, centre (cut to 34 characters; ⇧⌘C copies it
  in full) and the speed line; the iterations and everything Diagnostics had are the HUD's
  **developer details** (`HUDDeveloperDetails`), off by default: Overlays ▸ "Developer details
  in the HUD", View ▸ "Developer Details in Status", or the Settings window. The orbit's c and
  |zₙ|, the map note and the π reading were already drawn on the canvas by Rust. Buttons read
  change-only model properties (`autopilotActive`, `journeyActive`, and new `flightActive`,
  `journeySuspended`, `journeySeed`, `viewTurned`), assigned only when they change.
- **macOS: a `ScrollView` + `LazyVStack` instead of the grouped `Form`** (the `LazyVStack`
  became a plain `VStack` on 2026-10-17 after a second permanent hang; see the last section). Every row of every
  open section is its own child of the lazy stack: `ControlCardRows` splits a section's rows
  with `Group(subviews:)` (macOS 15) and draws each row's slice of the rounded card (corners on
  the first and last, a separator between rows). Tile grids (Places, palettes) are split into
  one stack row per grid row (`TileRows`; columns from the sidebar width, `controlRowTight`
  container value: no separator between them). A `LabeledContentStyle` (label leading, value
  trailing), a `ToggleStyle` (small switch trailing) and `ControlRows.picker` / `.color`
  (labelled row, menu at its own size) give the grouped-form look outside a `Form`. iOS keeps
  its `Form` (a collection view there, lazy already). macOS 14 falls back to one card per
  section.
- **Cheaper tiles**: only the selected thumbnail or swatch gets a border shape, and a
  thumbnail attaches its `.task` only while it has no image.
- `UIPerf` now counts every section body (`section.*`) and the HUD, and labels each toggle as
  `open-*` / `close-*` (with min, median and max).

Pitfall found on the way: in a `LazyVStack`, a custom `Layout` can be proposed an infinite
width (the stack's ideal-size pass). `TileGrid` divided by it and crashed
(`Int(+Inf)`); it now treats a non-finite width like nil.

### Before and after (same scene, 3 runs each, median and range)

Measured 2026-10-16, M1-class Mac, Release Xcode build, background window, machine shared with
other agents' builds. Scene: `--ui-demo=perf`; "4 open" is View, Look, Quality (and
Diagnostics before); the toggles have Places, View, Look and Quality open.

| Phase | Before (main 4d6bf96) | After |
| --- | --- | --- |
| rendering, sections open: main busy | 23 % (23-23) | 14 % (13-16) |
| rendering, all collapsed (baseline) | 13 % (10-14) | 16 % (15-16) |
| rendering, sidebar bodies in 3 s | 12 + 12 leaf bodies (readout, diagnostics) | 1 (`section.view`, the autopilot button) |
| rendering with Places open: main busy | 25 % (24-25) | 15 % (15-15) |
| toggle View | 375 ms (357-390) | 10 ms (8-13) |
| toggle Look | 308 ms (175-313) | 14 ms (11-17) |
| toggle Journey | 400 ms (356-406) | 13 ms (10-23) |
| toggle Lighting | 245 ms (109-253) | 11 ms (8-16) |
| toggle Interior | 231 ms (212-237) | 20 ms (11-20) |
| toggle Overlays | 179 ms (103-204) | 18 ms (15-24) |
| toggle Quality | 160 ms (144-170) | 14 ms (11-16) |
| toggle Controls | 82 ms (37-100) | 15 ms (11-17) |
| toggle Places (open or close, the slower) | 433 ms (392-513) | 84 ms (74-110) |
| Places alone (all else collapsed), the slower toggle | 121 ms (117-126) | 47 ms (36-57) |
| first open of Places | 506 ms (462-517) | 167 ms (115-187) |
| accordion phase: main busy | 49 % (46-51) | 5 % (4-5) |
| search rebuild (worst keystroke, idle) | 425 ms (394-488) | 78 ms (74-92) |
| ditto while rendering | 456 ms (415-468) | 67 ms (41-73) |
| typing while rendering: main busy | 52 % (52-53) | 22 % (19-22) |

Toggle rows: the slower of the two toggles per section, per run. The "rendering" busy share
with sections open is now at the collapsed baseline (the remaining 13-16 % is the renderer's
own main-thread work and the HUD at 10 Hz; both runs of the baseline are within the noise).

### Still above 50 ms

Opening or closing Places (74-110 ms) and the search rebuild (67-92 ms): both put ~40-70 new
tiles or swatches and AppKit controls on screen at once (closing Places reveals View, the
palette grid and Quality; clearing the search re-adds every section). `sample` shows the time
in `NSHostingView.layout` and `DisplayList.ViewUpdater` (making the platform views and
layers), not in bodies. The toggle animation makes no difference (measured with and without).
Per-row laziness only helps for rows below the fold. What is left to try: fewer views per
tile (one `Canvas` per grid row), or keeping a closed Places section's views alive.

## Hang: main-thread livelock with a picker menu open (2026-10-16)

**Superseded** by the fix in "Hang: opening and scrolling the sidebar" below (2026-10-17): the
sidebar is a plain `VStack` now, so the lazy-stack cycle described here cannot run. The
`--ui-demo=perf-menu` scene stayed on branch `worktree-agent-a3339d320584abaa6` (commit
`e45be94`), never merged; it is not in the app.

Reported 2026-10-16 (Release build after the lazy sidebar, a slow 1e3000 view rendering, a
palette just chosen): the app hung with a picker's menu open. Two `sample`s 30 s apart show the
same thing. The main thread is 100 % inside AppKit's menu tracking loop
(`NSPopUpButtonCell trackMouse` → `NSMenuTrackingSession startRunningMenuEventLoop`, run-loop
mode `NSEventTrackingRunLoopMode`), in SwiftUI's run-loop observer
(`NSHostingView.beginTransaction` → `GraphHost.flushTransactions`). Every pass:

- `Update.dispatchActions` → `LazyLayoutViewCache.signalPrefetch` →
  `NSHostingView.requestUpdate(after:)` → `setNeedsUpdate` (no delay: straight to the next
  run-loop pass);
- the next pass runs `LazyLayoutCacheItem.AllItemsPhaseMutation` → `updateItemPhases`, then
  `LazySubviewPlacements.placeSubviews` / `LazyStack.place` over the sidebar's `ForEach` nodes,
  with items inserted and removed (`initialPlacement` / `finalPlacement`), anchor translation
  (`makeAnchorTranslationIfNeeded`) and the root re-measured (`RootGeometry`);
- which signals prefetch again.

No view body of ours runs (no app frames on the main thread), the Rust threads are idle and the
display link does not fire: a pure SwiftUI lazy-stack prefetch / placement cycle that never
settles, and inside the menu loop nothing breaks it.

**Not reproduced** (measured 2026-10-16, M1-class Mac, Release Xcode build, background window).
`--ui-demo=perf-menu` (`UIPerf.swift`) runs the run loop in `NSEventTrackingRunLoopMode` from a
timer callout (`track`), which is what the menu loop does, with a watchdog that quits on a stuck
main thread. With palettes, looks and palette or place categories changed inside the tracking
loop and right before it, autopilot rendering, all sections open, sidebar widths 290–440 and
scroll positions every 9–40 pt over the whole sidebar, and a deep view with the HUD's developer
details: tracking mode stayed at the default-mode cost (0–1 % idle, 5–14 % rendering). About
40 real menus (`performClick` on the `NSPopUpButton`) did not reproduce it either; real menus
show over other apps, so window tests must never open one. Pitfall in the sweeps: a long
autopilot run grows the renderer's own main-thread cost (`FractalHostBase.step` → Rust
`frame`) to 90–97 % busy in the default mode too; compare every tracking phase with a
default-mode control phase.

Not a fix: a plain `VStack` instead of the `LazyVStack` removes the prefetch machinery, but
costs (same scene, one run each, lazy → VStack): section toggles 13–37 ms → 30–207 ms (Places 93 → 152), the
search rebuild 61 → 416 ms while rendering, typing while rendering 17 → 45 % busy. The missing
piece is the state that keeps the prefetch from settling (the user's sections, width, scroll
position, imported palettes, restored settings, a real 1e3000 reference). Next time it hangs:
`sample` it, and note the open sections, the scroll position and which menu was open.

## Hang: opening and scrolling the sidebar (2026-10-17)

The user opened the control center and scrolled down (Places, Overlays and Quality open,
window 1728 × 1083); the main thread never answered again (hang report
`hang-20261017-205937.txt`, no "recovered" line). The stack was pure SwiftUI graph update
(`NSHostingView.beginTransaction` → `GraphHost.flushTransactions` → `DynamicBody.updateValue`),
the renderer idle. The ring buffer showed the canvas insets growing 157 → 330 pt in 0.13 s:
in a key window AppKit animates the reveal and lays the sidebar out at every width on the way
(the buffer had dropped the widths below 157).

How it was measured: `--ui-demo=perf-open` (background window) opens the control center the
first time after launch and again, scrolls it top to bottom and back with continuous
(trackpad-phase) scroll events at 1200 pt/s, with the user's sections, all of them, only
Places, and with legacy scroll bars. A background window does not animate the reveal, so
`perf-open` replays it: the split view's divider stepped from 20 to 330 pt in 16 steps of
8 ms (the item's minimum lowered for the replay). `--ui-demo=perf-stress` runs 40 seeded
rounds of open, random scrolls (200-4000 pt/s, random distances and pauses) and close, with
the user's sections or all of them, 30 % of them while the canvas renders, in a full-screen
window; it prints every run-loop iteration over 150 ms with what the script was doing, and a
stall watchdog `sample`s the process and quits when the main thread is stuck for 4 s.

What we found (measured 2026-10-17, M1-class Mac, `swift build -c release`, background window,
other agents' builds running):

- **Not reproduced as a permanent hang** with the real code paths. Two stuck runs (main thread
  busy for good, `sample`d) came from the replayed reveal while the split item could still
  collapse: each `NSSplitViewItem setCollapsed:` spent 1.3-1.5 s in `NSHostingView.layout` of
  the lazy stack (`LazySubviewPlacements`, `ForEach` node enumeration, text measuring), and the
  sidebar width bounced 290 ↔ 330 pt dozens of times. Partly an artefact of the replay, but
  the same layout work that the reveal triggers.
- **Every width change re-laid the whole sidebar out**: 20-60 ms per step with the
  `LazyVStack` (100-300 ms with a plain `VStack`), and the tile grids rebuilt with new row
  identities whenever the column count changed (1 → 2 → 3 columns while the sidebar grows).
  After a reveal from 20 pt the lazy stack's document height jumped to 15 000-17 000 pt (rows
  measured at a few points wide) against ~2 700 pt settled; scrolling then corrected those
  estimates. Before: 63 iterations over 150 ms in the stress run, the worst 566 ms (opening with
  all sections; 704 ms in another run), 98 s main-thread busy.
- The same lazy-stack cycle as the menu hang above (prefetch / placement in
  `LazyLayoutViewCache`) is the only mechanism we have seen keep the main thread for good, and
  it cannot be reproduced in a background window. The place zoom pill also called into Rust
  from `PlacesSection.body` (`placeZoomOffset()`, the explorer's mutex): cheap and uncontended
  (no other thread takes that lock), but against the rule that a body never calls into Rust.

The fix (`ControlCenterView.swift`):

1. **The content is laid out at the sidebar's settled width** (`LayoutWidth`): it reports the
   width it is given but lays its content out at `layoutWidth`, clipped; `layoutWidth` follows
   the live width 150 ms after it stops changing (never below the 290 pt minimum). The reveal,
   the hide and a divider drag cost no content layouts; the tile columns change once.
2. **A plain `VStack` instead of the `LazyVStack`**: no prefetch / placement machinery that can
   fail to settle. Its cost (below) is the price of a sidebar that cannot hang that way.
3. **The pill's offset is cached in the model** (`placeZoomOffset`, set on each canvas resize,
   not observed: a live window resize does not redraw 79 tiles per frame).

| perf-open (user's sections unless noted) | Before (lazy) | After |
| --- | --- | --- |
| first open after launch: longest iteration | 77 ms | 40 ms |
| second open | 60 ms | 40 ms |
| open, all sections | 42-69 ms | 60-119 ms |
| scroll down / up 1200 pt/s: longest iteration | 31-38 ms (10-11 ms 2nd pass) | 3-5 ms |
| scroll, all sections: longest / main busy | 32-36 ms / 40-44 % | 5 ms / 17-20 % |
| one width step (the reveal) | 20-64 ms | 16 ms median |
| stress: iterations over 150 ms, worst | 63, 566 ms | 8, 444 ms (all on opens with every section freshly built) |
| stress: main busy over 40 rounds | 98 s | 51 s |

What the plain stack costs elsewhere (`--ui-demo=perf`, same build): opening a section
30-150 ms (Look with the palette grid the slowest, Places 67-105 ms) against 10-20 ms lazy;
closing 5-35 ms; the search rebuild when the query clears (every section back) 327-350 ms
against 67-92 ms. Rendering with Places open stays at the collapsed baseline (8 % busy). The
lazy stack with only the settled width (fix 1) measured in between: stress worst 281 ms,
scroll longest 25-40 ms. If the hang never returns with a `VStack`, the remaining cost to
attack is the palette grid and the Places grid (fewer views per tile).
