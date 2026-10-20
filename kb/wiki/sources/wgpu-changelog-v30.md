---
title: "wgpu CHANGELOG (v27–v30) and Features docs"
type: source
authors: ["gfx-rs contributors"]
year: 2026
raw: "web"
status: skimmed
tags: [wgpu, gpu, rust, portability]
---

# wgpu CHANGELOG (v27–v30) and Features docs

**URL:** <https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md> · <https://docs.rs/wgpu/latest/wgpu/struct.Features.html>

## What it is

The release notes and feature reference of wgpu, the Rust WebGPU implementation over Vulkan, DX12, Metal and GL.

## What it covers

- Majors v27 (2025-10), v28 (2025-12), v29 (2026-03), v30 (2026-07); 30.0.1 on 2026-08-21: about one breaking release per quarter.
- `SUBGROUP` on Vulkan/DX12/Metal; indirect dispatch renamed `dispatch_workgroups_indirect` (v29); pass timestamps everywhere, inside-pass timestamps not on Apple GPUs; `IMMEDIATES` (ex push constants, v28); `PASSTHROUGH_SHADERS` stable since v29; v30 surface colour-space selection.
- Metal backend sets no math mode (no fast-math control), per wgpu-hal `metal/device.rs`.

## Reliability

Primary project documentation.

## Used by

Game-edition research notes (kept private, not in this wiki).

# Citations

- <https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md>
- <https://docs.rs/wgpu/latest/wgpu/struct.Features.html>
