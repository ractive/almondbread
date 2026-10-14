---
title: "Apple docs: MTLCompileOptions mathMode"
type: source
authors: ["Apple"]
year: 2024
raw: "web"
status: unread
tags: [metal, fast-math, compiler, floating-point]
---

# Apple docs: MTLCompileOptions mathMode

**URL:** <https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode> · **Saved:** not saved (`raw: "web"`) · **Terms:** Apple developer documentation

## What it is

Apple's reference page for the `mathMode` property of `MTLCompileOptions`, the per-library setting for how aggressively the Metal compiler may optimise floating-point maths. The page did not render when fetched (JavaScript-only documentation), so nothing was saved. The points below come from the ingest brief and are unverified.

## What it covers

- `mathMode` takes `.safe`, `.relaxed` or `.fast` and replaces the older boolean `fastMathEnabled` (unverified).
- A separate `mathFloatingPointFunctions` property chooses `.fast` or `.precise` versions of library functions such as `sin`, `exp` (unverified).
- What each mode allows (reassociation, contraction into FMA, ignoring NaN/infinity/signed zero, flushing denormals) is not confirmed. That matters for double-float error-free transforms, which reassociation can silently zero (src: [[sources/luma-gl-gpu-precision]] §Why This Is Fragile In WGSL And Metal; [[sources/thall-extended-precision]] §II-D FMAD or Just Angry?).
- The year 2024 is a guess (the API is believed to be new in the 2024 SDKs) (unverified).

## Reliability

Primary vendor documentation, but not read: the semantics above are unverified. Before relying on them, read the page (or the Metal Shading Language spec section on fast math) and test a TwoSum kernel under each mode. Tracked in [[questions/metal-fast-math-defaults]].

## Useful sections

None recorded (page did not render).

## Used by

- [[platform/metal-compute]]
- [[techniques/double-float]]
- [[questions/metal-fast-math-defaults]]
- [[questions/apple-gpu-denormals]]
