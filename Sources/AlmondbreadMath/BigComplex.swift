// Complex numbers over BigFloat, for viewports, references and Newton steps.

public struct BigComplex: Sendable, Equatable, CustomStringConvertible {
    public var re: BigFloat
    public var im: BigFloat

    public init(re: BigFloat, im: BigFloat) {
        self.re = re
        self.im = im
    }

    public init(_ re: Double, _ im: Double, precisionBits: Int) {
        self.re = BigFloat(re, precisionBits: precisionBits)
        self.im = BigFloat(im, precisionBits: precisionBits)
    }

    /// Parses decimal strings for both parts (see `BigFloat.init?(_:precisionBits:)`).
    public init?(re: String, im: String, precisionBits: Int) {
        guard let r = BigFloat(re, precisionBits: precisionBits),
              let i = BigFloat(im, precisionBits: precisionBits) else { return nil }
        self.init(re: r, im: i)
    }

    public static func zero(precisionBits: Int) -> BigComplex {
        BigComplex(re: BigFloat(precisionBits: precisionBits), im: BigFloat(precisionBits: precisionBits))
    }

    public var precisionBits: Int { max(re.precisionBits, im.precisionBits) }

    public func withPrecision(bits: Int) -> BigComplex {
        BigComplex(re: re.withPrecision(bits: bits), im: im.withPrecision(bits: bits))
    }

    public static func + (a: BigComplex, b: BigComplex) -> BigComplex {
        BigComplex(re: a.re + b.re, im: a.im + b.im)
    }

    public static func - (a: BigComplex, b: BigComplex) -> BigComplex {
        BigComplex(re: a.re - b.re, im: a.im - b.im)
    }

    public static prefix func - (a: BigComplex) -> BigComplex {
        BigComplex(re: -a.re, im: -a.im)
    }

    /// Three real multiplications are not used here on purpose: (ac - bd) and (ad + bc) with
    /// four correctly rounded products keep each part accurate relative to its own size.
    public static func * (a: BigComplex, b: BigComplex) -> BigComplex {
        BigComplex(re: a.re * b.re - a.im * b.im, im: a.re * b.im + a.im * b.re)
    }

    /// (x + iy)^2 = (x+y)(x-y) + 2xy i: two real multiplications.
    public func squared() -> BigComplex {
        BigComplex(re: (re + im) * (re - im), im: (re * im).timesPowerOfTwo(1))
    }

    public static func / (a: BigComplex, b: BigComplex) -> BigComplex {
        let den = b.re.squared() + b.im.squared()
        return BigComplex(re: (a.re * b.re + a.im * b.im) / den,
                          im: (a.im * b.re - a.re * b.im) / den)
    }

    public func timesPowerOfTwo(_ k: Int) -> BigComplex {
        BigComplex(re: re.timesPowerOfTwo(k), im: im.timesPowerOfTwo(k))
    }

    public func multiplied(by s: BigFloat) -> BigComplex {
        BigComplex(re: re * s, im: im * s)
    }

    /// |z|^2 (correctly rounded parts, one rounding for the sum).
    public var magnitudeSquared: BigFloat { re.squared() + im.squared() }

    /// |z| in extended range (for comparisons against radii).
    public var magnitudeDoubleExp: DoubleExp {
        DoubleExpComplex(re: re.doubleExp, im: im.doubleExp).magnitude
    }

    /// Nearest Double pair.
    public var doubleValue: SIMD2<Double> { SIMD2(re.doubleValue, im.doubleValue) }

    /// Float32 pair (for the GPU's float path).
    public var floatValue: SIMD2<Float> { SIMD2(Float(re.doubleValue), Float(im.doubleValue)) }

    /// Shared-exponent extended-range form (see `FloatExpComplex`).
    public var floatExpComplex: FloatExpComplex {
        let n = max(re.limbCount, im.limbCount)
        let a = re.withLimbCount(n), b = im.withLimbCount(n)
        return a.withRaw { ra in b.withRaw { rb in makeFloatExpComplex(ra, rb, n) } }
    }

    public var description: String { "(\(re), \(im))" }
}

/// Shared-exponent Float form of x + iy (see FloatExpComplex).
@inline(__always) @usableFromInline
func makeFloatExpComplex(_ x: RawFloat, _ y: RawFloat, _ n: Int) -> FloatExpComplex {
    if x.zero && y.zero { return .zero }
    let e: Int
    if x.zero { e = y.exp } else if y.zero { e = x.exp } else { e = max(x.exp, y.exp) }
    // Mantissas relative to 2^e, rounded once to Float.
    var mx: Double = 0, my: Double = 0
    if !x.zero {
        let d = x.exp &- e
        mx = d < -200 ? 0 : Double(jammedTopLimb(x.m, n)) * 0x1p-64 * pow2Double(d)
        if x.neg { mx = -mx }
    }
    if !y.zero {
        let d = y.exp &- e
        my = d < -200 ? 0 : Double(jammedTopLimb(y.m, n)) * 0x1p-64 * pow2Double(d)
        if y.neg { my = -my }
    }
    var fx = Float(mx), fy = Float(my)
    var ee = e
    if max(fx.magnitude, fy.magnitude) >= 1 {
        fx *= 0.5
        fy *= 0.5
        ee &+= 1
    }
    return FloatExpComplex(mantissa: SIMD2(fx, fy), exponent: Int32(clamping: ee))
}
