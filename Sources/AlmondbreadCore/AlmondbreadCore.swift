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

extension ReferenceOrbit {
    /// Copies the extended-range points into `buffer` (16 bytes each, 8-byte aligned), e.g. an
    /// `MTLBuffer`'s contents. Returns the number of points copied.
    public func copyExtended(into buffer: UnsafeMutableRawBufferPointer) -> Int {
        guard let base = buffer.baseAddress else { return 0 }
        precondition(Int(bitPattern: base) % 8 == 0, "FloatExpComplex buffer must be 8-byte aligned")
        let capacity = UInt64(buffer.count / AlmondbreadCoreLayout.floatExpComplexStride)
        return Int(copyExtendedTo(address: UInt64(UInt(bitPattern: base)), capacity: capacity))
    }

    /// Copies the float32 points (8 bytes each) into `buffer`. Returns the number copied.
    public func copyPoints(into buffer: UnsafeMutableRawBufferPointer) -> Int {
        guard let base = buffer.baseAddress else { return 0 }
        precondition(Int(bitPattern: base) % 4 == 0, "point buffer must be 4-byte aligned")
        let capacity = UInt64(buffer.count / AlmondbreadCoreLayout.pointStride)
        return Int(copyPointsTo(address: UInt64(UInt(bitPattern: base)), capacity: capacity))
    }
}
