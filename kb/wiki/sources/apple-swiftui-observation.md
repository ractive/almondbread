---
title: "Apple SwiftUI and Observation documentation, and the macOS 26 SwiftUI call stacks"
type: source
authors: ["Apple"]
year: 2026
raw: "web"
status: digested
tags: [swiftui, observation, macos, tooling, performance]
---

# Apple SwiftUI and Observation documentation, and the macOS 26 SwiftUI call stacks

**What:** Apple's documentation of the Observation framework and SwiftUI view updates
(<https://developer.apple.com/documentation/observation>,
<https://developer.apple.com/documentation/swiftui/managing-model-data-in-your-app>). Also
`sample` call trees of our own app on macOS 26 with the Xcode 26 SDK, taken 2026-10-16 and not
saved (local tool output). They show which SwiftUI types do the layout. Read 2026-10-16.

## Facts used

- With `@Observable` a view depends only on the properties its body reads while SwiftUI
  tracks it. A change to one of them re-runs that body, and nothing else does (docs).
- From the call trees: a macOS `Form` with `.formStyle(.grouped)` is laid out by SwiftUI
  itself (`ScrollViewLayoutComputer`, `GroupedFormRowLayout`, `_GroupedFormPickerLayout`
  measuring an `NSPopUpButton`). It does not use an `NSTableView` and is not lazy. The window's
  `NSHostingView` measures the content again for its size constraints
  (`invalidateSizeConstraintsIfNecessary`).
- From the call trees: a `LazyVGrid` inside that form runs `LazyStack.measureEstimates` on
  layout passes.

Used by [SwiftUI performance: the control center sidebar](../platform/swiftui-performance.md).

# Citations

- <https://developer.apple.com/documentation/observation>
- <https://developer.apple.com/documentation/swiftui/managing-model-data-in-your-app>
