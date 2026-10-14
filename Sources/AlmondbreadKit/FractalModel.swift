import Foundation
import Observation
import simd

/// Where we look: centre of the view in the complex plane and the size of one view point.
public struct Viewport: Equatable, Sendable {
    public var centerRe: Double
    public var centerIm: Double
    /// Complex units per view point (not per pixel: independent of the display scale).
    public var scale: Double

    public init(centerRe: Double, centerIm: Double, scale: Double) {
        self.centerRe = centerRe
        self.centerIm = centerIm
        self.scale = scale
    }

    public static let home = Viewport(centerRe: -0.75, centerIm: 0, scale: 3.0 / 800)

    /// Magnification relative to the home view.
    public var zoom: Double { Viewport.home.scale / scale }

    /// Double-precision limit: below this, neighbouring pixels collapse onto the same
    /// reference coordinate. Arbitrary-precision reference orbits will lift it.
    public var minScale: Double { 4e-16 * max(1, abs(centerRe), abs(centerIm)) }

    /// True when zooming in further is blocked by `minScale`.
    public var atPrecisionLimit: Bool { scale <= minScale * 1.001 }

    /// The whole set (re -2.25...0.75 plus margin) fitted into a view of `width` x `height` points.
    public static func fitting(width: Double, height: Double) -> Viewport {
        guard width > 0, height > 0 else { return home }
        return Viewport(centerRe: -0.75, centerIm: 0, scale: max(3.2 / width, 2.6 / height))
    }

    /// Complex coordinate of a point given in view points from the view centre (y up).
    public func complex(atOffset dx: Double, _ dy: Double) -> (Double, Double) {
        (centerRe + dx * scale, centerIm + dy * scale)
    }

    /// Zooms by `factor` (>1 zooms in) keeping the point at offset (dx, dy) fixed.
    public mutating func zoom(by factor: Double, aroundOffset dx: Double, _ dy: Double) {
        let (re, im) = complex(atOffset: dx, dy)
        scale = min(max(scale / factor, minScale), 0.05)
        centerRe = re - dx * scale
        centerIm = im - dy * scale
    }

    public mutating func pan(byPoints dx: Double, _ dy: Double) {
        centerRe -= dx * scale
        centerIm -= dy * scale
    }
}

public enum Supersampling: Int, CaseIterable, Identifiable, Sendable {
    case off = 1, x4 = 2, x9 = 3
    public var id: Int { rawValue }
    public var label: String {
        switch self {
        case .off: "Off"
        case .x4: "4× (2×2)"
        case .x9: "9× (3×3)"
        }
    }
}

/// All user-facing state. The renderer reads it every frame; SwiftUI edits it.
@MainActor
@Observable
public final class FractalModel {
    public var viewport = Viewport.home

    // Iterations
    public var autoIterations = true
    public var manualIterations = 1000
    public var maxIterationsCap = 200_000
    /// Floor the renderer learned from escapes past the nominal limit (statistics-driven auto
    /// iterations, kb: techniques/auto-iterations); applies from the next view on.
    @ObservationIgnored public var learnedIterations = 0
    /// Stop pixels whose orbit derivative shows an attracting cycle (kb: techniques/interior-detection).
    public var interiorDetection = true

    // Colouring
    public var paletteID = Palette.default.id
    public var colorDensity = 0.02        // palette cycles per iteration
    public var colorOffset = 0.0
    public var logColoring = false
    public var smoothColoring = true
    public var interiorColor = SIMD3<Float>(0, 0, 0)
    public var shading = 0.0
    public var lightAngle = 45.0           // degrees
    public var outline = 0.0
    public var outlineWidth = 1.5

    // Navigation
    /// Continuous zoom speed (Space), e-folds of magnification per second; Q / E change it.
    public var zoomSpeed = 1.6
    public static let zoomSpeedRange = 0.1...8.0
    /// When zoomSpeed last changed, for the HUD highlight.
    public var zoomSpeedChanged = Date.distantPast

    public func setZoomSpeed(_ v: Double) {
        zoomSpeed = min(max(v, Self.zoomSpeedRange.lowerBound), Self.zoomSpeedRange.upperBound)
        zoomSpeedChanged = Date()
    }

    /// Zoom speed as a magnification factor per second, for display.
    public var zoomFactorPerSecond: Double { exp(zoomSpeed) }

    // Quality
    public var supersampling = Supersampling.off
    public var interactivePreview = true

    // Stats (written by the renderer)
    // Throttled snapshots for the HUD and inspector (~10 Hz): views that observed `viewport`
    // directly re-rendered on every input event, and that SwiftUI work throttled the Metal
    // view to 60 Hz during zoom gestures (measured with --interaction-test).
    public var shownViewport = Viewport.home
    public var shownMaxIterations = 0
    public var lastGPUTime: Double = 0      // all iterate chunks of the current view so far
    public var lastChunks = 0
    public var isRendering = false
    public var lastReferenceTime: Double = 0
    public var lastReferenceLength = 0
    public var renderSize = SIMD2<Int>(0, 0)
    public var isPreview = false

    public init() {}

    public init(viewport: Viewport) { self.viewport = viewport }

    public var palette: Palette { Palette.named(paletteID) }

    /// Iteration limit: manual, or automatic: a heuristic that grows with zoom depth, raised to
    /// what the renderer learned from the last views' escape statistics.
    public var maxIterations: Int {
        if !autoIterations { return min(manualIterations, maxIterationsCap) }
        return min(max(depthIterations, learnedIterations), maxIterationsCap)
    }

    /// The depth-only part of the automatic limit.
    public var depthIterations: Int {
        let depth = max(0, log10(viewport.zoom))
        return Int(300 + 120 * pow(depth, 1.6))
    }

    public var deNeeded: Bool { shading > 0 || outline > 0 }

    /// Size of the fractal view in points, set by the UI; the home view is fitted to it.
    /// Not observed by the renderer (it reads the drawable size itself).
    @ObservationIgnored public var viewSize = CGSize.zero {
        didSet {
            // First layout: replace the fixed default with a view that fits the window.
            if !didFitInitialView, viewSize.width > 0, viewSize.height > 0 {
                didFitInitialView = true
                if viewport == .home { reset() }
            }
        }
    }
    @ObservationIgnored private var didFitInitialView = false

    public func reset() {
        viewport = Viewport.fitting(width: viewSize.width, height: viewSize.height)
    }

    /// Text for the clipboard: centre and scale with full Double precision.
    public var coordinatesText: String {
        let v = viewport
        return String(format: "re %.17g\nim %.17g\nscale %.6g (zoom %.6g×)", v.centerRe, v.centerIm, v.scale, v.zoom)
    }
}
