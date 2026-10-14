---
title: "Rust core to Swift: UniFFI and the XCFramework pipeline"
type: platform
status: draft
sources: ["[[sources/uniffi-user-guide]]"]
tags: [rust, swift, ffi, uniffi, xcframework, build, ios, macos]
---

# Rust core to Swift: UniFFI and the XCFramework pipeline

How the Rust workspace of [[decisions/rust-core]] reaches the Swift UI on macOS and
iOS, what a call across the boundary costs, and the rules that follow. All numbers
are our own measurements (measured 2026-10-14, M1 Pro, release builds, Xcode 26.6,
rustc 1.99, UniFFI 0.32).

## Pipeline

```text
crates/almondbread-math   pure Rust maths (no platform code, forbid(unsafe))
crates/almondbread-core   platform-neutral core (re-exports math; grows in R2)
crates/almondbread-render R2: backend trait, scheduler, Metal backend (skeleton)
crates/almondbread-ffi    #[uniffi::export] surface; crate-type lib + staticlib + cdylib
crates/uniffi-bindgen     the bindings generator as a workspace binary
        |
scripts/build-rust-xcframework.sh   (make rust)
  1. cargo build --release -p almondbread-ffi --target
       aarch64-apple-darwin, aarch64-apple-ios, aarch64-apple-ios-sim
     with MACOSX_DEPLOYMENT_TARGET=14.0, IPHONEOS_DEPLOYMENT_TARGET=17.0
  2. uniffi-bindgen generate --library <host libalmondbread_ffi.dylib> --language swift
       -> almondbread_ffi.swift, almondbread_ffiFFI.h, almondbread_ffiFFI.modulemap
  3. headers dir = the .h + the modulemap renamed module.modulemap
  4. xcodebuild -create-xcframework (-library <.a> -headers <dir>) x 3
       -> build/rust/AlmondbreadCoreFFI.xcframework  (swapped in atomically,
          rebuilt only when a library or the header hash changes)
  5. almondbread_ffi.swift -> Sources/AlmondbreadCore/Generated/ (only if changed)
        |
Package.swift
  .binaryTarget AlmondbreadCoreFFI  (path build/rust/AlmondbreadCoreFFI.xcframework)
  .target AlmondbreadCore           (generated bindings + thin helpers)
  .executableTarget almondbread-core-check
project.yml: both app targets depend on the AlmondbreadCore product.
```

- UniFFI runs in proc-macro mode: `uniffi::setup_scaffolding!()` plus
  `#[uniffi::export]` functions and `uniffi::Record` / `Object` / `Enum` / `Error`
  types; no UDL file. Library mode reads the interface from metadata embedded in the
  compiled library (src: [[sources/uniffi-user-guide]]), so the host dylib is enough
  to generate bindings for all slices.
- The binary target's name must equal the XCFramework's file name, so the
  framework is `AlmondbreadCoreFFI` and the Swift wrapper module `AlmondbreadCore`.
  The C module Swift imports is `almondbread_ffiFFI` (from the generated modulemap).
- `make build`, `make run`, `make app` and `make ios-sim` depend on `make rust`
  (a no-op rebuild costs ~3 s); `make rust-check` runs `cargo test`, clippy with
  `-D warnings` and `cargo fmt --check`; `make core-check` runs the parity check.
- Outputs are gitignored: `/target/`, `/build/`,
  `Sources/AlmondbreadCore/Generated/`.
- The release static library is ~30 MB (all of std plus line tables); the linker
  dead-strips it. The macOS app binary was 3.8 MB with the core linked.
- Release profile: `lto = "fat"`, `codegen-units = 1`. `panic` stays `unwind`:
  UniFFI catches panics at the boundary; with `abort` any failed assertion would
  kill the app. Throwing exports surface a panic as a Swift error, non-throwing
  ones still crash, so exports validate their arguments and return `CoreError`.
- `cargo check` of math, core, render and ffi passes for aarch64-apple-darwin,
  aarch64-apple-ios, aarch64-apple-ios-sim, x86_64-unknown-linux-gnu,
  x86_64-unknown-linux-musl (cdylib dropped there, static only) and
  x86_64-pc-windows-msvc.

## The R1 interface

| Export | Purpose |
| --- | --- |
| `core_version()` | "almondbread-core 0.1.0" |
| `ping(x)` | x + 1; measures the bare call cost |
| `normalize_coordinate(re, im, bits, digits?)` | correctly rounded parse + format |
| `recommended_precision_bits(spacing_log2, size)` | precision rule ([[techniques/reference-orbit-precision]]) |
| `select_reference(re, im, radius, max_iter, bits, method, cancel?)` | period + Newton nucleus or centre ([[techniques/reference-selection]]) |
| `compute_reference_orbit(re, im, bits, max_iter, bailout, cancel?)` | returns a `ReferenceOrbit` object owned by Rust |
| `ReferenceOrbit.copy_extended_to(address, capacity)` | one copy of the floatexp points into caller memory |
| `ReferenceOrbit.copy_points_to`, `points_interleaved()`, `final_z(digits?)` | float32 points, the full-precision last point |
| `CancelToken` (`cancel`, `is_cancelled`, `progress`) | an `AtomicBool` and a progress counter shared with Swift |
| `float_exp_complex_layout()` | size and alignment as Rust lays the GPU struct out |

`FloatExpComplex` is `#[repr(C, align(8))] { mantissa: [f32; 2], exponent: i32 }`:
16-byte stride, 8-byte alignment, the MSL `struct { float2 m; int e; }`
([[techniques/extended-range-floats]]). A compile-time assertion pins it in Rust and
`almondbread-core-check` compares it with Swift's `MemoryLayout` at run time.

## Measured call costs

`almondbread-core-check` (best of 5 rounds):

| Call | Cost |
| --- | ---: |
| `ping(u64) -> u64` | 3.6 ns |
| `core_version() -> String` (string lifted into Swift) | 66 ns |
| `compute_reference_orbit`, 1 iteration at 64 bits (two decimal parses, object handle) | 1.07 µs (Swift-only: 2.23 µs) |
| `copy_extended_to`, 1 M points (16 MB) | 0.34 ms (47 GB/s) |
| `points_interleaved() -> Vec<f32>`, 1 M points (8 MB) | 572 ms |

A scalar call costs a few nanoseconds: a frame's handful of control calls is free.
Bulk data is the trap: UniFFI serialises a `Vec<f32>` through a `RustBuffer`
(src: [[sources/uniffi-user-guide]]) and the generated Swift reads it back element by
element, about 1700 times slower than a memcpy.

## Rules

- Nothing per pixel or per iteration crosses the boundary ([[decisions/rust-core]]).
  A frame makes a handful of calls.
- Bulk data never travels as a UniFFI `Vec`: Rust keeps it in an object and copies it
  once into caller memory (`copy_extended_to(address, capacity)`; in R2 the Metal
  buffers are owned by Rust and nothing is copied at all). The one `unsafe` block in
  the FFI crate is this copy; the address, capacity and alignment contract is on the
  export, and the Swift helper `ReferenceOrbit.copyExtended(into:)` checks alignment.
- Long computations (orbits, reference selection) block: call them off the main
  thread, cancel with a `CancelToken` from any thread (polled every 4096
  iterations), read `progress()` for the UI.
- Coordinates cross as decimal strings, exact at their precision: no binary
  BigFloat format leaks into Swift.
- Bit-for-bit parity is checked through the real bindings: `make core-check`
  compares formatted coordinates, five reference orbits (final Z, every float32 and
  floatexp point), reference selection and cancellation with AlmondbreadMath.

## Open for R2

- Rust owns the `MTLDevice`, queues and buffers (`objc2-metal`); Swift hands over the
  `CAMetalLayer` once. Check how the layer pointer crosses UniFFI (raw `u64` address
  as here, or a small C ABI function).
- Callbacks from Rust into Swift (frame finished, progress) via UniFFI callback
  interfaces or foreign traits; measure their cost like the calls above.
- An async export (UniFFI supports `async fn`) instead of blocking calls on a
  Swift background queue.
