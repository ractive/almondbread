import AlmondbreadKit
import Foundation

/// `--verify`: renders a view on the GPU and with the CPU Double oracle and reports the diff
/// (kb: questions/float32-error-long-orbits), e.g.
///   AlmondbreadMac --verify --center -0.7436,0.1318 --scale 1e-9 --size 400x260 --iterations 5000
///                  [--heatmap diff.png] [--out gpu.png] [--interior 0] [--dump gpu.iter]
/// `--verify --fields a.iter --against b.iter` diffs two dumps instead (b plays the reference),
/// e.g. the same view before and after a renderer change.
@MainActor
enum VerifyCommand {
    static func run(arguments: [String]) -> Int32 {
        do {
            let options = try CommandOptions(arguments: arguments)
            if let a = options["fields"], let b = options["against"] {
                let fa = try IterationField(contentsOf: a), fb = try IterationField(contentsOf: b)
                guard fa.width == fb.width, fa.height == fb.height else { throw CommandError("size mismatch") }
                let (report, heatmap) = Verifier.compare(gpu: fa, cpu: fb)
                print(report.summary)
                if let path = options["heatmap"] {
                    try OffscreenRender.writePNG(bgra: heatmap, width: fa.width, height: fa.height, to: path)
                }
                return 0
            }
            if options["history"] != nil { return try history(options: options) }
            let (width, height) = try options.size(default: 400, 260)
            let model = FractalModel()
            model.viewport = try options.viewport(width: width, height: height)
            options.apply(to: model)
            guard let renderer = Renderer(model: model) else { throw OffscreenRender.Failure.noMetal }
            options.apply(to: renderer)
            let r = try verify(renderer: renderer, width: width, height: height)
            print(r.report.summary)
            print(String(format: "GPU %.2f ms (%d chunks), CPU %.0f ms", r.gpuTime * 1000, r.chunks, r.cpuTime * 1000))
            if let path = options["heatmap"] {
                try OffscreenRender.writePNG(bgra: r.heatmap, width: width, height: height, to: path)
            }
            if let path = options["out"] {
                try OffscreenRender.writePNG(bgra: r.image, width: width, height: height, to: path)
            }
            if let path = options["dump"] { try r.gpu.write(to: path) }
            return 0
        } catch {
            fail("\(error)")
            return 1
        }
    }

    /// `--history zoom`: a scripted session through the interactive scheduler (headless), like
    /// holding Space: starts `exp(speed × seconds)` zoomed out from the view and zooms in
    /// around `--pointer x,y` (points from the centre, y up) at `--speed` e-folds/s with
    /// auto iterations, at `--ppp` view points per pixel (0.5: Retina). After the key is
    /// released the final refined frame is diffed against the CPU oracle and a fresh headless
    /// GPU render of the same view; `--max-mismatch pct` (default 0.5) fails the command if
    /// either class mismatch is larger (stale, unfinished or misclassified pixels).
    static func history(options: CommandOptions) throws -> Int32 {
        let (width, height) = try options.size(default: 1280, 820)
        let ppp = options["ppp"].flatMap(Double.init) ?? 0.5
        let speed = options["speed"].flatMap(Double.init) ?? 1.6
        let seconds = options["seconds"].flatMap(Double.init) ?? 2
        let limit = options["max-mismatch"].flatMap(Double.init) ?? 0.5
        var pointer = (0.0, 0.0)
        if let p = options["pointer"] {
            let v = p.split(separator: ",").compactMap { Double($0) }
            guard v.count == 2 else { throw CommandError("bad --pointer \(p)") }
            pointer = (v[0], v[1])
        }
        let model = FractalModel()
        // --scale is per pixel as for --verify; the model's viewport counts view points.
        var end = try options.viewport(width: width, height: height)
        end.scale /= ppp
        options.apply(to: model)
        guard let renderer = Renderer(model: model) else { throw OffscreenRender.Failure.noMetal }
        options.apply(to: renderer)
        var start = end
        start.zoom(by: exp(-speed * seconds), aroundOffset: pointer.0, pointer.1)
        model.viewport = start
        // Settle on the start view first (the session begins from a refined frame).
        _ = renderer.simulate(width: width, height: height, pointsPerPixel: ppp, inputFrames: 0) { _, _ in }
        let frames = Int(seconds * 120)
        guard let sim = renderer.simulate(width: width, height: height, pointsPerPixel: ppp, inputFrames: frames, input: { _, dt in
            model.viewport.zoom(by: exp(speed * dt), aroundOffset: pointer.0, pointer.1)
        }) else { throw CommandError("simulated session did not finish") }
        let v = sim.viewport
        print(String(format: "history zoom: %d frames, final view %.17g, %.17g texel %.4g (zoom %.3g), maxIter %d, ref %d pts, "
                     + "level %dx%d%@, final frame %.0f ms after input",
                     sim.frames, v.centerRe, v.centerIm, sim.texelScale, v.zoom, sim.maxIter, sim.referenceLength,
                     sim.levelWidth, sim.levelHeight, sim.preview ? " (PREVIEW)" : "", sim.finalAfter * 1000))
        let field = IterationField(width: sim.levelWidth, height: sim.levelHeight, texels: sim.iterations)
        let cpu = CPUReference.render(centerRe: v.centerRe, centerIm: v.centerIm, scale: sim.texelScale,
                                      width: sim.levelWidth, height: sim.levelHeight, maxIter: sim.maxIter)
        let (vsCPU, heatmap) = Verifier.compare(gpu: field, cpu: cpu)
        print("vs CPU oracle:  " + vsCPU.summary)
        // Fresh headless render of the same texel grid, settings and iteration limit.
        let fresh = FractalModel(viewport: Viewport(centerRe: v.centerRe, centerIm: v.centerIm, scale: sim.texelScale))
        options.apply(to: fresh)
        fresh.autoIterations = false
        fresh.manualIterations = sim.maxIter
        fresh.maxIterationsCap = max(fresh.maxIterationsCap, sim.maxIter)
        guard let freshRenderer = Renderer(model: fresh) else { throw OffscreenRender.Failure.noMetal }
        let g = try OffscreenRender.render(renderer: freshRenderer, width: sim.levelWidth, height: sim.levelHeight,
                                           warmup: 0, readback: true)
        let freshField = IterationField(width: sim.levelWidth, height: sim.levelHeight, texels: g.iterations ?? [])
        let (vsFresh, freshHeatmap) = Verifier.compare(gpu: field, cpu: freshField)
        print("vs fresh GPU:   " + vsFresh.summary)
        if let path = options["heatmap"] {
            try OffscreenRender.writePNG(bgra: heatmap, width: sim.levelWidth, height: sim.levelHeight, to: path)
            try OffscreenRender.writePNG(bgra: freshHeatmap, width: sim.levelWidth, height: sim.levelHeight,
                                         to: path.replacingOccurrences(of: ".png", with: "-fresh.png"))
        }
        if let path = options["out"] { try OffscreenRender.writePNG(bgra: sim.pixels, width: width, height: height, to: path) }
        let worst = max(vsCPU.mismatchPct, vsFresh.mismatchPct)
        if sim.preview || worst > limit {
            fail(String(format: "history zoom FAILED: class mismatch %.3f%% > %.3f%%%@", worst, limit,
                        sim.preview ? " (final frame is a preview)" : ""))
            return 1
        }
        print(String(format: "history zoom OK: class mismatch %.3f%% <= %.3f%%", worst, limit))
        return 0
    }

    struct Result {
        let report: VerifyReport
        let heatmap: [UInt8]
        let image: [UInt8]
        let gpu: IterationField
        let gpuTime: Double
        let cpuTime: Double
        let chunks: Int
    }

    /// GPU render with readback, CPU render of the same grid, diff.
    static func verify(renderer: Renderer, width: Int, height: Int) throws -> Result {
        let model = renderer.model
        let g = try OffscreenRender.render(renderer: renderer, width: width, height: height, warmup: 0, readback: true)
        guard let texels = g.iterations else { throw OffscreenRender.Failure.renderFailed }
        let gpu = IterationField(width: width, height: height, texels: texels)
        let v = model.viewport
        let t0 = Date()
        let cpu = CPUReference.render(centerRe: v.centerRe, centerIm: v.centerIm, scale: v.scale,
                                      width: width, height: height, maxIter: model.maxIterations)
        let cpuTime = Date().timeIntervalSince(t0)
        let (report, heatmap) = Verifier.compare(gpu: gpu, cpu: cpu)
        return Result(report: report, heatmap: heatmap, image: g.pixels, gpu: gpu, gpuTime: g.iterateGPUTime,
                      cpuTime: cpuTime, chunks: g.chunks)
    }
}
