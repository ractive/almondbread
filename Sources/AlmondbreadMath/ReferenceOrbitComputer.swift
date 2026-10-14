// High-precision reference orbit Z_{n+1} = Z_n^2 + C (kb: techniques/reference-orbit-precision).
//
// The loop runs on preallocated RawFloat registers (no allocation per iteration) and is
// specialised for 1...16 limbs (64...1024 bits): the limb count becomes a compile-time
// constant, so the Comba products are fully unrolled. Above 16 limbs the same code runs
// with a runtime limb count and Karatsuba from 40 limbs.
//
// Two formulations, both three real products per iteration's worth of work:
//   .squarings:       X^2, Y^2, (X+Y)^2;  X' = X^2 - Y^2 + Cx,  Y' = (X+Y)^2 - (X^2+Y^2) + Cy
//   .multiplications: (X+Y)(X-Y), X*Y;    X' = (X+Y)(X-Y) + Cx, Y' = 2XY + Cy
// The squaring form costs ~1.5 n^2 limb products against 2 n^2 for the multiplication form,
// but its Y' has absolute (not relative) error ~ulp(|Z|^2) when XY is small. Both are far
// below the pixel spacing at the recommended precision; the default is the faster one at the
// requested size (measured, see `Variant.automatic`).
//
// Precision rule (see `recommendedPrecisionBits`):
//     bits = ceil(-log2(pixelSpacing) + log2(imageSizeInPixels) + log2(max(|C|, 1)) + 1) + guard
// with guard = 48 by default, rounded up to whole 64-bit limbs. The first term resolves one
// pixel, the second keeps C itself exact to well below a pixel anywhere in the image, |C|
// accounts for C's integer bits, and the guard absorbs rounding error growth along the orbit.

import simd

public struct ReferenceOrbitComputer: Sendable {
    public enum Variant: Sendable {
        /// Three squarings per iteration.
        case squarings
        /// Two general multiplications per iteration.
        case multiplications
        /// Fastest measured on M1 Pro for the limb count.
        case automatic
    }

    public struct Result: Sendable {
        /// Points written: Z_0 ... Z_{count-1} (Z_0 = 0). Always >= 1.
        public var count: Int
        /// Iterations performed (count - 1).
        public var iterations: Int { count - 1 }
        /// The orbit escaped the bailout radius; its last point is the first outside.
        public var escaped: Bool
        /// Stopped early by the cancellation check.
        public var cancelled: Bool
        /// The last point at full precision (to resume or to verify).
        public var finalZ: BigComplex
    }

    /// The point C, at `precisionBits`.
    public let c: BigComplex
    public let precisionBits: Int
    public var variant: Variant = .automatic

    /// C is rounded to `precisionBits` (rounded up to whole limbs).
    public init(c: BigComplex, precisionBits: Int) {
        let bits = BigFloat.limbCount(forBits: precisionBits) * 64
        self.precisionBits = bits
        self.c = c.withPrecision(bits: bits)
    }

    /// Bits for a reference at `c` in a view with the given pixel spacing and image size.
    public static func recommendedPrecisionBits(pixelSpacingLog2: Double, imageSize: Int,
                                                centerMagnitude: Double = 2,
                                                guardBits: Int = 48) -> Int {
        let raw = -pixelSpacingLog2 + _log2Int(max(1, imageSize)) + _log2Int(Int(max(1, centerMagnitude.rounded(.up)))) + 1
        let bits = Int(raw.rounded(.up)) + guardBits
        return max(64, (bits + 63) / 64 * 64)
    }

    /// Same, with the pixel spacing as a BigFloat.
    public static func recommendedPrecisionBits(pixelSpacing: BigFloat, imageSize: Int,
                                                centerMagnitude: Double = 2,
                                                guardBits: Int = 48) -> Int {
        recommendedPrecisionBits(pixelSpacingLog2: pixelSpacing.log2Magnitude, imageSize: imageSize,
                                 centerMagnitude: centerMagnitude, guardBits: guardBits)
    }

    var resolvedVariant: Variant {
        switch variant {
        case .automatic:
            // Measured 2026-10-14 on M1 Pro (release): see synthesis/benchmarks-arbitrary-precision.
            return BigFloat.limbCount(forBits: precisionBits) <= automaticMultiplicationLimit ? .multiplications : .squarings
        default:
            return variant
        }
    }

    /// Computes the orbit into caller-provided buffers (either may be nil). Each needs room
    /// for `maxIterations + 1` points; `capacity` is checked. Stops at escape
    /// (|Z|^2 > bailout^2), after `maxIterations`, or when `isCancelled()` returns true; it
    /// is polled, and `progress` called with the iterations done, every `checkInterval`
    /// iterations. Safe to run on any background thread.
    public func compute(maxIterations: Int, bailout: Double = 2,
                        points: UnsafeMutablePointer<SIMD2<Float>>?,
                        extended: UnsafeMutablePointer<FloatExpComplex>?,
                        capacity: Int,
                        checkInterval: Int = 4096,
                        isCancelled: () -> Bool = { false },
                        progress: ((Int) -> Void)? = nil) -> Result {
        precondition(maxIterations >= 0 && capacity >= maxIterations + 1, "orbit buffer too small")
        let n = BigFloat.limbCount(forBits: precisionBits)
        let ws = RawWorkspace(limbs: n, registers: 10)
        var x = ws.register(0), y = ws.register(1)
        var cx = ws.register(2), cy = ws.register(3)
        ws.load(&cx, c.re)
        ws.load(&cy, c.im)
        var t = (ws.register(4), ws.register(5), ws.register(6), ws.register(7), ws.register(8), ws.register(9))
        points?[0] = .zero
        extended?[0] = .zero
        let b2 = bailout * bailout
        let useSquarings = resolvedVariant == .squarings
        var done = 0
        var escaped = false
        var cancelled = false
        let interval = max(1, checkInterval)
        while done < maxIterations {
            let end = min(maxIterations, done + interval)
            let (reached, esc) = runOrbitChunk(n: n, squarings: useSquarings, x: &x, y: &y, cx: cx, cy: cy,
                                               t: &t, scratch: ws.scratch, start: done, end: end,
                                               bailout2: b2, points: points, extended: extended)
            done = reached
            if esc { escaped = true; break }
            progress?(done)
            if done < maxIterations && isCancelled() { cancelled = true; break }
        }
        let fz = BigComplex(re: ws.store(x), im: ws.store(y))
        withExtendedLifetime(ws) {}
        return Result(count: done + 1, escaped: escaped, cancelled: cancelled, finalZ: fz)
    }

    /// Convenience: the orbit as arrays.
    public func compute(maxIterations: Int, bailout: Double = 2,
                        isCancelled: () -> Bool = { false }) -> (points: [SIMD2<Float>], extended: [FloatExpComplex], result: Result) {
        var pts = [SIMD2<Float>](repeating: .zero, count: maxIterations + 1)
        var ext = [FloatExpComplex](repeating: .zero, count: maxIterations + 1)
        let r = pts.withUnsafeMutableBufferPointer { pp in
            ext.withUnsafeMutableBufferPointer { ep in
                compute(maxIterations: maxIterations, bailout: bailout, points: pp.baseAddress!,
                        extended: ep.baseAddress!, capacity: maxIterations + 1, isCancelled: isCancelled)
            }
        }
        pts.removeLast(pts.count - r.count)
        ext.removeLast(ext.count - r.count)
        return (pts, ext, r)
    }
}

/// Limb counts up to this use the multiplication form under `.automatic`.
let automaticMultiplicationLimit = 16

@inline(__always) func _log2Int(_ v: Int) -> Double {
    // log2 for positive integers without libm: exponent plus a short series on the mantissa.
    BigFloat(v, precisionBits: 64).log2Magnitude
}

typealias OrbitTemps = (RawFloat, RawFloat, RawFloat, RawFloat, RawFloat, RawFloat)

/// Dispatches to a loop specialised for the limb count.
func runOrbitChunk(n: Int, squarings: Bool, x: inout RawFloat, y: inout RawFloat,
                   cx: RawFloat, cy: RawFloat, t: inout OrbitTemps, scratch: Limbs,
                   start: Int, end: Int, bailout2: Double,
                   points: UnsafeMutablePointer<SIMD2<Float>>?,
                   extended: UnsafeMutablePointer<FloatExpComplex>?) -> (Int, Bool) {
    @inline(__always) func go(_ k: Int) -> (Int, Bool) {
        squarings
            ? orbitLoopSquarings(n: k, x: &x, y: &y, cx: cx, cy: cy, t: &t, scratch: scratch, start: start,
                                 end: end, bailout2: bailout2, points: points, extended: extended)
            : orbitLoopMultiplications(n: k, x: &x, y: &y, cx: cx, cy: cy, t: &t, scratch: scratch, start: start,
                                       end: end, bailout2: bailout2, points: points, extended: extended)
    }
    switch n {
    case 1: return go(1)
    case 2: return go(2)
    case 3: return go(3)
    case 4: return go(4)
    case 5: return go(5)
    case 6: return go(6)
    case 7: return go(7)
    case 8: return go(8)
    case 9: return go(9)
    case 10: return go(10)
    case 11: return go(11)
    case 12: return go(12)
    case 13: return go(13)
    case 14: return go(14)
    case 15: return go(15)
    case 16: return go(16)
    default: return go(n)
    }
}

/// Stores Z at index i and tests escape. Returns true if escaped.
@inline(__always)
func emitPoint(_ x: RawFloat, _ y: RawFloat, _ n: Int, _ i: Int, _ bailout2: Double,
               _ points: UnsafeMutablePointer<SIMD2<Float>>?,
               _ extended: UnsafeMutablePointer<FloatExpComplex>?) -> Bool {
    let xd = rawToDoubleFast(x, n), yd = rawToDoubleFast(y, n)
    if let p = points { p[i] = SIMD2(Float(xd), Float(yd)) }
    if let e = extended { e[i] = makeFloatExpComplex(x, y, n) }
    return xd * xd + yd * yd > bailout2
}

@inline(__always)
func orbitLoopSquarings(n: Int, x: inout RawFloat, y: inout RawFloat, cx: RawFloat, cy: RawFloat,
                        t: inout OrbitTemps, scratch: Limbs, start: Int, end: Int, bailout2: Double,
                        points: UnsafeMutablePointer<SIMD2<Float>>?,
                        extended: UnsafeMutablePointer<FloatExpComplex>?) -> (Int, Bool) {
    var x2 = t.0, y2 = t.1, s = t.2, s2 = t.3, nrm = t.4, d = t.5
    defer { t = (x2, y2, s, s2, nrm, d) }
    var i = start
    while i < end {
        rawSqr(&x2, x, n, scratch)
        rawSqr(&y2, y, n, scratch)
        rawAdd(&s, x, y, subtract: false, n, scratch)
        rawSqr(&s2, s, n, scratch)
        rawAdd(&nrm, x2, y2, subtract: false, n, scratch)
        rawAdd(&d, x2, y2, subtract: true, n, scratch)
        rawAdd(&x, d, cx, subtract: false, n, scratch)
        rawAdd(&d, s2, nrm, subtract: true, n, scratch)
        rawAdd(&y, d, cy, subtract: false, n, scratch)
        i &+= 1
        if emitPoint(x, y, n, i, bailout2, points, extended) { return (i, true) }
    }
    return (i, false)
}

@inline(__always)
func orbitLoopMultiplications(n: Int, x: inout RawFloat, y: inout RawFloat, cx: RawFloat, cy: RawFloat,
                              t: inout OrbitTemps, scratch: Limbs, start: Int, end: Int, bailout2: Double,
                              points: UnsafeMutablePointer<SIMD2<Float>>?,
                              extended: UnsafeMutablePointer<FloatExpComplex>?) -> (Int, Bool) {
    var s = t.0, d = t.1, p1 = t.2, p2 = t.3
    defer { t.0 = s; t.1 = d; t.2 = p1; t.3 = p2 }
    var i = start
    while i < end {
        rawAdd(&s, x, y, subtract: false, n, scratch)
        rawAdd(&d, x, y, subtract: true, n, scratch)
        rawMul(&p1, s, d, n, scratch)
        rawMul(&p2, x, y, n, scratch)
        rawAdd(&x, p1, cx, subtract: false, n, scratch)
        if !p2.zero { p2.exp &+= 1 }
        rawAdd(&y, p2, cy, subtract: false, n, scratch)
        i &+= 1
        if emitPoint(x, y, n, i, bailout2, points, extended) { return (i, true) }
    }
    return (i, false)
}
