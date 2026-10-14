import AlmondbreadKit
import Foundation

/// `--bench`: renders the fixed benchmark set (BenchmarkLocation.all) and prints a markdown
/// table of timings plus GPU-vs-CPU verify metrics at a small size (kb: synthesis/benchmarks).
///   AlmondbreadMac --bench [--sizes 3456x2234,1600x1000] [--verify-size 400x259]
///                  [--only home,feigenbaum] [--repeat 2] [--out dir] [--interior 0|1] [--no-verify 1]
/// Exits 1 if any location paints more than 0.02 % CPU-exterior pixels as interior.
@MainActor
enum BenchCommand {
    static func run(arguments: [String]) -> Int32 {
        do {
            let options = try CommandOptions(arguments: arguments)
            let sizes = try (options["sizes"] ?? "3456x2234,1600x1000").split(separator: ",")
                .map { try CommandOptions.parseSize(String($0)) }
            let verifySize = try options.size(default: 400, 259, key: "verify-size")
            let only = options["only"].map { Set($0.split(separator: ",").map(String.init)) }
            let repeats = max(1, options["repeat"].flatMap(Int.init) ?? 2)
            let outDir = options["out"]
            let doVerify = options["no-verify"] != "1"

            let model = FractalModel()
            options.apply(to: model)
            model.autoIterations = false
            guard let renderer = Renderer(model: model) else { throw OffscreenRender.Failure.noMetal }
            options.apply(to: renderer)
            if let outDir { try FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true) }

            print("| location | size | maxIter | GPU ms | total ms | chunks | px-iter | done-iter | Giter/s | ref ms |")
            print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|")
            var verifyRows: [String] = []
            var falseInterior: [String] = []
            for loc in BenchmarkLocation.all where only?.contains(loc.name) ?? true {
                model.manualIterations = loc.maxIterations
                model.maxIterationsCap = max(model.maxIterationsCap, loc.maxIterations)
                for (w, h) in sizes {
                    model.viewport = Viewport(centerRe: loc.centerRe, centerIm: loc.centerIm, scale: loc.scale(forWidth: w))
                    // Warm-up builds the reference orbit and textures; then best of `repeats`.
                    var last: Renderer.OffscreenResult?
                    var gpuMs = Double.infinity, wallMs = Double.infinity
                    for i in 0..<repeats {
                        let r = try OffscreenRender.render(renderer: renderer, width: w, height: h,
                                                           warmup: i == 0 ? 1 : 0, readback: i == repeats - 1)
                        gpuMs = min(gpuMs, r.iterateGPUTime * 1000)
                        wallMs = min(wallMs, r.wallTime * 1000)
                        last = r
                    }
                    guard let r = last else { continue }
                    var pxIter = 0.0
                    for t in r.iterations ?? [] { pxIter += t.x >= 0 ? Double(t.x) : Double(loc.maxIterations) }
                    let done = r.iterationsDone > 0 ? String(format: "%.3g", Double(r.iterationsDone)) : "-"
                    let rate = (r.iterationsDone > 0 ? Double(r.iterationsDone) : pxIter) / gpuMs / 1e6
                    print(String(format: "| %@ | %dx%d | %d | %.1f | %.1f | %d | %.3g | %@ | %.1f | %.2f |",
                                 loc.name, w, h, loc.maxIterations, gpuMs, wallMs,
                                 r.chunks, pxIter, done, rate, model.lastReferenceTime * 1000))
                    if let outDir {
                        try OffscreenRender.writePNG(bgra: r.pixels, width: w, height: h, to: "\(outDir)/\(loc.name)-\(w).png")
                        if let t = r.iterations {
                            try IterationField(width: t.count / h, height: h, texels: t).write(to: "\(outDir)/\(loc.name)-\(w).iter")
                        }
                    }
                }
                if doVerify {
                    let (w, h) = verifySize
                    model.viewport = Viewport(centerRe: loc.centerRe, centerIm: loc.centerIm, scale: loc.scale(forWidth: w))
                    let v = try VerifyCommand.verify(renderer: renderer, width: w, height: h)
                    let m = v.report
                    // Exterior painted as interior is a bug, not float noise (which splits
                    // evenly both ways): fail the run (kb: techniques/interior-detection).
                    if m.falseInteriorPct > 0.02 {
                        falseInterior.append(String(format: "%@ %.3f%%", loc.name, m.falseInteriorPct))
                    }
                    verifyRows.append(String(format: "| %@ | %dx%d | %.3f | %.3f | %.3f | %.3f | %.3g | %.3g | %.3g | %.3f | %.1f | %.1f |",
                                             loc.name, w, h, m.escapeDiffersPct, m.mismatchPct, m.falseInteriorPct,
                                             m.falseExteriorPct, m.maxError, m.medianError, m.p99Error, m.over005Pct,
                                             100 * Double(m.cpuInterior) / Double(max(1, m.pixels)),
                                             100 * Double(m.detectedInterior) / Double(max(1, m.pixels))))
                    if let outDir {
                        try OffscreenRender.writePNG(bgra: v.heatmap, width: w, height: h, to: "\(outDir)/\(loc.name)-diff.png")
                        try v.gpu.write(to: "\(outDir)/\(loc.name)-verify.iter")
                    }
                }
            }
            if doVerify {
                print("")
                print("| location | verify size | escape-n differs % | class mismatch % | GPU-int/CPU-ext % | GPU-ext/CPU-int % | max abs Δmu | median | p99 | >0.05 % | CPU interior % | GPU detected % |")
                print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
                verifyRows.forEach { print($0) }
            }
            if !falseInterior.isEmpty {
                fail("bench FAILED: exterior pixels painted interior (GPU-int/CPU-ext > 0.02%): "
                     + falseInterior.joined(separator: ", "))
                return 1
            }
            return 0
        } catch {
            fail("\(error)")
            return 1
        }
    }
}
