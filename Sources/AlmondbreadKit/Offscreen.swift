import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Headless rendering: the same two GPU passes as the interactive view, into a PNG.
/// Used by `AlmondbreadMac --render` to check output and time the kernels without a window.
public enum OffscreenRender {
    public struct Output {
        public let png: Data
        public let iterateGPUTime: Double
        public let colorizeGPUTime: Double
        public let referenceTime: Double
        public let referenceLength: Int
        public let maxIterations: Int
    }

    public enum Failure: Error, CustomStringConvertible {
        case noMetal, renderFailed, encodeFailed
        public var description: String {
            switch self {
            case .noMetal: "Metal is not available"
            case .renderFailed: "GPU render failed"
            case .encodeFailed: "PNG encoding failed"
            }
        }
    }

    /// Renders `model` at `width` x `height` pixels, one view point per pixel.
    /// `warmup` renders discarded first, so timings exclude pipeline and allocation warm-up.
    @MainActor
    public static func renderPNG(model: FractalModel, width: Int, height: Int, warmup: Int = 1) throws -> Output {
        guard let renderer = Renderer(model: model) else { throw Failure.noMetal }
        let r = try render(renderer: renderer, width: width, height: height, warmup: warmup)
        guard let png = encodePNG(bgra: r.pixels, width: width, height: height) else { throw Failure.encodeFailed }
        return Output(png: png, iterateGPUTime: r.iterateGPUTime, colorizeGPUTime: r.colorizeGPUTime,
                      referenceTime: renderer.model.lastReferenceTime, referenceLength: renderer.model.lastReferenceLength,
                      maxIterations: renderer.model.maxIterations)
    }

    /// Renders with an existing renderer (benchmarks reuse one, so the Metal library is compiled
    /// once). `readback` also returns the raw iteration texels for the verifier.
    @MainActor
    public static func render(renderer: Renderer, width: Int, height: Int, warmup: Int = 1,
                              readback: Bool = false) throws -> Renderer.OffscreenResult {
        for _ in 0..<warmup { _ = renderer.renderOffscreen(width: width, height: height) }
        guard let r = renderer.renderOffscreen(width: width, height: height, readback: readback) else {
            throw Failure.renderFailed
        }
        return r
    }

    /// Writes BGRA8 pixels as a PNG file.
    public static func writePNG(bgra: [UInt8], width: Int, height: Int, to path: String) throws {
        guard let png = encodePNG(bgra: bgra, width: width, height: height) else { throw Failure.encodeFailed }
        try png.write(to: URL(fileURLWithPath: path))
    }

    /// Encodes sRGB BGRA8 pixels (the drawable format) as PNG.
    static func encodePNG(bgra: [UInt8], width: Int, height: Int) -> Data? {
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let provider = CGDataProvider(data: Data(bgra) as CFData),
              let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                                  bytesPerRow: width * 4, space: space,
                                  bitmapInfo: CGBitmapInfo(rawValue: CGBitmapInfo.byteOrder32Little.rawValue
                                                           | CGImageAlphaInfo.noneSkipFirst.rawValue),
                                  provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        else { return nil }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return out as Data
    }
}
