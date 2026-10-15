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

## Partial answer (M7, measured)

Measured 2026-10-15, M1 Pro, macOS 26.5, MSL 3.0, runtime compilation
(`newLibraryWithSource`), the float-float known-answer test of `almondbread precision --kat 1`
([[techniques/double-float]] "As built (M7)", [[synthesis/benchmarks]] M7):

- **`mathMode = .fast` breaks the error-free transforms**: TwoSum(1, 1e-8) and
  QuickTwoSum(1, 2^-30) return a low part of 0, and a 1000-step float-float orbit is only
  float32-accurate (4.5e-8 against f64). TwoProd through an explicit `fma(a, b, -p)` keeps its
  exact low part.
- **`mathMode = .safe` keeps them all** (TwoSum low part 1e-8 exactly, orbit 9.4e-16).
- `setMathMode:` exists on this OS (checked with `respondsToSelector:`); the deployment
  targets (macOS 14, iOS 17) predate it, so the code falls back to
  `fastMathEnabled = false` there (untested on such an OS).
- Still open: the default mode when nothing is set, and `.relaxed`. The float32 library sets
  fast math explicitly and the float-float library `.safe`, so neither depends on the default.
