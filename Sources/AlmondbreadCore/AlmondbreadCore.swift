// AlmondbreadCore: Swift face of the Rust core (crates/almondbread-ffi, kb: decisions/rust-core).
//
// Generated/almondbread_ffi.swift holds the UniFFI bindings; it is written by
// scripts/build-rust-xcframework.sh (`make rust`) together with
// build/rust/AlmondbreadCoreFFI.xcframework and must not be edited. This file adds only thin,
// allocation-free conveniences. Rule: nothing per pixel or per iteration crosses the FFI.

public enum AlmondbreadCoreLayout {
    /// Byte stride of one extended-range orbit point (MSL `struct { float2 m; int e; }`).
    public static let floatExpComplexStride = 16
    /// Byte stride of one float32 orbit point (`float2`).
    public static let pointStride = 8
}

// The raw `copyExtendedTo` / `copyPointsTo` exports take an address and a capacity that Rust
// cannot check for liveness (the FFI's one unsafe contract, crates/almondbread-ffi `copy_to`);
// call them only through these wrappers, which derive both from one buffer. A misaligned
// buffer throws `CoreError.InvalidArgument` (it used to trap).
extension ReferenceOrbit {
    /// Copies the extended-range points into `buffer` (16 bytes each, 8-byte aligned), e.g. an
    /// `MTLBuffer`'s contents. Returns the number of points copied.
    public func copyExtended(into buffer: UnsafeMutableRawBufferPointer) throws -> Int {
        guard let base = buffer.baseAddress else { return 0 }
        let capacity = UInt64(buffer.count / AlmondbreadCoreLayout.floatExpComplexStride)
        return Int(try copyExtendedTo(address: UInt64(UInt(bitPattern: base)), capacity: capacity))
    }

    /// Copies the float32 points (8 bytes each, 4-byte aligned) into `buffer`. Returns the
    /// number copied.
    public func copyPoints(into buffer: UnsafeMutableRawBufferPointer) throws -> Int {
        guard let base = buffer.baseAddress else { return 0 }
        let capacity = UInt64(buffer.count / AlmondbreadCoreLayout.pointStride)
        return Int(try copyPointsTo(address: UInt64(UInt(bitPattern: base)), capacity: capacity))
    }
}
