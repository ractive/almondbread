// almondbread-core-check: headless proof of the Rust core pipeline (kb: platform/rust-swift-ffi).
//
//   swift run -c release almondbread-core-check        (make core-check)
//
// Calls the Rust core through the UniFFI bindings (AlmondbreadCore) and checks every result
// against the Swift implementation (AlmondbreadMath): coordinate parse/format, reference
// orbits bit for bit (final Z, float32 and floatexp points), reference selection, and
// cancellation. Then measures the FFI call overhead. Exits non-zero on any mismatch.

import AlmondbreadCore
import AlmondbreadMath
import Dispatch
import Foundation

var failures = 0
func check(_ ok: Bool, _ what: @autoclosure () -> String) {
    if !ok { failures += 1; print("FAIL: \(what())") }
}

func now() -> UInt64 { DispatchTime.now().uptimeNanoseconds }

print(coreVersion())
let layout = floatExpComplexLayout()
check(layout == [UInt32(MemoryLayout<FloatExpComplex>.stride), UInt32(MemoryLayout<FloatExpComplex>.alignment)],
      "FloatExpComplex layout Rust \(layout) vs Swift \(MemoryLayout<FloatExpComplex>.stride)/\(MemoryLayout<FloatExpComplex>.alignment)")

// 1. Coordinates: Rust parse+format must equal Swift's, digit for digit.
let texts = ["-0.743643887037158704752191506114774", "0.131825904205311970493132056385139",
             "1e-300", "-2.5", "0.1", "123456789.000000001", "-1.0000000000000000000000000000001e-50"]
for bits in [64, 128, 320, 1024] {
    for t in texts {
        for digits in [nil, 5, 40] as [Int?] {
            let r = try normalizeCoordinate(re: t, im: t, precisionBits: UInt32(bits), digits: digits.map(UInt32.init))
            let s = BigFloat(t, precisionBits: bits)!.toDecimalString(significantDigits: digits)
            check(r.re == s && r.im == s, "format \(t) @\(bits) digits \(String(describing: digits)): rust \(r.re) swift \(s)")
        }
    }
}
do {
    _ = try normalizeCoordinate(re: "1.2.3", im: "0", precisionBits: 64, digits: nil)
    check(false, "malformed coordinate accepted")
} catch CoreError.Malformed {
} catch {
    check(false, "unexpected error \(error)")
}
check(recommendedPrecisionBits(pixelSpacingLog2: -100 * 3.321928094887362, imageSize: 2048) ==
      UInt32(ReferenceOrbitComputer.recommendedPrecisionBits(pixelSpacingLog2: -100 * 3.321928094887362, imageSize: 2048)),
      "precision rule")

// 2. Reference orbits: identical bits in both languages.
let orbitCases: [(String, String, Int, Int)] = [
    ("-0.743643887037158704752191506114774", "0.131825904205311970493132056385139", 192, 20_000),
    ("-1.7548776662466927600495088963585286918946066177727931500", "0", 256, 5_000),
    ("0.2500000000000000000001", "0.0000000000000000000001", 64, 3_000),
    ("-0.75", "0.1", 128, 1_000),           // escapes
    ("-0.5", "0.5", 2048, 2_000),
]
for (re, im, bits, iters) in orbitCases {
    let rust = try computeReferenceOrbit(re: re, im: im, precisionBits: UInt32(bits), maxIterations: UInt64(iters),
                                         bailout: 2, cancel: nil)
    let c = BigComplex(re: re, im: im, precisionBits: bits)!
    let (pts, ext, sw) = ReferenceOrbitComputer(c: c, precisionBits: bits).compute(maxIterations: iters, bailout: 2)
    let n = Int(rust.count())
    check(n == sw.count && rust.escaped() == sw.escaped, "orbit \(re) @\(bits): count rust \(n) swift \(sw.count)")
    let fz = rust.finalZ(digits: nil)
    check(fz.re == sw.finalZ.re.description && fz.im == sw.finalZ.im.description, "orbit \(re) @\(bits): final Z differs")
    var rpts = [SIMD2<Float>](repeating: .zero, count: n)
    var rext = [FloatExpComplex](repeating: .zero, count: n)
    let cp = rpts.withUnsafeMutableBytes { rust.copyPoints(into: $0) }
    let ce = rext.withUnsafeMutableBytes { rust.copyExtended(into: $0) }
    check(cp == n && ce == n, "orbit copy counts")
    let flat = rust.pointsInterleaved()
    var same = flat.count == 2 * n
    for i in 0..<min(n, sw.count) where same {
        same = rpts[i] == pts[i] && rext[i] == ext[i] && flat[2 * i] == pts[i].x && flat[2 * i + 1] == pts[i].y
        if !same { print("  first difference at point \(i): \(rpts[i]) \(rext[i]) vs \(pts[i]) \(ext[i])") }
    }
    check(same, "orbit \(re) @\(bits): points differ")
}

// 3. Reference selection.
let center = ("-0.743643887037158704752191506114774", "0.131825904205311970493132056385139")
for radius in ["1e-10", "1e-20", "1e-25"] {
    let r = try selectReference(re: center.0, im: center.1, radius: radius, maxIterations: 100_000, precisionBits: 192,
                                method: .ball, cancel: nil)
    let c = BigComplex(re: center.0, im: center.1, precisionBits: 192)!
    let s = ReferenceSelector.selectReference(center: c, radius: BigFloat(radius, precisionBits: 64)!,
                                              maxIterations: 100_000, precisionBits: 192)
    check(r.isNucleus == s.isNucleus && r.period.map(Int.init) == s.period && r.newtonSteps == UInt32(s.newtonSteps)
          && r.point.re == s.point.re.description && r.point.im == s.point.im.description,
          "selectReference radius \(radius): rust p=\(String(describing: r.period)) swift p=\(String(describing: s.period))")
    print("reference radius \(radius): nucleus=\(r.isNucleus) period=\(r.period.map(String.init) ?? "-") steps=\(r.newtonSteps)")
}

// 4. Cancellation from another thread.
let token = CancelToken()
DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(20)) { token.cancel() }
let cancelled = try computeReferenceOrbit(re: "-0.1", im: "0.1", precisionBits: 4096, maxIterations: 100_000_000,
                                          bailout: 2, cancel: token)
check(cancelled.cancelled() && cancelled.count() < 100_000_001 && token.progress() > 0,
      "cancellation: cancelled=\(cancelled.cancelled()) count=\(cancelled.count())")

// 5. FFI overhead (best of 5 rounds).
func best(_ rounds: Int = 5, _ body: () throws -> Void) rethrows -> Double {
    var b = Double.infinity
    for _ in 0..<rounds {
        let t0 = now()
        try body()
        b = min(b, Double(now() - t0))
    }
    return b
}
let calls = 1_000_000
var acc: UInt64 = 0
let tPing = best { for i in 0..<calls { acc &+= ping(x: UInt64(i)) } } / Double(calls)
let tVersion = best { for _ in 0..<100_000 { acc &+= UInt64(coreVersion().utf8.count) } } / 100_000
let tTiny = try best { for _ in 0..<10_000 {
    let o = try computeReferenceOrbit(re: "-0.1", im: "0.1", precisionBits: 64, maxIterations: 1, bailout: 2, cancel: nil)
    acc &+= o.count()
} } / 10_000
let big = try computeReferenceOrbit(re: "-0.1", im: "0.1", precisionBits: 64, maxIterations: 999_999, bailout: 2, cancel: nil)
var buf = [FloatExpComplex](repeating: .zero, count: 1_000_000)
let tCopy = best { buf.withUnsafeMutableBytes { acc &+= UInt64(big.copyExtended(into: $0)) } }
let tVec = best { acc &+= UInt64(big.pointsInterleaved().count) }
// Swift-only reference for the tiny call: the same work without the FFI.
let tSwiftTiny = best { for _ in 0..<10_000 {
    let r = ReferenceOrbitComputer(c: BigComplex(re: "-0.1", im: "0.1", precisionBits: 64)!, precisionBits: 64)
        .compute(maxIterations: 1)
    acc &+= UInt64(r.result.count)
} } / 10_000
print(String(format: "ffi: ping %.1f ns/call, version string %.0f ns/call, 1-iteration orbit %.2f us (Swift-only %.2f us)",
             tPing, tVersion, tTiny / 1000, tSwiftTiny / 1000))
print(String(format: "ffi: 1M-point floatexp copy %.2f ms (%.1f GB/s), 1M-point Vec<f32> return %.2f ms",
             tCopy / 1e6, 16e6 / tCopy, tVec / 1e6))
if acc == 42 { print("") }  // keep the work observable

if failures == 0 {
    print("core-check OK")
} else {
    print("core-check: \(failures) failure(s)")
    exit(1)
}
