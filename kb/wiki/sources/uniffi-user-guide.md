---
title: "The UniFFI user guide (Mozilla)"
type: source
authors: ["Mozilla UniFFI contributors"]
year: 2026
raw: "web"
status: skimmed
tags: [rust, swift, ffi, uniffi, build]
---

# The UniFFI user guide (Mozilla)

**URL:** <https://mozilla.github.io/uniffi-rs/latest/> · **Saved:** not saved (web) ·
**Terms:** MPL-2.0 project documentation

## What it is

The official guide to UniFFI, Mozilla's generator of foreign-language bindings
(Kotlin, Swift, Python, Ruby) for a Rust library. Read for version 0.32, which
almondbread uses ([Rust core to Swift: UniFFI and the XCFramework pipeline](../platform/rust-swift-ffi.md)).

## What it covers

- Two ways to declare the interface: a UDL file, or proc macros on the Rust code
  (`#[uniffi::export]` on functions and `impl` blocks, derives `uniffi::Record`,
  `uniffi::Object`, `uniffi::Enum`, `uniffi::Error`, plus `uniffi::setup_scaffolding!()`
  once per crate).
- Records are passed by value (serialised); objects are reference-counted Rust values
  (`Arc<T>`) handed out as opaque handles; `Result<T, E>` becomes a throwing function
  in Swift; `async fn` exports become Swift `async` functions; callback interfaces
  and foreign traits let Rust call back into Swift.
- Compound types (strings, `Vec`, records, optionals) cross the boundary serialised
  in a `RustBuffer` and are read back by generated code.
- "Library mode": `uniffi-bindgen generate --library <compiled lib>` reads the
  interface from metadata embedded in the compiled library instead of a UDL file.
- Swift output: a `.swift` source file, a C header and a clang module map; the
  guide's Apple chapters show packaging the static libraries and header into an
  XCFramework for Xcode and SwiftPM.

## Reliability

Primary documentation of the tool; behaviour was confirmed by our own build
(measured 2026-10-14, M1 Pro). The cost of `Vec` serialisation is not quantified in
the guide; our measurement is on [Rust core to Swift: UniFFI and the XCFramework pipeline](../platform/rust-swift-ffi.md).

# Citations

- <https://mozilla.github.io/uniffi-rs/latest/>
