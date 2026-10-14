import Foundation

/// Per-pixel iteration results in the GPU's iteration-texture encoding, so GPU and CPU renders
/// compare value by value: `n` is the escape iteration (-1: interior or maxIter, -2: not
/// computed), `s` the smooth offset, smooth count `mu = n + s`
/// (kb: techniques/smooth-coloring). For interior pixels `s` is 1 when the GPU's derivative
/// test fired, 0 when the pixel ran to maxIter.
public struct IterationField: Sendable {
    public let width: Int
    public let height: Int
    public var n: [Float]
    public var s: [Float]

    public init(width: Int, height: Int, n: [Float], s: [Float]) {
        self.width = width; self.height = height; self.n = n; self.s = s
    }

    public init(width: Int, height: Int, texels: [SIMD2<Float>]) {
        self.init(width: width, height: height, n: texels.map(\.x), s: texels.map(\.y))
    }

    /// Raw dump for before/after comparisons: width, height (Int32) then (n, s) Float32 pairs.
    public func write(to path: String) throws {
        var data = Data()
        withUnsafeBytes(of: Int32(width)) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: Int32(height)) { data.append(contentsOf: $0) }
        for k in 0..<n.count {
            withUnsafeBytes(of: n[k]) { data.append(contentsOf: $0) }
            withUnsafeBytes(of: s[k]) { data.append(contentsOf: $0) }
        }
        try data.write(to: URL(fileURLWithPath: path))
    }

    public init(contentsOf path: String) throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let (w, h, pairs) = data.withUnsafeBytes { raw -> (Int, Int, [Float]) in
            let w = Int(raw.loadUnaligned(fromByteOffset: 0, as: Int32.self))
            let h = Int(raw.loadUnaligned(fromByteOffset: 4, as: Int32.self))
            let count = (raw.count - 8) / 4
            return (w, h, (0..<count).map { raw.loadUnaligned(fromByteOffset: 8 + 4 * $0, as: Float.self) })
        }
        guard pairs.count == 2 * w * h else { throw CocoaError(.fileReadCorruptFile) }
        self.init(width: w, height: h, n: stride(from: 0, to: pairs.count, by: 2).map { pairs[$0] },
                  s: stride(from: 1, to: pairs.count, by: 2).map { pairs[$0] })
    }
}

/// CPU oracle: direct Double iteration z ← z² + c from z = 0, no perturbation, no interior
/// test, same pixel grid, bailout and smooth formula as the GPU kernel. Float32 errors in the
/// GPU path are silent, so this is the ground truth the verifier diffs against
/// (kb: questions/float32-error-long-orbits). Valid while Double resolves the pixel spacing,
/// i.e. scales above about 1e-15 (the same limit as the Double reference orbit).
public enum CPUReference {
    public static func render(centerRe: Double, centerIm: Double, scale: Double,
                              width: Int, height: Int, maxIter: Int, bailout: Double = 65536) -> IterationField {
        var n = [Float](repeating: 0, count: width * height)
        var s = [Float](repeating: 0, count: width * height)
        let b2 = bailout * bailout
        let halfW = Double(width) / 2, halfH = Double(height) / 2
        n.withUnsafeMutableBufferPointer { nb in
            s.withUnsafeMutableBufferPointer { sb in
                // Buffers are disjoint per row, so concurrent writes are safe.
                let np = nb.baseAddress!, sp = sb.baseAddress!
                DispatchQueue.concurrentPerform(iterations: height) { y in
                    // Same sample position as the kernel: texel centre, y flipped.
                    let ci = centerIm - (Double(y) + 0.5 - halfH) * scale
                    for x in 0..<width {
                        let cr = centerRe + (Double(x) + 0.5 - halfW) * scale
                        var zr = 0.0, zi = 0.0, r2 = 0.0
                        var i = 0
                        while i < maxIter {
                            let t = zr * zr - zi * zi + cr
                            zi = 2 * zr * zi + ci
                            zr = t
                            i += 1
                            r2 = zr * zr + zi * zi
                            if r2 > b2 { break }
                        }
                        let k = y * width + x
                        if r2 > b2 {
                            np[k] = Float(i)
                            sp[k] = Float(1 - log2(0.5 * log(r2)))
                        } else {
                            np[k] = -1
                            sp[k] = 0
                        }
                    }
                }
            }
        }
        return IterationField(width: width, height: height, n: n, s: s)
    }
}

/// GPU-vs-CPU diff metrics (kb: synthesis/benchmarks).
public struct VerifyReport: Sendable {
    public var pixels = 0
    /// Both exterior, escape iteration n differs.
    public var escapeDiffers = 0
    /// GPU says interior, CPU escapes (derivative-test false positives or float error).
    /// Unresolved GPU pixels (n = -3, drawn as unresolved, not as interior) are not counted.
    public var falseInterior = 0
    /// GPU unresolved (past the nominal limit, not proven interior) where the CPU escapes.
    public var unresolvedExterior = 0
    /// GPU unresolved pixels.
    public var unresolved = 0
    /// GPU escapes, CPU says interior.
    public var falseExterior = 0
    /// GPU interior pixels found by the derivative test (s == 1) rather than maxIter.
    public var detectedInterior = 0
    public var cpuInterior = 0
    /// Smooth-count error |Δmu| over pixels exterior in both.
    public var maxError = 0.0
    public var medianError = 0.0
    public var p99Error = 0.0
    /// Pixels (both exterior) with |Δmu| > 0.05.
    public var over005 = 0

    func pct(_ k: Int) -> Double { pixels > 0 ? 100 * Double(k) / Double(pixels) : 0 }
    public var escapeDiffersPct: Double { pct(escapeDiffers) }
    public var mismatchPct: Double { pct(falseInterior + falseExterior) }
    public var falseInteriorPct: Double { pct(falseInterior) }
    public var unresolvedPct: Double { pct(unresolved) }
    public var falseExteriorPct: Double { pct(falseExterior) }
    public var over005Pct: Double { pct(over005) }

    public var summary: String {
        String(format: "pixels %d  escape-n differs %.3f%%  class mismatch %.3f%% (GPU-int/CPU-ext %.3f%%, GPU-ext/CPU-int %.3f%%)  "
               + "|Δmu| max %.3g median %.3g p99 %.3g  >0.05: %.3f%%  interior CPU %.1f%%, GPU detected %.1f%%  "
               + "unresolved %.3f%% (CPU escapes %.3f%%)",
               pixels, escapeDiffersPct, mismatchPct, falseInteriorPct, falseExteriorPct,
               maxError, medianError, p99Error, over005Pct, pct(cpuInterior), pct(detectedInterior),
               unresolvedPct, pct(unresolvedExterior))
    }
}

public enum Verifier {
    /// Compares `gpu` against `cpu` and renders a heatmap (BGRA8, sRGB): exterior pixels in a
    /// log ramp of |Δmu| from 1e-4 (black) through red to yellow-white at ≥1, false interior
    /// magenta, false exterior cyan, agreed interior dark grey; GPU unresolved blue (CPU
    /// escapes) or dark blue (CPU interior).
    public static func compare(gpu: IterationField, cpu: IterationField) -> (VerifyReport, [UInt8]) {
        precondition(gpu.width == cpu.width && gpu.height == cpu.height)
        let count = gpu.width * gpu.height
        var r = VerifyReport()
        r.pixels = count
        var errors: [Double] = []
        errors.reserveCapacity(count)
        var img = [UInt8](repeating: 0, count: count * 4)
        func put(_ k: Int, _ red: Double, _ green: Double, _ blue: Double) {
            img[4 * k] = UInt8(max(0, min(255, blue * 255)))
            img[4 * k + 1] = UInt8(max(0, min(255, green * 255)))
            img[4 * k + 2] = UInt8(max(0, min(255, red * 255)))
            img[4 * k + 3] = 255
        }
        for k in 0..<count {
            let gIn = gpu.n[k] < 0, cIn = cpu.n[k] < 0
            if cIn { r.cpuInterior += 1 }
            if cpu.n[k] == -3 {
                // The reference field is a GPU render too (`--fields`, fresh renders): its
                // unresolved pixels are unknown, not a class.
                put(k, 0.05, 0.05, 0.35)
                continue
            }
            if gpu.n[k] == -3 {
                // Unresolved: not drawn as interior, so neither a false interior nor exterior.
                r.unresolved += 1
                if !cIn { r.unresolvedExterior += 1; put(k, 0.2, 0.3, 1) } else { put(k, 0.05, 0.05, 0.35) }
                continue
            }
            if gIn && gpu.s[k] == 1 { r.detectedInterior += 1 }
            switch (gIn, cIn) {
            case (true, true):
                put(k, 0.12, 0.12, 0.12)
            case (true, false):
                r.falseInterior += 1
                put(k, 1, 0, 1)
            case (false, true):
                r.falseExterior += 1
                put(k, 0, 1, 1)
            case (false, false):
                if gpu.n[k] != cpu.n[k] { r.escapeDiffers += 1 }
                let e = abs((Double(gpu.n[k]) + Double(gpu.s[k])) - (Double(cpu.n[k]) + Double(cpu.s[k])))
                errors.append(e)
                if e > 0.05 { r.over005 += 1 }
                let t = max(0, min(1, (log10(max(e, 1e-6)) + 4) / 4))   // 1e-4 → 0, 1 → 1
                put(k, min(1, 2 * t), max(0, 2 * t - 1), max(0, 3 * t - 2))
            }
        }
        if !errors.isEmpty {
            errors.sort()
            r.maxError = errors.last!
            r.medianError = errors[errors.count / 2]
            r.p99Error = errors[min(errors.count - 1, errors.count * 99 / 100)]
        }
        return (r, img)
    }
}
