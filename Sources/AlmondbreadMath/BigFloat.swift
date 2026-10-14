// BigFloat: arbitrary-precision binary floating point (kb: decisions/arbitrary-precision).
//
// Representation: a sign, an Int exponent and an n-limb UInt64 mantissa, value
//     (-1)^isNegative * mantissa / 2^(64n) * 2^exponent,  top mantissa bit set,
// so |value| is in [2^(exponent-1), 2^exponent). Precision is fixed per value at creation,
// in whole limbs (precisionBits rounded up to a multiple of 64). Every operation rounds to
// nearest, ties to even (correctly rounded) except `squareRoot` (within 1 ulp). The exponent
// is a full Int, so there is no overflow or underflow in practice: derivatives of size
// 1e100000 and radii of 1e-100000 are fine, which a fixed-point format could not do.
//
// Why a floating format and not fixed point: the orbit itself stays within |Z| <= 2, but
// Newton's derivative and the ball-method radii span enormous ranges, and small |Z_n| near
// a nucleus keeps relative precision. Exponent handling costs a few instructions per add,
// which is noise next to the O(n^2) products above two limbs.
//
// Value semantics with copy-on-write storage. The operators allocate a result; the compound
// assignments (`+=`, `*=`, `formSquare()` ...) reuse uniquely referenced storage. The hot
// loops (ReferenceOrbitComputer, period detection, Newton) do not use BigFloat at all; they
// run on preallocated RawFloat registers.
//
// Mixed precision: the result of a binary operation has the larger of the two precisions.

public struct BigFloat: Sendable {
    @usableFromInline var limbs: ContiguousArray<UInt64>
    public internal(set) var isNegative: Bool
    /// Binary exponent: |value| is in [2^(exponent-1), 2^exponent). 0 for zero.
    public internal(set) var exponent: Int

    // MARK: Basic properties

    public var limbCount: Int { limbs.count }
    public var precisionBits: Int { limbs.count * 64 }
    public var isZero: Bool { limbs[limbs.count - 1] == 0 }
    /// The normalised mantissa limbs, least significant first.
    public var significandLimbs: [UInt64] { Array(limbs) }

    /// Number of limbs for a precision in bits (at least one).
    @inlinable public static func limbCount(forBits bits: Int) -> Int { max(1, (bits + 63) / 64) }

    // MARK: Initialisers

    /// Zero at the given precision.
    public init(precisionBits: Int) {
        limbs = ContiguousArray(repeating: 0, count: BigFloat.limbCount(forBits: precisionBits))
        isNegative = false
        exponent = 0
    }

    @usableFromInline init(zeroLimbs n: Int) {
        limbs = ContiguousArray(repeating: 0, count: n)
        isNegative = false
        exponent = 0
    }

    /// Exact conversion from Double (any precision holds all 53 bits).
    public init(_ value: Double, precisionBits: Int) {
        self.init(precisionBits: precisionBits)
        let n = limbCount
        withMutableRaw { r in rawSetDouble(&r, value, n) }
    }

    /// Conversion from Int (exact when precisionBits >= 64).
    public init(_ value: Int, precisionBits: Int) {
        self.init(negative: value < 0, integerSignificand: [UInt64(value.magnitude)], exponent: 0,
                  precisionBits: precisionBits)
    }

    /// value = (-1)^negative * S * 2^exponent, where S is the unsigned integer with the given
    /// little-endian limbs; rounded to nearest-even at `precisionBits`.
    public init(negative: Bool, integerSignificand: [UInt64], exponent: Int, precisionBits: Int) {
        let n = BigFloat.limbCount(forBits: precisionBits)
        self.init(zeroLimbs: n)
        var s = BigNat.trimmed(integerSignificand)
        if s.isEmpty { return }
        let k = s.count
        let bl = s.withUnsafeMutableBufferPointer { sp in
            limbs.withUnsafeMutableBufferPointer { lp in
                roundWide(sp.baseAddress!, k, stickyIn: false, n, lp.baseAddress!)
            }
        }
        isNegative = negative
        self.exponent = bl + exponent
    }

    /// The same value at another precision (exact when widening, rounded when narrowing).
    public func withPrecision(bits: Int) -> BigFloat {
        withLimbCount(BigFloat.limbCount(forBits: bits))
    }

    @usableFromInline func withLimbCount(_ n: Int) -> BigFloat {
        let m = limbCount
        if n == m { return self }
        var r = BigFloat(zeroLimbs: n)
        if isZero { return r }
        r.isNegative = isNegative
        if n > m {
            for i in 0..<m { r.limbs[n - m + i] = limbs[i] }
            r.exponent = exponent
        } else {
            var src = limbs
            let adj = src.withUnsafeMutableBufferPointer { sp in
                r.limbs.withUnsafeMutableBufferPointer { rp in
                    roundMantissa(sp.baseAddress!, m, n, rp.baseAddress!)
                }
            }
            r.exponent = exponent + adj
        }
        return r
    }

    // MARK: Raw access

    /// Runs `body` with a read-only RawFloat view of this value.
    func withRaw<R>(_ body: (RawFloat) -> R) -> R {
        limbs.withUnsafeBufferPointer { p in
            var raw = RawFloat(m: UnsafeMutablePointer(mutating: p.baseAddress!))
            raw.neg = isNegative
            raw.exp = exponent
            raw.zero = p[p.count - 1] == 0
            return body(raw)
        }
    }

    /// Runs `body` with a mutable RawFloat view, then stores the header back.
    mutating func withMutableRaw<R>(_ body: (inout RawFloat) -> R) -> R {
        let neg = isNegative, e = exponent
        var header = (neg, e, false)
        let result: R = limbs.withUnsafeMutableBufferPointer { p in
            var raw = RawFloat(m: p.baseAddress!)
            raw.neg = neg
            raw.exp = e
            raw.zero = p[p.count - 1] == 0
            let res = body(&raw)
            header = (raw.zero ? false : raw.neg, raw.zero ? 0 : raw.exp, raw.zero)
            if raw.zero { limbsZero(p.baseAddress!, p.count) }
            return res
        }
        isNegative = header.0
        exponent = header.1
        return result
    }

    @usableFromInline
    static func binary(_ a: BigFloat, _ b: BigFloat,
                       _ op: (inout RawFloat, RawFloat, RawFloat, Int, Limbs) -> Void) -> BigFloat {
        let n = max(a.limbCount, b.limbCount)
        let pa = a.withLimbCount(n), pb = b.withLimbCount(n)
        var r = BigFloat(zeroLimbs: n)
        withUnsafeTemporaryAllocation(of: UInt64.self, capacity: rawScratchLimbs(n)) { s in
            pa.withRaw { ra in
                pb.withRaw { rb in
                    r.withMutableRaw { rr in op(&rr, ra, rb, n, s.baseAddress!) }
                }
            }
        }
        return r
    }

    /// In place: self = op(self, b), reusing storage when uniquely referenced.
    @usableFromInline
    mutating func formBinary(_ b: BigFloat,
                             _ op: (inout RawFloat, RawFloat, RawFloat, Int, Limbs) -> Void) {
        if b.limbCount > limbCount {
            self = BigFloat.binary(self, b, op)
            return
        }
        let n = limbCount
        let pb = b.withLimbCount(n)
        withUnsafeTemporaryAllocation(of: UInt64.self, capacity: rawScratchLimbs(n)) { s in
            pb.withRaw { rb in
                withMutableRaw { rr in
                    let a0 = rr
                    op(&rr, a0, rb, n, s.baseAddress!)
                }
            }
        }
    }

    // MARK: Arithmetic

    public static func + (a: BigFloat, b: BigFloat) -> BigFloat {
        binary(a, b) { r, x, y, n, s in rawAdd(&r, x, y, subtract: false, n, s) }
    }

    public static func - (a: BigFloat, b: BigFloat) -> BigFloat {
        binary(a, b) { r, x, y, n, s in rawAdd(&r, x, y, subtract: true, n, s) }
    }

    public static func * (a: BigFloat, b: BigFloat) -> BigFloat {
        binary(a, b) { r, x, y, n, s in rawMul(&r, x, y, n, s) }
    }

    public static func += (a: inout BigFloat, b: BigFloat) {
        a.formBinary(b) { r, x, y, n, s in rawAdd(&r, x, y, subtract: false, n, s) }
    }

    public static func -= (a: inout BigFloat, b: BigFloat) {
        a.formBinary(b) { r, x, y, n, s in rawAdd(&r, x, y, subtract: true, n, s) }
    }

    public static func *= (a: inout BigFloat, b: BigFloat) {
        a.formBinary(b) { r, x, y, n, s in rawMul(&r, x, y, n, s) }
    }

    public static prefix func - (a: BigFloat) -> BigFloat {
        var r = a
        r.negate()
        return r
    }

    public mutating func negate() {
        if !isZero { isNegative.toggle() }
    }

    public var magnitude: BigFloat {
        var r = self
        r.isNegative = false
        return r
    }

    /// self * self (dedicated squaring, about half the limb products of a multiply).
    public func squared() -> BigFloat {
        var r = self
        r.formSquare()
        return r
    }

    public mutating func formSquare() {
        let n = limbCount
        withUnsafeTemporaryAllocation(of: UInt64.self, capacity: rawScratchLimbs(n)) { s in
            withMutableRaw { rr in
                let a0 = rr
                rawSqr(&rr, a0, n, s.baseAddress!)
            }
        }
    }

    /// self * 2^k (exact).
    public func timesPowerOfTwo(_ k: Int) -> BigFloat {
        var r = self
        if !r.isZero { r.exponent += k }
        return r
    }

    /// self * m for a machine integer m (correctly rounded).
    public func multiplied(by m: Int) -> BigFloat {
        if isZero || m == 0 { return BigFloat(zeroLimbs: limbCount) }
        let n = limbCount
        var wide = [UInt64](repeating: 0, count: n + 1)
        var src = limbs
        src.withUnsafeMutableBufferPointer { sp in
            wide.withUnsafeMutableBufferPointer { wp in
                wp[n] = limbsMul1(wp.baseAddress!, sp.baseAddress!, n, UInt64(m.magnitude))
            }
        }
        var r = BigFloat(zeroLimbs: n)
        let bl = wide.withUnsafeMutableBufferPointer { wp in
            r.limbs.withUnsafeMutableBufferPointer { rp in
                roundWide(wp.baseAddress!, n + 1, stickyIn: false, n, rp.baseAddress!)
            }
        }
        r.isNegative = isNegative != (m < 0)
        r.exponent = exponent - 64 * n + bl
        return r
    }

    /// Correctly rounded quotient. Division by zero is a precondition failure.
    public static func / (a: BigFloat, b: BigFloat) -> BigFloat {
        precondition(!b.isZero, "BigFloat division by zero")
        let n = max(a.limbCount, b.limbCount)
        if a.isZero { return BigFloat(zeroLimbs: n) }
        let pa = a.withLimbCount(n), pb = b.withLimbCount(n)
        // q = floor(ma * 2^(64(n+1)) / mb) has 64n+1 or 64n+2 bits; the remainder is sticky.
        let u = [UInt64](repeating: 0, count: n + 1) + Array(pa.limbs)
        let (q, rem) = BigNat.divMod(u, Array(pb.limbs))
        var r = BigFloat(zeroLimbs: n)
        var qq = q
        let bl = qq.withUnsafeMutableBufferPointer { qp in
            r.limbs.withUnsafeMutableBufferPointer { rp in
                roundWide(qp.baseAddress!, qp.count, stickyIn: !rem.isEmpty, n, rp.baseAddress!)
            }
        }
        r.isNegative = pa.isNegative != pb.isNegative
        r.exponent = pa.exponent - pb.exponent - 64 * (n + 1) + bl
        return r
    }

    public static func /= (a: inout BigFloat, b: BigFloat) { a = a / b }

    /// Square root (Newton on the reciprocal-free form, within 1 ulp). Negative input is a
    /// precondition failure.
    public func squareRoot() -> BigFloat {
        precondition(!isNegative, "square root of a negative BigFloat")
        if isZero { return self }
        let n = limbCount
        let work = n + 1
        let x = withLimbCount(work)
        // Seed from Double on an even exponent so the seed's exponent is exact.
        let e = exponent - (exponent & 1)           // x = m * 2^e with m in [0.25, 1)
        let mant = withRaw { r -> Double in rawDoubleMantissa(r, n) } * (exponent & 1 == 0 ? 1 : 2)
        var y = BigFloat(mant.squareRoot(), precisionBits: work * 64).timesPowerOfTwo(e / 2)
        // Each Newton step doubles the correct bits (~50 from the seed).
        var bits = 50
        while bits < work * 64 + 8 {
            y = (y + x / y).timesPowerOfTwo(-1)
            bits *= 2
        }
        y = (y + x / y).timesPowerOfTwo(-1)
        return y.withLimbCount(n)
    }

    // MARK: Conversions

    /// Nearest Double (ties to even; may double-round in the subnormal range).
    public var doubleValue: Double { withRaw { rawToDouble($0, limbCount) } }

    /// log2 |value| as a Double (-infinity for zero), accurate to ~1e-15 relative.
    public var log2Magnitude: Double {
        if isZero { return -.infinity }
        let mant = withRaw { rawDoubleMantissa($0, limbCount) }
        return Double(exponent) + mant.log2Value
    }

    /// Float mantissa in [0.5, 1) and Int32 exponent (exponent clamped to the Int32 range).
    public var floatExp: FloatExp {
        if isZero { return .zero }
        let n = limbCount
        var mant = withRaw { r -> Float in Float(jammedTopLimb(r.m, n)) * 0x1p-64 }
        var e = exponent
        if mant == 1 { mant = 0.5; e += 1 }
        return FloatExp(mantissa: isNegative ? -mant : mant, exponent: Int32(clamping: e))
    }

    /// Double mantissa in [0.5, 1) and Int64 exponent.
    public var doubleExp: DoubleExp {
        if isZero { return .zero }
        var mant = withRaw { rawDoubleMantissa($0, limbCount) }
        var e = exponent
        if mant == 1 { mant = 0.5; e += 1 }
        return DoubleExp(mantissa: isNegative ? -mant : mant, exponent: Int64(e))
    }

    /// Exact value as (negative, integer significand S, binary exponent E): value = ±S·2^E.
    public var exactParts: (negative: Bool, significand: [UInt64], exponent: Int) {
        (isNegative, Array(limbs), exponent - 64 * limbCount)
    }
}

private extension Double {
    /// log2 for x in [0.5, 1] without libm (keeps the module free of platform imports):
    /// x = (1 + t) / (1 - t), ln x = 2 (t + t^3/3 + t^5/5 + ...), |t| <= 1/3.
    var log2Value: Double {
        let t = (self - 1) / (self + 1)
        let t2 = t * t
        var term = t
        var sum = 0.0
        var k = 1.0
        while k < 60 {
            sum += term / k
            term *= t2
            k += 2
            if abs(term) < 1e-20 { break }
        }
        return 2 * sum / 0.6931471805599453
    }
}

// MARK: - Comparison

extension BigFloat: Comparable {
    /// Value equality (precision is ignored).
    public static func == (a: BigFloat, b: BigFloat) -> Bool { compare(a, b) == 0 }

    public static func < (a: BigFloat, b: BigFloat) -> Bool { compare(a, b) < 0 }

    /// -1, 0, 1.
    public static func compare(_ a: BigFloat, _ b: BigFloat) -> Int {
        let n = max(a.limbCount, b.limbCount)
        let pa = a.withLimbCount(n), pb = b.withLimbCount(n)
        return pa.withRaw { ra in pb.withRaw { rb in rawCompare(ra, rb, n) } }
    }
}

extension BigFloat: CustomStringConvertible {
    /// Shortest-safe decimal form: enough digits to round-trip at this precision.
    public var description: String { toDecimalString() }
}
