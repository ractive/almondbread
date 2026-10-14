import AlmondbreadKit
import Foundation

/// `--render`: headless PNG output through the same GPU passes as the app.
@MainActor
enum RenderCommand {
    static func run(arguments: [String]) -> Int32 {
        do {
            let options = try CommandOptions(arguments: arguments)
            guard let path = options["render"] else { throw CommandError("missing output path") }
            let (width, height) = try options.size(default: 1600, 1000)
            let model = FractalModel()
            let viewport = try options.viewport(width: width, height: height)
            model.viewport = viewport
            options.apply(to: model)
            let repeats = options["repeat"].flatMap(Int.init) ?? 1

            guard let renderer = Renderer(model: model) else { throw OffscreenRender.Failure.noMetal }
            options.apply(to: renderer)
            var times: [Double] = []
            var walls: [Double] = []
            var out: Renderer.OffscreenResult?
            for i in 0..<max(1, repeats) {
                let o = try OffscreenRender.render(renderer: renderer, width: width, height: height, warmup: i == 0 ? 1 : 0)
                times.append(o.iterateGPUTime)
                walls.append(o.wallTime)
                out = o
            }
            guard let out else { return 1 }
            try OffscreenRender.writePNG(bgra: out.pixels, width: width, height: height, to: path)
            let ms = times.map { String(format: "%.2f", $0 * 1000) }.joined(separator: ", ")
            let wall = walls.map { String(format: "%.1f", $0 * 1000) }.joined(separator: ", ")
            print(String(format: "%@: %dx%d  zoom %.3g  maxIter %d  ref %d pts %.2f ms  iterate GPU [%@] ms  wall [%@] ms  chunks %d  iters %.3g  colorize %.2f ms",
                         path, width, height, viewport.zoom, model.maxIterations, model.lastReferenceLength,
                         model.lastReferenceTime * 1000, ms, wall, out.chunks, Double(out.iterationsDone),
                         out.colorizeGPUTime * 1000))
            return 0
        } catch {
            fail("\(error)")
            return 1
        }
    }
}
