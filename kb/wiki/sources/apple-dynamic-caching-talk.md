---
title: "Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)"
type: source
authors: ["Apple"]
year: 2023
raw: "web"
status: unread
tags: [apple-gpu, dynamic-caching, occupancy, registers, m3, m5]
---

# Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)

**URL:** <https://developer.apple.com/videos/play/tech-talks/111375> · **Saved:** not saved (`raw: "web"`) · **Terms:** Apple developer video; not redistributable

## What it is

An Apple developer tech talk (video) on the GPU in M3 and A17 Pro, mainly Dynamic Caching. A later talk on the M5 GPU (<https://developer.apple.com/kr/videos/play/tech-talks/111431/>) covers the second generation. Neither video was watched or transcribed for this wiki. The points below come from the ingest brief's research notes and are unverified against the videos.

## What it covers

- Dynamic Caching (M3 / A17 Pro): registers are allocated by what a shader actually uses while it runs, not by the worst case the compiler reserves for the whole kernel (unverified).
- Registers, threadgroup memory and tile memory share one on-chip pool, split dynamically (unverified).
- Works without app changes; existing shaders gain occupancy automatically (unverified).
- Second-generation talk (M5): warns that running too many SIMD-groups can cause spilling, so occupancy is not free (unverified).
- Consequence for almondbread: the M1/M2 occupancy and register-pressure numbers in [metal-benchmarks README (Apple GPU microarchitecture)](metal-benchmarks-readme.md) (fixed ~208 KB register file, saturation at 24 SIMDs/core) may not carry over to M3 and later (inference).

## Reliability

Primary vendor source, but not read here: status `unread`. Watch both talks and record exact claims and timestamps before citing this page for anything beyond "Dynamic Caching exists".

## Useful sections

None recorded yet (video; no headings captured).

## Used by

- [Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)
- [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)

# Citations

- <https://developer.apple.com/videos/play/tech-talks/111375>
- <https://developer.apple.com/kr/videos/play/tech-talks/111431/>
- [metal-benchmarks README (Apple GPU microarchitecture)](metal-benchmarks-readme.md)
