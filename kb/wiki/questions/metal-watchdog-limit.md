---
title: "What is the exact Metal GPU watchdog limit on macOS and iOS?"
type: question
status: open
sources: ["[[sources/mlx-watchdog-issue]]", "[[sources/fraktaler-3-site]]"]
tags: [metal, watchdog, ios, scheduling]
---

# What is the exact Metal GPU watchdog limit on macOS and iOS?

## Question

How long may one Metal command buffer run before the OS kills it, on macOS (with
and without an active display, on battery) and on iOS/iPadOS? Do waits count? How
many timeouts before the device is revoked?

## Why it matters

It bounds the work per command buffer in the chunked scheduler
([[platform/gpu-watchdog-and-chunking]]) and decides how cautious the iOS port
must be ([[platform/ios-ipad-considerations]]).

## What sources say

- Community report: "~5 seconds", per command buffer not per dispatch; error
  `kIOGPUCommandBufferCallbackErrorImpactingInteractivity`; a separate trigger when
  a display is active (MLX #3267); `AGX_RELAX_CDM_CTXSTORE_TIMEOUT=1` disables it
  system-wide (src: [[sources/mlx-watchdog-issue]]).
- Fraktaler 3 warns that big tiles or large step limits cause OS GPU timeouts with
  OpenCL, without giving a number (src: [[sources/fraktaler-3-site]]).
- No Apple document with the threshold was found.

## How to resolve

The design does not depend on the number: target 10 to 30 ms per command buffer.
Still worth measuring once: a test kernel that spins for a configurable time per
command buffer, stepping 1 s, 2 s, 4 s, 8 s, recording the error code, on macOS
with the display on/off and on an iPad. Record date, device and OS version here.

## Observations

- Measured 2026-10-14, M1 Pro, macOS 26 (Darwin 25.5), release: a single compute
  command buffer of **10.4 s** (milestone-1 single-dispatch kernel, feigenbaum view at
  3456×2234, 200 000 iterations) completed without error from a headless command-line
  process, with the built-in display on and the user's session active. So the limit
  for a background, non-presenting process is above 10 s on this machine, contradicting
  a flat "~5 s" (src: [[sources/mlx-watchdog-issue]]); the reported trigger may need
  the process to be presenting or the display to be starved. Not probed further: longer
  stalls freeze the user's display. Still open for presenting apps and for iOS.
- almondbread's chunked scheduler keeps command buffers at ~16 ms (p99 17.6 ms over a
  1e7-iteration render), so the question no longer constrains the design
  ([[platform/gpu-watchdog-and-chunking]], [[synthesis/benchmarks]]).
