// Extended-range types for the GPU and for low-precision bookkeeping (kb: techniques/extended-range-floats).
//
// Convention (matches the kb pseudocode): value = mantissa * 2^exponent with
// 0.5 <= |mantissa| < 1, and zero as mantissa 0, exponent 0. GPU code must test the
// mantissa for zero rather than compare exponents.

/// Float mantissa + Int32 exponent (8 bytes; MSL `struct { float m; int e; }`).
public struct FloatExp: Sendable, Equatable {
    public var mantissa: Float
    public var exponent: Int32

    public init(mantissa: Float, exponent: Int32) {
        self.mantissa = mantissa
        self.exponent = exponent
    }

    public static let zero = FloatExp(mantissa: 0, exponent: 0)

    /// Nearest Double of the value (0 or inf outside Double's range).
    public var doubleValue: Double { Double(mantissa) * pow2Double(Int(exponent)) }
}

/// A complex value with one shared exponent: (mantissa.x + i mantissa.y) * 2^exponent, where
/// the larger component's mantissa magnitude is in [0.5, 1). The smaller component is
/// flushed to zero when it is more than ~2^-149 below the larger, which is far below float
/// precision relative to the larger one. Stride 16 bytes, alignment 8 (MSL
/// `struct { float2 m; int e; }`). This is the extended-range reference orbit form: kb
/// techniques/extended-range-floats notes that real and imaginary parts can share one
/// exponent, and the rescaled-iteration full steps only need |Z| to float precision.
public struct FloatExpComplex: Sendable, Equatable {
    public var mantissa: SIMD2<Float>
    public var exponent: Int32

    public init(mantissa: SIMD2<Float>, exponent: Int32) {
        self.mantissa = mantissa
        self.exponent = exponent
    }

    public static let zero = FloatExpComplex(mantissa: .zero, exponent: 0)

    /// Nearest Double pair (flushes to 0 outside Double's range).
    public var doubleValue: SIMD2<Double> {
        let s = pow2Double(Int(exponent))
        return SIMD2(Double(mantissa.x) * s, Double(mantissa.y) * s)
    }
}

/// Double mantissa + Int64 exponent, with the small set of operations the period detector
/// and size estimates need (radii of 1e-100000, derivatives of 1e+100000).
public struct DoubleExp: Sendable, Equatable, Comparable, CustomStringConvertible {
    public var mantissa: Double
    public var exponent: Int64

    public init(mantissa: Double, exponent: Int64) {
        self.mantissa = mantissa
        self.exponent = exponent
    }

    public static let zero = DoubleExp(mantissa: 0, exponent: 0)
    public static let one = DoubleExp(1.0)

    /// From a Double, normalised.
    public init(_ d: Double) {
        precondition(d.isFinite, "DoubleExp from a non-finite Double")
        if d == 0 { self = .zero; return }
        let e = Int64(d.exponent) + 1           // |d| = 1.f * 2^(e-1)
        let m = d.magnitude.significand * 0.5   // in [0.5, 1)
        self.init(mantissa: d < 0 ? -m : m, exponent: e)
    }

    @inline(__always) static func normalised(_ m: Double, _ e: Int64) -> DoubleExp {
        if m == 0 || !m.isFinite { return m == 0 ? .zero : DoubleExp(mantissa: m, exponent: e) }
        let k = Int64(m.exponent) + 1
        let mm = m.magnitude.significand * 0.5
        return DoubleExp(mantissa: m < 0 ? -mm : mm, exponent: e + k)
    }

    public var isZero: Bool { mantissa == 0 }

    /// The value as Double (0 or inf outside Double's range).
    public var doubleValue: Double {
        if exponent > 1100 { return mantissa < 0 ? -.infinity : .infinity }
        if exponent < -1100 { return 0 }
        let e = Int(exponent)
        if e >= -1000 { return mantissa * pow2Double(e) }
        return mantissa * pow2Double(e + 200) * 0x1p-200
    }

    /// log2 |value| (-infinity for zero).
    public var log2Magnitude: Double {
        if isZero { return -.infinity }
        // |mantissa| in [0.5, 1): log2 via the same series as BigFloat.
        let m = mantissa.magnitude
        let t = (m - 1) / (m + 1)
        let t2 = t * t
        var term = t, sum = 0.0, k = 1.0
        while k < 60 { sum += term / k; term *= t2; k += 2; if abs(term) < 1e-20 { break } }
        return Double(exponent) + 2 * sum / 0.6931471805599453
    }

    public var magnitude: DoubleExp { DoubleExp(mantissa: mantissa.magnitude, exponent: exponent) }

    public static func * (a: DoubleExp, b: DoubleExp) -> DoubleExp {
        normalised(a.mantissa * b.mantissa, a.exponent + b.exponent)
    }

    public static func / (a: DoubleExp, b: DoubleExp) -> DoubleExp {
        precondition(!b.isZero, "DoubleExp division by zero")
        return normalised(a.mantissa / b.mantissa, a.exponent - b.exponent)
    }

    public static func + (a: DoubleExp, b: DoubleExp) -> DoubleExp {
        if a.isZero { return b }
        if b.isZero { return a }
        let (x, y) = a.exponent >= b.exponent ? (a, b) : (b, a)
        let d = x.exponent - y.exponent
        if d > 60 { return x }
        return normalised(x.mantissa + y.mantissa * pow2Double(-Int(d)), x.exponent)
    }

    public static prefix func - (a: DoubleExp) -> DoubleExp {
        DoubleExp(mantissa: -a.mantissa, exponent: a.exponent)
    }

    public static func - (a: DoubleExp, b: DoubleExp) -> DoubleExp { a + (-b) }

    public func squareRoot() -> DoubleExp {
        precondition(mantissa >= 0, "square root of a negative DoubleExp")
        if isZero { return .zero }
        if exponent & 1 == 0 {
            return DoubleExp.normalised(mantissa.squareRoot(), exponent / 2)
        }
        return DoubleExp.normalised((mantissa * 2).squareRoot(), (exponent - 1) / 2)
    }

    /// Multiplies by 2^k.
    public func timesPowerOfTwo(_ k: Int64) -> DoubleExp {
        isZero ? self : DoubleExp(mantissa: mantissa, exponent: exponent + k)
    }

    public static func < (a: DoubleExp, b: DoubleExp) -> Bool {
        let sa = a.mantissa < 0 ? -1 : (a.mantissa > 0 ? 1 : 0)
        let sb = b.mantissa < 0 ? -1 : (b.mantissa > 0 ? 1 : 0)
        if sa != sb { return sa < sb }
        if sa == 0 { return false }
        if a.exponent != b.exponent {
            return sa > 0 ? a.exponent < b.exponent : a.exponent > b.exponent
        }
        return a.mantissa < b.mantissa
    }

    public var description: String {
        if isZero { return "0" }
        let l10 = log2Magnitude * 0.30102999566398120
        let e10 = (l10).rounded(.down)
        var m = 1.0
        // 10^(l10 - e10) via repeated multiplication-free exp2 of the fractional part.
        let frac = (l10 - e10) / 0.30102999566398120   // in log2 units, [0, 3.33)
        m = exp2Approx(frac)
        let sign = mantissa < 0 ? "-" : ""
        return "\(sign)\(m)e\(Int(e10))"
    }
}

/// 2^x for x in [0, 4) (description only).
private func exp2Approx(_ x: Double) -> Double {
    let i = x.rounded(.down)
    let f = (x - i) * 0.6931471805599453
    var term = 1.0, sum = 1.0, k = 1.0
    while k < 30 { term *= f / k; sum += term; k += 1 }
    return sum * pow2Double(Int(i))
}

/// Complex DoubleExp with separate exponents (bookkeeping only, not a GPU type).
public struct DoubleExpComplex: Sendable, Equatable {
    public var re: DoubleExp
    public var im: DoubleExp

    public init(re: DoubleExp, im: DoubleExp) {
        self.re = re
        self.im = im
    }

    public static let zero = DoubleExpComplex(re: .zero, im: .zero)
    public static let one = DoubleExpComplex(re: .one, im: .zero)

    public static func + (a: DoubleExpComplex, b: DoubleExpComplex) -> DoubleExpComplex {
        DoubleExpComplex(re: a.re + b.re, im: a.im + b.im)
    }

    public static func * (a: DoubleExpComplex, b: DoubleExpComplex) -> DoubleExpComplex {
        DoubleExpComplex(re: a.re * b.re - a.im * b.im, im: a.re * b.im + a.im * b.re)
    }

    public static func / (a: DoubleExpComplex, b: DoubleExpComplex) -> DoubleExpComplex {
        let den = b.re * b.re + b.im * b.im
        return DoubleExpComplex(re: (a.re * b.re + a.im * b.im) / den,
                                im: (a.im * b.re - a.re * b.im) / den)
    }

    public var magnitudeSquared: DoubleExp { re * re + im * im }
    public var magnitude: DoubleExp { magnitudeSquared.squareRoot() }
}
