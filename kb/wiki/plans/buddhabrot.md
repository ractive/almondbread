---
title: "Buddhabrot and Nebulabrot as a look"
type: plan
milestone: BB
status: planned
priority: 9
effort: M
appeal: high
audience: [casual, artists, education]
depth: "shallow; Metropolis–Hastings sampling helps close-ups"
realtime-ipad: "no"
reuse: "low–medium: the orbit overlay's orbits, progressive display, palettes and tone mapping"
depends-on: []
risks:
  - "slow convergence: noisy for seconds to minutes"
  - "iPad thermals under a long accumulation"
  - "atomic adds into a texture contend on hot pixels"
sources: ["[[synthesis/other-fractal-types]]", "[[sources/superliminal-buddhabrot]]", "[[decisions/controls]]", "[[techniques/frame-reuse-and-progressive]]"]
tags: [buddhabrot, nebulabrot, look, accumulation, optional]
created: 2026-10-19
updated: 2026-10-19
---

# Buddhabrot and Nebulabrot as a look

Status: planned as an optional look, independent of the formula track: it can be done at any
time ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5, item 3).

## Goal

Show the Mandelbrot set as the famous Buddhabrot: a histogram of where escaping orbits go.
Nebulabrot maps three iteration limits to RGB (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)). It is
the same orbits the orbit overlay draws, accumulated ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Orbit overlay").

## Scope

- A look "Buddhabrot" (and "Nebulabrot") for still views, shallow (whole set to ~1e-3).
- GPU sampling of `c` (uniform, then Metropolis–Hastings for close-ups, Boswell's method,
  src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)), orbit iteration, atomic adds into a histogram
  texture, progressive display with tone mapping.
- Accumulation runs while the view rests; any motion clears and restarts at low resolution.
- Export as PNG at full quality (long accumulation).

## Non-goals

- Deep zoom: "not possible to zoom arbitrarily deeply" (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)).
- Real-time flight with the look on (the view falls back to the normal look while moving).
- Anti-Buddhabrot and other variants (later, if wanted).

## Design sketch

- New kernel `buddha_accumulate`: each thread samples `c`, iterates up to the limit, and if the
  orbit escapes, iterates again and adds 1 to the histogram bin of each `z_n` in the view.
  Rejecting `c` in the main cardioid and the period-2 bulb with the closed-form test saves most
  of the non-escaping work.
- Metropolis–Hastings: mutate `c` near samples whose orbits hit the view; needed for views much
  smaller than the set.
- Display: tone-map the histogram (log or power curve), palette from the current look; the
  progressive display and history code show it like a refining frame
  ([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)).

## Affected modules

- `almondbread-render/src/shaders/mandelbrot.metal` (new kernels, not touching `iterate`),
  `metal/mod.rs`, `scheduler.rs` (a rest-only job kind), `host.rs`.
- `almondbread-core/src/settings.rs` (look), `palette.rs` (tone map), `oracle.rs` (a CPU
  Buddhabrot for tests).
- FFI and Swift: look picker entry, a progress readout.

## Verification

- CPU Buddhabrot vs GPU histogram at a small size and fixed seed: same distribution within
  sampling noise (chi-square or relative L1 below a bound).
- Stills at the whole set, seahorse valley and a 1e-3 view, looked at; time to a clean image
  on the Mac and on the iPad (thermal state logged).
- Mandelbrot fast path unaffected: `iterate` unchanged; bench A/B ≈ 1.0 with the look off.
- `make interaction-test`: motion with the look selected falls back without stalls.

## Review gate

The user looks at Buddhabrot and Nebulabrot on the Mac and the iPad. Decide whether it is a look
in the picker, a tour step ("where the orbits go"), or both.

## Risks

- Noise for a long time at large sizes; set expectations with a progress line.
- iPad thermals: the device policy already lowers GPU budgets at serious thermal state
  ([iOS and iPadOS on a device: first run, signing, memory, thermal, background](../platform/ios-device.md)).
- Atomic contention on bright bins; measure, consider per-threadgroup bins.

## Open questions

- Does the look keep the current palette, or a dedicated nebula colour map?
- Worth adding the orbit overlay's "orbit cloud" as a mini Buddhabrot of one `c`?

## Effort breakdown

M overall (1–2 weeks).

- Kernels, histogram, tone map: 3 days.
- Metropolis–Hastings sampling: 2 days.
- Scheduler job kind, UI, export: 2 days.
- Verification, iPad thermals: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
- [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)
