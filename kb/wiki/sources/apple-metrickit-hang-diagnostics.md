---
title: "Apple: MetricKit MXHangDiagnostic and main-thread hangs"
type: source
authors: ["Apple"]
year: 2021
raw: "web"
status: skimmed
tags: [apple, metrickit, hangs, diagnostics, main-thread]
---

# Apple: MetricKit MXHangDiagnostic and main-thread hangs

**URL:** <https://developer.apple.com/documentation/metrickit/mxhangdiagnostic> ·
<https://developer.apple.com/documentation/xcode/understanding-hangs-in-your-app> ·
**Saved:** not saved (`raw: "web"`) · **Terms:** Apple developer documentation

## What it is

Apple's documentation of MetricKit's hang diagnostics and of app hangs in general. Read for the
hang watchdog ([Hang prevention (loop guards, extreme-value tests, main-thread watchdog)](../techniques/hang-prevention.md)); the points below are from memory of these pages
and were not re-checked line by line (unverified).

## What it covers

- `MXMetricManager.shared.add(subscriber)`; the subscriber's `didReceive(_: [MXDiagnosticPayload])`
  gets payloads at most once a day (immediately on iOS 15+ / macOS 12+ for diagnostics), each
  with `hangDiagnostics: [MXHangDiagnostic]?`: hang duration and a `callStackTree` (JSON, with
  binary UUIDs and offsets for offline symbolication). Available iOS 14+, macOS 12+.
- Apple calls a main run loop that does not service events for more than 250 ms a hang; the
  system's hang reporting on macOS (spindump, the beach ball) starts at about 2 s.
- Diagnostics come from the system's sampling, so they need no private API and work for App
  Store builds; they arrive late (next launch or next day), so they complement an in-app
  watchdog rather than replace it.

## Reliability

Primary source (Apple). Thresholds and delivery timing change between OS releases.
