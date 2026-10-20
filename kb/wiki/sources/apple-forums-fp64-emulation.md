---
title: "Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs"
type: source
authors: ["Philip Turner"]
year: 2022
raw: "web"
status: unread
tags: [fp64-emulation, double-float, apple-gpu, metal]
---

# Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs

**URL:** <https://developer.apple.com/forums/thread/718737> · **Saved:** not saved (`raw: "web"`) · **Terms:** forum post; copyright the posters

## What it is

An Apple Developer Forums thread on emulating 64-bit floating point on Apple GPUs, which have no FP64 hardware. Philip Turner (author of [metal-benchmarks README (Apple GPU microarchitecture)](metal-benchmarks-readme.md)) argues for integer-based emulation. Not saved or read for this wiki; the points below come from the ingest brief and are unverified. The year 2022 is inferred from the thread number and Turner's FP64 work at that time (unverified).

## What it covers

- Claim: integer-based FP64 emulation (integer mantissa arithmetic plus exponent handling) beats double-single (float-float) on Apple GPUs (unverified).
- His estimate: more than 27x for multiply and well over 4x for add, excluding exponent handling (unverified; the brief does not say whether these are slowdowns vs FP32 or advantages, so read the thread before using the numbers).
- Context: his README quotes emulated IEEE "FP59" (e11m48) at ADD 1:36, MUL 1:52, FMA 1:68 of the F32 rate (src: [metal-benchmarks README (Apple GPU microarchitecture)](metal-benchmarks-readme.md) §Operations per Second). These ratios and the forum numbers cannot be reconciled until the thread is read.

## Reliability

Secondary opinion with estimates, not measurements, and status `unread`. The claim that integer emulation beats double-single conflicts with the usual view that float-float is the cheap option when ~48 bits and f32 range are enough (src: [luma.gl: GPU floating-point precision techniques](luma-gl-gpu-precision.md) §The Main Alternatives). The two compare different products (full FP64 range vs 48-bit significand at f32 range). Benchmark before deciding ([Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)).

## Useful sections

None recorded (not read).

## Used by

- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)
- [Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)

# Citations

- <https://developer.apple.com/forums/thread/718737>
- [metal-benchmarks README (Apple GPU microarchitecture)](metal-benchmarks-readme.md)
- [luma.gl: GPU floating-point precision techniques](luma-gl-gpu-precision.md)
