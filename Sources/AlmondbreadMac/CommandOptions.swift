import AlmondbreadKit
import Foundation

/// `--key value` options shared by the headless commands (`--render`, `--verify`, `--bench`).
struct CommandOptions {
    private(set) var values: [String: String] = [:]

    /// Flags that take no value.
    static let flags: Set<String> = ["verify", "bench"]

    init(arguments: [String]) throws {
        var i = 0
        while i < arguments.count {
            let a = arguments[i]
            guard a.hasPrefix("--") else { throw CommandError("unexpected argument \(a)") }
            let key = String(a.dropFirst(2))
            if CommandOptions.flags.contains(key) {
                values[key] = "1"
                i += 1
            } else if i + 1 < arguments.count {
                values[key] = arguments[i + 1]
                i += 2
            } else {
                throw CommandError("missing value for \(a)")
            }
        }
    }

    subscript(_ key: String) -> String? { values[key] }

    func size(default w: Int, _ h: Int, key: String = "size") throws -> (Int, Int) {
        guard let size = values[key] else { return (w, h) }
        return try CommandOptions.parseSize(size)
    }

    static func parseSize(_ size: String) throws -> (Int, Int) {
        let parts = size.split(separator: "x").compactMap { Int($0) }
        guard parts.count == 2, parts[0] > 0, parts[1] > 0 else { throw CommandError("bad size \(size)") }
        return (parts[0], parts[1])
    }

    /// Viewport from `--center re,im` and `--scale s` (complex units per pixel); without a
    /// scale, the home view's 3.0 x 2.5 complex units are fitted into the image.
    /// `--chunk-ms`: GPU time per iterate command buffer the scheduler aims for.
    @MainActor
    func apply(to renderer: Renderer) {
        if let v = values["chunk-ms"].flatMap(Double.init) { renderer.chunkTarget = v / 1000 }
    }

    func viewport(width: Int, height: Int) throws -> Viewport {
        var viewport = Viewport.home
        if let center = values["center"] {
            let parts = center.split(separator: ",").compactMap { Double($0) }
            guard parts.count == 2 else { throw CommandError("bad --center \(center)") }
            (viewport.centerRe, viewport.centerIm) = (parts[0], parts[1])
        }
        if let s = values["scale"].flatMap(Double.init) {
            viewport.scale = s
        } else {
            viewport.scale = max(3.0 / Double(width), 2.5 / Double(height))
            if values["center"] == nil { viewport.centerRe = -0.75 }
        }
        return viewport
    }

    /// Applies the rendering settings shared by all commands to `model`.
    @MainActor
    func apply(to model: FractalModel) {
        if let n = values["iterations"].flatMap(Int.init) {
            model.autoIterations = false
            model.manualIterations = n
            model.maxIterationsCap = max(model.maxIterationsCap, n)
        }
        if let v = values["interior"] { model.interiorDetection = v != "0" }
        if let v = values["shading"].flatMap(Double.init) { model.shading = v }
        if let v = values["outline"].flatMap(Double.init) { model.outline = v }
        if let v = values["density"].flatMap(Double.init) { model.colorDensity = v }
        if let v = values["offset"].flatMap(Double.init) { model.colorOffset = v }
        if let v = values["palette"] { model.paletteID = v }
        if values["log"] == "1" { model.logColoring = true }
        if let v = values["ss"].flatMap(Int.init), let ss = Supersampling(rawValue: v) { model.supersampling = ss }
    }
}

struct CommandError: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}

func fail(_ message: String) {
    FileHandle.standardError.write(Data("almondbread: \(message)\n".utf8))
}
