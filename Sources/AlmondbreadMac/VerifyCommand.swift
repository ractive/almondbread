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
            if let o = options["ref-offset"] {
                // Reference reuse: the orbit of a point `dx,dy` pixels from the centre, made at
                // `--ref-scale` times the scale (default 1).
                let v = o.split(separator: ",").compactMap { Double($0) }
                guard v.count == 2 else { throw CommandError("bad --ref-offset \(o)") }
                var rv = model.viewport
                rv.centerRe += v[0] * rv.scale
                rv.centerIm += v[1] * rv.scale
                rv.scale *= options["ref-scale"].flatMap(Double.init) ?? 1
                renderer.primeReference(viewport: rv, maxIter: model.maxIterations)
            }
            let r = try verify(renderer: renderer, width: width, height: height,
                               cpuIterations: options["cpu-iterations"].flatMap(Int.init))
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

    /// `--history zoom|out|pan`: a scripted session through the interactive scheduler
    /// (headless, the code the view runs per refresh), e.g. holding Space: starts
    /// `exp(speed × seconds)` zoomed out from the view and zooms in around `--pointer x,y`
    /// (points from the centre, y up) at `--speed` e-folds/s with auto iterations, at `--ppp`
    /// view points per pixel (0.5: Retina). After the key is released and the view refined
    /// (unresolved pixels continued up to the extension limit), the final frame is diffed
    /// against the CPU oracle at that limit (`--cpu-iterations` overrides) and against a fresh
    /// headless GPU render of the same view. Fails (exit 1) when the final frame is a preview,
    /// paints more than `--max-false-interior` % (default 0.05) of CPU-escaping pixels as
    /// interior, or differs from the fresh render in more than `--max-mismatch` % (default
    /// 0.05) of the pixels (stale or unfinished pixels). `--then f` adds a second session
    /// zooming one step of factor f (one mouse-wheel notch is 1.15–1.32) after the first,
    /// as in the neck regression where one step used to change the picture.
    static func history(options: CommandOptions) throws -> Int32 {
        let (width, height) = try options.size(default: 1280, 820)
        let ppp = options["ppp"].flatMap(Double.init) ?? 0.5
        let speed = options["speed"].flatMap(Double.init) ?? 1.6
        let seconds = options["seconds"].flatMap(Double.init) ?? 2
        let maxMismatch = options["max-mismatch"].flatMap(Double.init) ?? 0.05
        let maxFalseInterior = options["max-false-interior"].flatMap(Double.init) ?? 0.05
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
        // Deterministic for the diff: let the extension run to its limit (the app stops after
        // `extensionBudget` GPU seconds).
        renderer.extensionBudget = options["extension-budget"].flatMap(Double.init) ?? 120
        // zoom: Space toward the pointer; out: Shift+Space; pan: WASD at the keys' speed in
        // the direction of --pointer (both end at the given view).
        let mode = options["history"] ?? "zoom"
        let panLength = (pointer.0 * pointer.0 + pointer.1 * pointer.1).squareRoot()
        let pan = panLength > 0 ? (pointer.0 / panLength, pointer.1 / panLength) : (1.0, 0.0)
        let panSpeed = 0.4 * Double(width) * ppp   // view points per second (KeyControls.panSpeed)
        var start = end
        switch mode {
        case "zoom": start.zoom(by: exp(-speed * seconds), aroundOffset: pointer.0, pointer.1)
        case "out": start.zoom(by: exp(speed * seconds), aroundOffset: pointer.0, pointer.1)
        case "pan": start.pan(byPoints: pan.0 * panSpeed * seconds, pan.1 * panSpeed * seconds)
        default: throw CommandError("unknown --history \(mode) (zoom, out, pan)")
        }
        model.viewport = start
        // Settle on the start view first (the session begins from a refined frame).
        _ = renderer.simulate(width: width, height: height, pointsPerPixel: ppp, inputFrames: 0) { _, _ in }
        let frames = Int(seconds * 120)
        var sessions: [Renderer.SimulationResult] = []
        guard let first = renderer.simulate(width: width, height: height, pointsPerPixel: ppp, inputFrames: frames, input: { _, dt in
            switch mode {
            case "zoom": model.viewport.zoom(by: exp(speed * dt), aroundOffset: pointer.0, pointer.1)
            case "out": model.viewport.zoom(by: exp(-speed * dt), aroundOffset: pointer.0, pointer.1)
            default: model.viewport.pan(byPoints: -pan.0 * panSpeed * dt, -pan.1 * panSpeed * dt)
            }
        }) else { throw CommandError("simulated session did not finish") }
        sessions.append(first)
        if let f = options["then"].flatMap(Double.init) {
            guard let second = renderer.simulate(width: width, height: height, pointsPerPixel: ppp, inputFrames: 1, input: { _, _ in
                model.viewport.zoom(by: f, aroundOffset: pointer.0, pointer.1)
            }) else { throw CommandError("second session did not finish") }
            sessions.append(second)
        }
        var failed: [String] = []
        for (i, sim) in sessions.enumerated() {
            let v = sim.viewport
            let tag = sessions.count > 1 ? "[\(i + 1)] " : ""
            print(String(format: "%@history %@: %d frames, final view %.17g, %.17g texel %.4g (zoom %.3g), maxIter %d (hard %d), "
                         + "ref %d pts, level %dx%d%@, complete %.0f ms / resolved %.0f ms after input, unresolved %d, "
                         + "learned limit %d",
                         tag, mode, sim.frames, v.centerRe, v.centerIm, sim.texelScale, v.zoom, sim.maxIter, sim.hardIter,
                         sim.referenceLength, sim.levelWidth, sim.levelHeight, sim.preview ? " (PREVIEW)" : "",
                         sim.finalAfter * 1000, sim.doneAfter * 1000, sim.unresolved, model.learnedIterations))
            let field = IterationField(width: sim.levelWidth, height: sim.levelHeight, texels: sim.iterations)
            let cpuIter = options["cpu-iterations"].flatMap(Int.init) ?? sim.hardIter
            let cpu = CPUReference.render(centerRe: v.centerRe, centerIm: v.centerIm, scale: sim.texelScale,
                                          width: sim.levelWidth, height: sim.levelHeight, maxIter: cpuIter)
            let (vsCPU, heatmap) = Verifier.compare(gpu: field, cpu: cpu)
            print("\(tag)vs CPU oracle at \(cpuIter):  " + vsCPU.summary)
            // Fresh headless render of the same texel grid, settings and limits.
            let fresh = FractalModel(viewport: Viewport(centerRe: v.centerRe, centerIm: v.centerIm, scale: sim.texelScale))
            options.apply(to: fresh)
            fresh.supersampling = .off
            fresh.autoIterations = false
            fresh.manualIterations = sim.maxIter
            fresh.maxIterationsCap = max(model.maxIterationsCap, sim.maxIter)
            guard let freshRenderer = Renderer(model: fresh) else { throw OffscreenRender.Failure.noMetal }
            freshRenderer.offscreenExtension = max(1, sim.hardIter / max(1, sim.maxIter))
            freshRenderer.extensionBudget = renderer.extensionBudget
            let g = try OffscreenRender.render(renderer: freshRenderer, width: sim.levelWidth, height: sim.levelHeight,
                                               warmup: 0, readback: true)
            let freshField = IterationField(width: sim.levelWidth, height: sim.levelHeight, texels: g.iterations ?? [])
            let (vsFresh, freshHeatmap) = Verifier.compare(gpu: field, cpu: freshField)
            print("\(tag)vs fresh GPU:   " + vsFresh.summary)
            if let path = options["heatmap"] {
                let base = sessions.count > 1 ? path.replacingOccurrences(of: ".png", with: "-\(i + 1).png") : path
                try OffscreenRender.writePNG(bgra: heatmap, width: sim.levelWidth, height: sim.levelHeight, to: base)
                try OffscreenRender.writePNG(bgra: freshHeatmap, width: sim.levelWidth, height: sim.levelHeight,
                                             to: base.replacingOccurrences(of: ".png", with: "-fresh.png"))
            }
            if let path = options["out"] {
                let base = sessions.count > 1 ? path.replacingOccurrences(of: ".png", with: "-\(i + 1).png") : path
                try OffscreenRender.writePNG(bgra: sim.pixels, width: width, height: height, to: base)
            }
            if sim.preview { failed.append("\(tag)final frame is a preview") }
            if vsCPU.falseInteriorPct > maxFalseInterior {
                failed.append(String(format: "%@%.3f%% exterior pixels painted interior (> %.3f%%)", tag, vsCPU.falseInteriorPct, maxFalseInterior))
            }
            if vsFresh.mismatchPct > maxMismatch {
                failed.append(String(format: "%@%.3f%% differ from a fresh render (> %.3f%%)", tag, vsFresh.mismatchPct, maxMismatch))
            }
        }
        if !failed.isEmpty {
            fail("history \(mode) FAILED: " + failed.joined(separator: "; "))
            return 1
        }
        print("history \(mode) OK")
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
    static func verify(renderer: Renderer, width: Int, height: Int, cpuIterations: Int? = nil) throws -> Result {
        let model = renderer.model
        let g = try OffscreenRender.render(renderer: renderer, width: width, height: height, warmup: 0, readback: true)
        guard let texels = g.iterations else { throw OffscreenRender.Failure.renderFailed }
        let gpu = IterationField(width: width, height: height, texels: texels)
        let v = model.viewport
        let t0 = Date()
        // `--cpu-iterations n`: oracle limit (default the GPU's), e.g. far above it to tell
        // unresolved exterior from interior.
        let cpuIter = cpuIterations ?? model.maxIterations
        let cpu = CPUReference.render(centerRe: v.centerRe, centerIm: v.centerIm, scale: v.scale,
                                      width: width, height: height, maxIter: cpuIter)
        let cpuTime = Date().timeIntervalSince(t0)
        let (report, heatmap) = Verifier.compare(gpu: gpu, cpu: cpu)
        return Result(report: report, heatmap: heatmap, image: g.pixels, gpu: gpu, gpuTime: g.iterateGPUTime,
                      cpuTime: cpuTime, chunks: g.chunks)
    }
}
