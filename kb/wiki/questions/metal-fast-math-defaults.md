---
title: "What does Metal's default math mode allow for runtime-compiled kernels?"
type: question
status: open
sources: ["[[sources/apple-metal-compile-options]]", "[[sources/luma-gl-gpu-precision]]"]
tags: [metal, fast-math, double-float, compiler]
---

# What does Metal's default math mode allow for runtime-compiled kernels?

## Question

When compiling with `makeLibrary(source:options:)`, what is the default
`MTLCompileOptions.mathMode`, and does `.relaxed` or `.fast` permit reassociation
or FMA contraction that would break TwoSum/TwoProd? From which OS versions do
`mathMode` and `mathFloatingPointFunctions` exist?

## Why it matters

Float-float code ([[techniques/double-float]]) silently collapses to float32 if the
compiler reassociates. The research notes assumed fast math is on by default
(unverified).

## What sources say

- `mathMode` (`.safe`, `.relaxed`, `.fast`) replaces `fastMathEnabled`;
  `mathFloatingPointFunctions` selects `.fast`/`.precise` library functions; the page
  did not render, so semantics are unverified (src: [[sources/apple-metal-compile-options]]).
- Error-free transforms need round-to-nearest, no reassociation and no unintended
  contraction; source-level tricks (temporaries, parentheses) are not guarantees
  (src: [[sources/luma-gl-gpu-precision]] §Why This Is Fragile In WGSL And Metal).

## How to resolve

1. Read the `MTLCompileOptions` and MSL spec text offline (Xcode documentation).
2. Startup known-answer test in each mode: TwoSum(1, 1e-8) low limb, and a
   TwoProd residual check, on real hardware. Record which modes preserve them.
3. Update [[platform/metal-compute]].
