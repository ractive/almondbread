import Foundation
import XCTest
import simd
@testable import AlmondbreadMath

// Fixtures come from oracle/generate_fixtures.py (exact Python integer arithmetic with
// explicit round-to-nearest-even, and mpmath for nuclei). Expected values are bit-exact.

enum Fixture {
    static func load(_ name: String) throws -> Any {
        let url = try XCTUnwrap(Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Fixtures"))
        return try JSONSerialization.jsonObject(with: Data(contentsOf: url))
    }

    static func limbs(fromHex hex: String) -> [UInt64] {
        var out: [UInt64] = []
        var end = hex.endIndex
        while end > hex.startIndex {
            let start = hex.index(end, offsetBy: -16, limitedBy: hex.startIndex) ?? hex.startIndex
            out.append(UInt64(hex[start..<end], radix: 16)!)
            end = start
        }
        return out
    }

    /// Decodes [negative, hex, exponent] (or null = zero).
    static func value(_ any: Any?, prec: Int) -> BigFloat {
        guard let arr = any as? [Any] else { return BigFloat(precisionBits: prec) }
        let neg = arr[0] as! Bool
        let hex = arr[1] as! String
        let e = (arr[2] as! NSNumber).intValue
        return BigFloat(negative: neg, integerSignificand: limbs(fromHex: hex), exponent: e, precisionBits: prec)
    }

    static func hex(_ limbs: [UInt64]) -> String {
        var s = ""
        for (i, l) in limbs.reversed().enumerated() {
            let h = String(l, radix: 16)
            s += i == 0 ? h : String(repeating: "0", count: 16 - h.count) + h
        }
        return s
    }
}

/// Bit-exact comparison against an encoded expected value.
func assertBits(_ v: BigFloat, _ expected: Any?, prec: Int, _ what: @autoclosure () -> String,
                file: StaticString = #filePath, line: UInt = #line) {
    XCTAssertEqual(v.precisionBits, prec, "precision of \(what())", file: file, line: line)
    guard let arr = expected as? [Any] else {
        XCTAssertTrue(v.isZero, "\(what()): expected 0, got \(v)", file: file, line: line)
        return
    }
    let neg = arr[0] as! Bool
    let hex = arr[1] as! String
    let e = (arr[2] as! NSNumber).intValue
    let parts = v.exactParts
    let ok = !v.isZero && parts.negative == neg && Fixture.hex(parts.significand) == hex && parts.exponent == e
    XCTAssertTrue(ok, "\(what()): got \(parts.negative) \(Fixture.hex(parts.significand)) e\(parts.exponent), expected \(neg) \(hex) e\(e)",
                  file: file, line: line)
}

final class ArithmeticOracleTests: XCTestCase {
    func testArithmeticAgainstOracle() throws {
        let cases = try Fixture.load("arith") as! [[String: Any]]
        XCTAssertGreaterThan(cases.count, 1000)
        for (i, c) in cases.enumerated() {
            let prec = (c["prec"] as! NSNumber).intValue
            let a = Fixture.value(c["a"], prec: prec)
            let b = Fixture.value(c["b"], prec: prec)
            assertBits(a + b, c["add"], prec: prec, "case \(i) add")
            assertBits(a - b, c["sub"], prec: prec, "case \(i) sub")
            assertBits(a * b, c["mul"], prec: prec, "case \(i) mul")
            assertBits(a.squared(), c["sqr"], prec: prec, "case \(i) sqr")
            assertBits(a / b, c["div"], prec: prec, "case \(i) div")
            let small = (c["small"] as! NSNumber).intValue
            assertBits(a.multiplied(by: small), c["mulSmall"], prec: prec, "case \(i) mulSmall")
            // Compound forms must agree with the operators.
            var x = a; x += b
            assertBits(x, c["add"], prec: prec, "case \(i) +=")
            var y = a; y *= b
            assertBits(y, c["mul"], prec: prec, "case \(i) *=")
            var z = a; z.formSquare()
            assertBits(z, c["sqr"], prec: prec, "case \(i) formSquare")
            if let bits = c["doubleBits"] as? String {
                XCTAssertEqual(a.doubleValue.bitPattern, UInt64(bits, radix: 16)!, "case \(i) double")
            }
        }
    }

    func testParseAgainstOracle() throws {
        let cases = try Fixture.load("parse") as! [[String: Any]]
        for c in cases {
            let prec = (c["prec"] as! NSNumber).intValue
            let text = c["text"] as! String
            let v = try XCTUnwrap(BigFloat(text, precisionBits: prec), "parse \(text)")
            assertBits(v, c["value"], prec: prec, "parse \(text) @\(prec)")
        }
    }

    func testFormatAgainstOracle() throws {
        let cases = try Fixture.load("format") as! [[String: Any]]
        for c in cases {
            let prec = (c["prec"] as! NSNumber).intValue
            let v = Fixture.value(c["value"], prec: prec)
            let digits = (c["digits"] as? NSNumber)?.intValue
            XCTAssertEqual(v.toDecimalString(significantDigits: digits), c["text"] as! String)
        }
    }

    func testDecimalRoundTrip() throws {
        let cases = try Fixture.load("arith") as! [[String: Any]]
        for c in cases.prefix(400) {
            let prec = (c["prec"] as! NSNumber).intValue
            for key in ["a", "mul", "div"] {
                let v = Fixture.value(c[key], prec: prec)
                let s = v.description
                let back = try XCTUnwrap(BigFloat(s, precisionBits: prec))
                XCTAssertTrue(back == v && back.exactParts.significand == v.exactParts.significand, "round trip \(s)")
            }
        }
    }

    func testMalformedStrings() {
        for s in ["", "-", ".", "1e", "1.2.3", "abc", "1e+", "--1", "1 2"] {
            XCTAssertNil(BigFloat(s, precisionBits: 128), s)
        }
        XCTAssertEqual(BigFloat(" -0.5e1 ", precisionBits: 64)!.doubleValue, -5)
        XCTAssertEqual(BigFloat(".25", precisionBits: 64)!.doubleValue, 0.25)
    }
}

final class OrbitOracleTests: XCTestCase {
    func testReferenceOrbitBitExact() throws {
        let cases = try Fixture.load("orbit") as! [[String: Any]]
        for c in cases {
            let prec = (c["prec"] as! NSNumber).intValue
            let iters = (c["iterations"] as! NSNumber).intValue
            let center = try XCTUnwrap(BigComplex(re: c["re"] as! String, im: c["im"] as! String, precisionBits: prec))
            for (variant, key) in [(ReferenceOrbitComputer.Variant.squarings, "squarings"),
                                   (.multiplications, "multiplications")] {
                var comp = ReferenceOrbitComputer(c: center, precisionBits: prec)
                comp.variant = variant
                let (pts, ext, r) = comp.compute(maxIterations: iters, bailout: 2)
                let expectedCount = (c[key + "Count"] as! NSNumber).intValue
                XCTAssertEqual(r.iterations, expectedCount, "\(c["re"]!) \(key) iterations")
                let z = c[key] as! [Any?]
                assertBits(r.finalZ.re, z[0], prec: prec, "\(c["re"]!) @\(prec) \(key) re")
                assertBits(r.finalZ.im, z[1], prec: prec, "\(c["re"]!) @\(prec) \(key) im")
                // Float outputs agree with the full-precision final point.
                let last = pts[r.count - 1]
                XCTAssertEqual(last.x, Float(r.finalZ.re.doubleValue), accuracy: abs(last.x) * 1e-6 + 1e-30)
                XCTAssertEqual(last.y, Float(r.finalZ.im.doubleValue), accuracy: abs(last.y) * 1e-6 + 1e-30)
                let e = ext[r.count - 1].doubleValue
                XCTAssertEqual(e.x, r.finalZ.re.doubleValue, accuracy: abs(e.x) * 1e-6 + 1e-300)
                XCTAssertEqual(e.y, r.finalZ.im.doubleValue, accuracy: max(abs(e.x), abs(e.y)) * 1e-6 + 1e-300)
                // Against the high-precision orbit (prec + 256 bits).
                let high = c["high"] as! [String]
                let hre = BigFloat(high[0], precisionBits: prec + 256)!
                let him = BigFloat(high[1], precisionBits: prec + 256)!
                let err = max((r.finalZ.re - hre).log2Magnitude, (r.finalZ.im - him).log2Magnitude)
                print("orbit \(c["re"]!) @\(prec) bits \(key): \(r.iterations) iters, log2|error vs high| = \(err)")
            }
        }
    }

    func testOrbitBuffersAndCancellation() {
        let c = BigComplex(-0.1, 0.1, precisionBits: 256)
        let comp = ReferenceOrbitComputer(c: c, precisionBits: 256)
        var polls = 0
        var progressCalls: [Int] = []
        var pts = [SIMD2<Float>](repeating: .zero, count: 100_001)
        let r = pts.withUnsafeMutableBufferPointer { p in
            comp.compute(maxIterations: 100_000, points: p.baseAddress!, extended: nil, capacity: p.count,
                         checkInterval: 1000, isCancelled: { polls += 1; return polls >= 3 },
                         progress: { progressCalls.append($0) })
        }
        XCTAssertTrue(r.cancelled)
        XCTAssertEqual(r.iterations, 3000)
        XCTAssertEqual(progressCalls, [1000, 2000, 3000])
        // Double-precision cross-check of the first few points.
        var x = 0.0, y = 0.0
        for i in 1...20 {
            (x, y) = (x * x - y * y - 0.1, 2 * x * y + 0.1)
            XCTAssertEqual(pts[i].x, Float(x), accuracy: 1e-6)
            XCTAssertEqual(pts[i].y, Float(y), accuracy: 1e-6)
        }
    }

    func testEscapeStopsOrbit() {
        let c = BigComplex(0.5, 0.5, precisionBits: 128)
        let (pts, ext, r) = ReferenceOrbitComputer(c: c, precisionBits: 128).compute(maxIterations: 1000)
        XCTAssertTrue(r.escaped)
        XCTAssertEqual(pts.count, r.count)
        XCTAssertEqual(ext.count, r.count)
        XCTAssertGreaterThan(simd_length(SIMD2<Double>(pts.last!)), 2)
        XCTAssertLessThan(r.iterations, 10)
    }

    func testPrecisionRule() {
        // 1e-100 spacing, 2048 px: 333 + 11 + 2 + 48 -> 448 bits (7 limbs).
        let bits = ReferenceOrbitComputer.recommendedPrecisionBits(pixelSpacingLog2: -100 * 3.321928094887362,
                                                                   imageSize: 2048)
        XCTAssertEqual(bits, 448)
        XCTAssertEqual(ReferenceOrbitComputer.recommendedPrecisionBits(pixelSpacingLog2: -10, imageSize: 1000), 128)
    }
}

final class NucleusOracleTests: XCTestCase {
    func testNucleiAgainstMpmath() throws {
        let cases = try Fixture.load("nuclei") as! [[String: Any]]
        XCTAssertFalse(cases.isEmpty)
        for c in cases {
            let prec = (c["prec"] as! NSNumber).intValue
            let p = (c["period"] as! NSNumber).intValue
            let start = try XCTUnwrap(BigComplex(re: c["re"] as! String, im: c["im"] as! String, precisionBits: prec))
            let radius = BigFloat(c["radius"] as! String, precisionBits: 64)!
            let found = PeriodDetection.findPeriod(center: start, radius: radius, maxPeriod: 200_000)
            XCTAssertEqual(found, (c["ballPeriod"] as? NSNumber)?.intValue, "ball period at \(c["re"]!)")
            let nr = try XCTUnwrap(NucleusFinder.findNucleus(near: start, period: p), "newton \(c["re"]!)")
            let ere = BigFloat(c["nucleusRe"] as! String, precisionBits: prec + 128)!
            let eim = BigFloat(c["nucleusIm"] as! String, precisionBits: prec + 128)!
            let err = max((nr.nucleus.re - ere).log2Magnitude, (nr.nucleus.im - eim).log2Magnitude)
            print("nucleus p=\(p) @\(prec) bits: \(nr.steps) Newton steps, log2|error| = \(err)")
            XCTAssertLessThan(err, Double(-prec + 24), "nucleus accuracy p=\(p)")
            // The nucleus orbit returns to ~0 at iteration p; the extended form keeps the tiny
            // Z_p that float32 flushes to zero.
            let (pts, ext, orbit) = ReferenceOrbitComputer(c: nr.nucleus, precisionBits: prec).compute(maxIterations: p)
            XCTAssertEqual(orbit.iterations, p)
            let zLog2 = max(orbit.finalZ.re.log2Magnitude, orbit.finalZ.im.log2Magnitude)
            XCTAssertLessThan(zLog2, Double(-prec + 2 * p + 24), "Z_p ~ 0 for p=\(p)")
            if !orbit.finalZ.re.isZero, zLog2 < -200 {
                XCTAssertEqual(pts[p], .zero)
                XCTAssertEqual(Double(ext[p].exponent), zLog2.rounded(.down) + 1, accuracy: 1)
            }
        }
    }

    func testKnownRealNuclei() throws {
        let p3 = try XCTUnwrap(NucleusFinder.findNucleus(near: BigComplex(-1.75, 0, precisionBits: 128), period: 3))
        XCTAssertTrue(p3.nucleus.re.toDecimalString(significantDigits: 17).hasPrefix("-1.754877666246692"))
        XCTAssertTrue(p3.nucleus.im.isZero || p3.nucleus.im.log2Magnitude < -120)
        let p4 = try XCTUnwrap(NucleusFinder.findNucleus(near: BigComplex(-1.94, 0, precisionBits: 128), period: 4))
        XCTAssertTrue(p4.nucleus.re.toDecimalString(significantDigits: 17).hasPrefix("-1.940799806529484"))
        // Z_p at the nucleus is ~0 at working precision.
        let (_, _, r) = ReferenceOrbitComputer(c: p4.nucleus, precisionBits: 128).compute(maxIterations: 4)
        XCTAssertLessThan(max(r.finalZ.re.log2Magnitude, r.finalZ.im.log2Magnitude), -110)
    }

    func testMinibrotSizeEstimate() {
        // Period 2 (the disc at -1) gives 0.5 with this normalisation; period-3 minibrot ~0.019.
        XCTAssertEqual(NucleusFinder.minibrotSize(nucleus: BigComplex(-1, 0, precisionBits: 128), period: 2).doubleValue,
                       0.5, accuracy: 1e-12)
        let p3 = NucleusFinder.findNucleus(near: BigComplex(-1.75, 0, precisionBits: 128), period: 3)!
        let size = NucleusFinder.minibrotSize(nucleus: p3.nucleus, period: 3).doubleValue
        print("period-3 minibrot size estimate \(size)")
        XCTAssertGreaterThan(size, 0.01)
        XCTAssertLessThan(size, 0.03)
    }

    func testBoxAndFirstOrderPeriods() {
        let c = BigComplex(-1.75, 0.001, precisionBits: 128)
        let r = BigFloat(0.01, precisionBits: 64)
        XCTAssertEqual(PeriodDetection.findPeriod(center: c, radius: r, maxPeriod: 1000, method: .box), 3)
        XCTAssertEqual(PeriodDetection.findPeriod(center: c, radius: r, maxPeriod: 1000, method: .ballFirstOrder), 3)
        XCTAssertEqual(PeriodDetection.findPeriod(center: c, radius: r, maxPeriod: 1000, method: .ball), 3)
    }

    func testReferenceSelector() {
        let view = BigComplex(-1.752, 0.0005, precisionBits: 128)
        let choice = ReferenceSelector.selectReference(center: view, radius: BigFloat(0.01, precisionBits: 64),
                                                       maxIterations: 1000)
        XCTAssertTrue(choice.isNucleus)
        XCTAssertEqual(choice.period, 3)
        // Outside the set: no period, falls back to the centre.
        let out = BigComplex(0.5, 0.5, precisionBits: 128)
        let c2 = ReferenceSelector.selectReference(center: out, radius: BigFloat(1e-6, precisionBits: 64), maxIterations: 1000)
        XCTAssertFalse(c2.isNucleus)
        XCTAssertEqual(c2.point, out)
    }
}
