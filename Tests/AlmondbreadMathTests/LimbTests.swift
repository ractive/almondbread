import XCTest
@testable import AlmondbreadMath

final class LimbTests: XCTestCase {
    /// Karatsuba (mul and sqr) against the Comba basecase on random operands.
    func testKaratsubaMatchesBasecase() {
        var rng = SystemRandomNumberGenerator()
        for n in [40, 41, 47, 48, 63, 64, 65, 96, 127, 128, 200, 256, 257] {
            var a = (0..<n).map { _ in UInt64.random(in: .min ... .max, using: &rng) }
            var b = (0..<n).map { _ in UInt64.random(in: .min ... .max, using: &rng) }
            if n % 2 == 0 { a[n - 1] = .max; b[0] = .max }       // carry-heavy
            var r1 = [UInt64](repeating: 0, count: 2 * n)
            var r2 = r1, r3 = r1, r4 = r1
            var scratch = [UInt64](repeating: 0, count: productScratchLimbs(n) + 1)
            a.withUnsafeMutableBufferPointer { ap in
                b.withUnsafeMutableBufferPointer { bp in
                    scratch.withUnsafeMutableBufferPointer { sp in
                        r1.withUnsafeMutableBufferPointer { mulBasecase($0.baseAddress!, ap.baseAddress!, bp.baseAddress!, n) }
                        r2.withUnsafeMutableBufferPointer { mulKaratsuba($0.baseAddress!, ap.baseAddress!, bp.baseAddress!, n, sp.baseAddress!) }
                        r3.withUnsafeMutableBufferPointer { sqrBasecase($0.baseAddress!, ap.baseAddress!, n) }
                        r4.withUnsafeMutableBufferPointer { sqrKaratsuba($0.baseAddress!, ap.baseAddress!, n, sp.baseAddress!) }
                    }
                }
            }
            XCTAssertEqual(r1, r2, "mul n=\(n)")
            XCTAssertEqual(r3, r4, "sqr n=\(n)")
        }
    }

    func testSquareBasecaseMatchesMul() {
        for n in 1...20 {
            var a = (0..<n).map { _ in UInt64.random(in: .min ... .max) }
            if n > 2 { a[1] = .max; a[n - 1] = .max }
            var b = a
            var r1 = [UInt64](repeating: 0, count: 2 * n), r2 = r1
            a.withUnsafeMutableBufferPointer { ap in
                b.withUnsafeMutableBufferPointer { bp in
                    r1.withUnsafeMutableBufferPointer { mulBasecase($0.baseAddress!, ap.baseAddress!, bp.baseAddress!, n) }
                    r2.withUnsafeMutableBufferPointer { sqrBasecase($0.baseAddress!, ap.baseAddress!, n) }
                }
            }
            XCTAssertEqual(r1, r2, "n=\(n)")
        }
    }

    func testDivModAndSqrt() {
        let a = BigFloat("2", precisionBits: 512)!
        let r = a.squareRoot()
        let err = (r * r - a).log2Magnitude
        XCTAssertLessThan(err, -505)
        let third = BigFloat(1, precisionBits: 256) / BigFloat(3, precisionBits: 256)
        XCTAssertEqual(third.toDecimalString(significantDigits: 30), "0.333333333333333333333333333333")
        XCTAssertEqual(BigFloat("1e-1000", precisionBits: 4096)!.toDecimalString(significantDigits: 5), "1e-1000")
    }

    func testFloatExpConversions() {
        let v = BigFloat("-3.0e-1000", precisionBits: 256)!
        let fe = v.floatExp
        XCTAssertLessThan(fe.mantissa, 0)
        XCTAssertEqual(Double(fe.exponent), v.log2Magnitude.rounded(.down) + 1)
        XCTAssertTrue(fe.mantissa.magnitude >= 0.5 && fe.mantissa.magnitude < 1)
        let de = v.doubleExp
        XCTAssertEqual(de.log2Magnitude, v.log2Magnitude, accuracy: 1e-9)
        XCTAssertEqual(BigFloat(0.75, precisionBits: 64).floatExp, FloatExp(mantissa: 0.75, exponent: 0))
        XCTAssertEqual(BigFloat(-6.0, precisionBits: 64).doubleExp, DoubleExp(mantissa: -0.75, exponent: 3))
        let z = BigComplex(re: BigFloat("1e-400", precisionBits: 192)!, im: BigFloat("-2e-400", precisionBits: 192)!)
        let fc = z.floatExpComplex
        XCTAssertTrue(max(fc.mantissa.x.magnitude, fc.mantissa.y.magnitude) >= 0.5)
        XCTAssertEqual(Double(fc.mantissa.x / fc.mantissa.y), -0.5, accuracy: 1e-6)
        XCTAssertEqual(BigFloat(1.5, precisionBits: 64).doubleValue, 1.5)
        XCTAssertEqual(BigFloat(-0x1p-1074, precisionBits: 64).doubleValue, -0x1p-1074)
    }
}
