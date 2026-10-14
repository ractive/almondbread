// almondbread-math-check: benchmarks and smoke checks for AlmondbreadMath.
//
//   swift run -c release almondbread-math-check bench [bits...]  reference-orbit iterations/s per precision
//   swift run -c release almondbread-math-check nucleus    period detection + Newton at a deep location
//   swift run -c release almondbread-math-check ops        BigFloat op timings
//   swift run -c release almondbread-math-check deep [n]   n levels of minibrot-in-minibrot Newton zoom
//
// Pure Swift; the GMP comparison lives in Tests/AlmondbreadMathTests/oracle/gmp_orbit_bench.c.

import AlmondbreadMath
import Dispatch

func seconds(_ body: () -> Void) -> Double {
    let t0 = DispatchTime.now().uptimeNanoseconds
    body()
    return Double(DispatchTime.now().uptimeNanoseconds - t0) * 1e-9
}

/// A C with all mantissa bits populated, inside the main cardioid (no escape).
func benchPoint(bits: Int) -> BigComplex {
    var re = BigFloat("-0.50000000000000000000123456789012345678901234567890123456789", precisionBits: bits)!
    let im = BigFloat("0.500000000000000000000987654321098765432109876543210987654321", precisionBits: bits)!
    re += BigFloat(0.123456789, precisionBits: bits).timesPowerOfTwo(-(max(bits, 80) - 8))
    return BigComplex(re: re, im: im)
}

func benchOrbit(bits: Int, variant: ReferenceOrbitComputer.Variant, outputs: Bool) -> Double {
    var comp = ReferenceOrbitComputer(c: benchPoint(bits: bits), precisionBits: bits)
    comp.variant = variant
    var iters = 1000
    while true {
        var pts = [SIMD2<Float>](repeating: .zero, count: outputs ? iters + 1 : 0)
        var ext = [FloatExpComplex](repeating: .zero, count: outputs ? iters + 1 : 0)
        var res: ReferenceOrbitComputer.Result?
        let t = seconds {
            pts.withUnsafeMutableBufferPointer { pp in
                ext.withUnsafeMutableBufferPointer { ep in
                    res = comp.compute(maxIterations: iters, points: outputs ? pp.baseAddress : nil,
                                       extended: outputs ? ep.baseAddress : nil, capacity: iters + 1)
                }
            }
        }
        precondition(!res!.escaped, "benchmark orbit escaped")
        precondition(res!.iterations == iters, "benchmark orbit stopped early")
        if t > 0.4 || iters > 50_000_000 { return Double(iters) / t }
        iters *= 4
    }
}

func pad(_ s: String, _ w: Int) -> String { s.count >= w ? s : String(repeating: " ", count: w - s.count) + s }

func runBench() {
    print("Reference orbit, iterations per second (release, single thread)")
    print(pad("bits", 6), pad("sqr+out", 12), pad("mul+out", 12), pad("sqr", 12), pad("mul", 12))
    let sizes = CommandLine.arguments.count > 2 ? CommandLine.arguments.dropFirst(2).map { Int($0)! }
        : [64, 128, 192, 256, 384, 512, 768, 1024, 2048, 4096, 8192, 16384]
    for bits in sizes {
        let a = benchOrbit(bits: bits, variant: .squarings, outputs: true)
        let b = benchOrbit(bits: bits, variant: .multiplications, outputs: true)
        let c = benchOrbit(bits: bits, variant: .squarings, outputs: false)
        let d = benchOrbit(bits: bits, variant: .multiplications, outputs: false)
        func f(_ v: Double) -> String { pad(String(Int(v)), 12) }
        print(pad(String(bits), 6), f(a), f(b), f(c), f(d))
    }
}

func runOps() {
    for bits in [128, 512, 4096] {
        let a = benchPoint(bits: bits).re, b = benchPoint(bits: bits).im
        var x = a
        let n = 200_000 / max(1, bits / 128)
        let tm = seconds { for _ in 0..<n { x = a * b } }
        let ta = seconds { for _ in 0..<n { x = a + b } }
        let td = seconds { for _ in 0..<(n / 10) { x = a / b } }
        let tf = seconds { for _ in 0..<(n / 100) { _ = x.description } }
        let tp = seconds { for _ in 0..<(n / 100) { x = BigFloat(a.description, precisionBits: bits)! } }
        print("\(bits) bits: mul \(Int(tm / Double(n) * 1e9)) ns, add \(Int(ta / Double(n) * 1e9)) ns, div \(Int(td / Double(n / 10) * 1e9)) ns, format \(Int(tf / Double(n / 100) * 1e6)) us, parse \(Int(tp / Double(n / 100) * 1e6)) us (allocating operators)")
    }
}

func runNucleus() {
    // Seahorse valley: find the minibrot inside a 1e-25 view and its nucleus.
    let bits = 192
    let center = BigComplex(re: "-0.743643887037158704752191506114774", im: "0.131825904205311970493132056385139",
                            precisionBits: bits)!
    for radius in ["1e-10", "1e-20", "1e-25", "1e-32"] {
        let r = BigFloat(radius, precisionBits: 64)!
        var choice: ReferenceSelector.Choice?
        let t = seconds {
            choice = ReferenceSelector.selectReference(center: center, radius: r, maxIterations: 100_000,
                                                       precisionBits: bits)
        }
        let c = choice!
        print("radius \(radius): nucleus=\(c.isNucleus) period=\(c.period.map(String.init) ?? "-") detected=\(c.detectedPeriod.map(String.init) ?? "-") steps=\(c.newtonSteps) size=\(c.minibrotSize.map { "\($0)" } ?? "-") in \(String(format3(t))) s")
        if c.isNucleus { print("  re \(c.point.re.toDecimalString(significantDigits: 45))\n  im \(c.point.im.toDecimalString(significantDigits: 45))") }
    }
}

/// Zooms into successively deeper minibrots: from nucleus N with complex size s, look near
/// N + s*w (w in the mini-set's seahorse valley) within radius |s|*1e-3, then Newton.
func runDeep(levels: Int) {
    var bits = 256
    var c = BigComplex(re: "-0.743643887037158704752191506114774", im: "0.131825904205311970493132056385139",
                       precisionBits: bits)!
    var radius = BigFloat("1e-25", precisionBits: 64)!
    let w = DoubleExpComplex(re: DoubleExp(-0.7436), im: DoubleExp(0.1318))
    for level in 0..<levels {
        var choice: ReferenceSelector.Choice?
        let t = seconds {
            choice = ReferenceSelector.selectReference(center: c, radius: radius, maxIterations: 10_000_000, precisionBits: bits)
        }
        guard let ch = choice, ch.isNucleus, let p = ch.period else { print("level \(level): no nucleus, detected \(String(describing: choice?.detectedPeriod)) at re=\(c.re) im=\(c.im) r=\(radius)"); return }
        let sc = NucleusFinder.minibrotSizeComplex(nucleus: ch.point, period: p)
        print("level \(level): start re=\(c.re.toDecimalString(significantDigits: bits * 3 / 10)) im=\(c.im.toDecimalString(significantDigits: bits * 3 / 10)) radius=\(radius.toDecimalString(significantDigits: 3)) bits=\(bits) -> period \(p), size \(sc.magnitude), \(ch.newtonSteps) Newton steps, \(format3(t)) s")
        // Next start: N + s * w.
        let off = sc * w
        let size = sc.magnitude
        bits = max(bits, Int(-size.log2Magnitude) + 96) / 64 * 64 + 64
        let n = ch.point.withPrecision(bits: bits)
        let offRe = BigFloat(off.re.mantissa, precisionBits: bits).timesPowerOfTwo(Int(off.re.exponent))
        let offIm = BigFloat(off.im.mantissa, precisionBits: bits).timesPowerOfTwo(Int(off.im.exponent))
        c = BigComplex(re: n.re + offRe, im: n.im + offIm)
        radius = BigFloat(size.mantissa * 1e-3, precisionBits: 64).timesPowerOfTwo(Int(size.exponent))
    }
}

func format3(_ v: Double) -> String { String(Double(Int(v * 1000)) / 1000) }

let args = CommandLine.arguments.dropFirst()
switch args.first ?? "bench" {
case "bench": runBench()
case "ops": runOps()
case "nucleus": runNucleus()
case "deep": runDeep(levels: Int(CommandLine.arguments.dropFirst(2).first ?? "4") ?? 4)
default: print("usage: almondbread-math-check [bench [bits...]|ops|nucleus|deep [levels]]")
}
