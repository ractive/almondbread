import AlmondbreadCore
import Foundation
import Observation
import simd

/// Anti-aliasing choices of the inspector (samples per axis; the Rust settings carry the
/// factor). The view accumulates factor² jittered samples progressively while idle.
public enum Supersampling: Int, CaseIterable, Identifiable, Sendable {
    case off = 1, x4 = 2, x9 = 3, x16 = 4
    public var id: Int { rawValue }
    public var label: String {
        switch self {
        case .off: "Off"
        case .x4: "4 samples"
        case .x9: "9 samples"
        case .x16: "16 samples"
        }
    }
}

/// Something that runs a display link for the explorer (the platform view).
@MainActor
protocol ExplorerHost: AnyObject {
    /// Resume the display link: the explorer needs frames.
    func requestFrames()
    /// The HUD moved or resized (it is an arrow-cursor hole in the canvas).
    func hudFrameChanged()
}

/// Edge panning choices of the inspector (pointer mode; kb: decisions/controls).
public enum EdgePanning: String, CaseIterable, Identifiable, Sendable {
    case off, whileZooming, always
    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .off: "Off"
        case .whileZooming: "While zooming"
        case .always: "Always"
        }
    }
    var ffi: EdgePan {
        switch self {
        case .off: .off
        case .whileZooming: .whileZooming
        case .always: .always
        }
    }
}

/// The UI's view of one window's explorer: a mirror of the Rust settings that SwiftUI binds
/// to (every change is forwarded, an infrequent control-plane call) and a ~10 Hz snapshot
/// of the renderer's stats for the HUD and inspector. Rendering, motion and all policy live
/// in Rust (`Explorer`, kb: platform/render-architecture); nothing here runs per frame.
@MainActor
@Observable
public final class ExplorerModel {
    // Iterations
    public var autoIterations = true { didSet { push() } }
    public var manualIterations = 1000 { didSet { push() } }
    public var maxIterationsCap = 200_000 { didSet { push() } }
    public var interiorDetection = true { didSet { push() } }

    // Colouring
    public var paletteID = "classic" { didSet { push() } }
    public var colorDensity = 0.02 { didSet { push() } }
    public var colorOffset = 0.0 { didSet { push() } }
    public var logColoring = false { didSet { push() } }
    public var smoothColoring = true { didSet { push() } }
    /// Linear RGB (what the kernel blends in).
    public var interiorColor = SIMD3<Float>(0, 0, 0) { didSet { push() } }
    public var shading = 0.0 { didSet { push() } }
    public var lightAngle = 45.0 { didSet { push() } }
    public var outline = 0.0 { didSet { push() } }
    public var outlineWidth = 1.5 { didSet { push() } }
    /// Linear RGB.
    public var edgeColor = SIMD3<Float>(0, 0, 0) { didSet { push() } }
    public var specular = 0.0 { didSet { push() } }
    public var shininess = 24.0 { didSet { push() } }
    public var lightHeight = 1.0 { didSet { push() } }
    public var glow = 0.0 { didSet { push() } }
    /// Linear RGB.
    public var glowColor = SIMD3<Float>(1, 1, 1) { didSet { push() } }
    public var glowWidth = 6.0 { didSet { push() } }
    public var paletteFilter = true { didSet { push() } }
    /// Palette cycles per second while the view rests (0: off).
    public var cycleSpeed = 0.0 { didSet { push() } }
    public var depthTint = 0.0 { didSet { push() } }
    /// The look last applied (the inspector's picker); edits keep it selected.
    public var lookID = "aurora"

    // Quality
    public var supersampling = Supersampling.off { didSet { push() } }
    public var interactivePreview = true { didSet { push() } }

    // Navigation (persisted; the navigator in Rust applies it)
    public var edgePanning: EdgePanning = EdgePanning(rawValue: UserDefaults.standard.string(forKey: ExplorerModel.edgePanKey) ?? "")
        ?? .whileZooming {
        didSet {
            UserDefaults.standard.set(edgePanning.rawValue, forKey: Self.edgePanKey)
            explorer?.setEdgePan(mode: edgePanning.ffi)
        }
    }
    static let edgePanKey = "edgePanning"

    /// Autopilot assist: the heading leans towards nearby boundary detail while the pointer
    /// rests (persisted; off by default; kb: decisions/controls, autopilot).
    public var autopilotAssist: Bool = UserDefaults.standard.bool(forKey: ExplorerModel.assistKey) {
        didSet {
            UserDefaults.standard.set(autopilotAssist, forKey: Self.assistKey)
            explorer?.setAutopilotAssist(on: autopilotAssist)
        }
    }
    static let assistKey = "autopilotAssist"

    /// The HUD's frame in SwiftUI's global space (the canvas shows the arrow cursor there).
    @ObservationIgnored public var hudFrame: CGRect? {
        didSet { if hudFrame != oldValue { host?.hudFrameChanged() } }
    }

    /// Renderer stats, refreshed at ~10 Hz while the view animates (views reading these
    /// re-render at that rate, never per input event: kb synthesis/benchmarks, M2 findings).
    public private(set) var stats: ExplorerStats?
    /// When the zoom speed (Q / E, handled in Rust) last changed, for the HUD highlight.
    public private(set) var zoomSpeedChanged = Date.distantPast
    /// Why the renderer could not be created, if it could not.
    public internal(set) var failure: String?

    public static let palettes: [PaletteInfo] = AlmondbreadCore.palettes()
    public static let looks: [LookInfo] = AlmondbreadCore.looks()

    @ObservationIgnored private(set) var explorer: Explorer?
    @ObservationIgnored private weak var host: ExplorerHost?
    @ObservationIgnored private var pulling = false

    public init() {}

    /// Connects the view's explorer; the model takes over Rust's settings.
    func attach(_ explorer: Explorer, host: ExplorerHost) {
        self.explorer = explorer
        self.host = host
        explorer.setEdgePan(mode: edgePanning.ffi)
        explorer.setAutopilotAssist(on: autopilotAssist)
        pull(explorer.settings())
        refreshStats()
    }

    // MARK: Settings

    private func pull(_ s: ExplorerSettings) {
        pulling = true
        defer { pulling = false }
        autoIterations = s.autoIterations
        manualIterations = Int(s.manualIterations)
        maxIterationsCap = Int(s.maxIterationsCap)
        interiorDetection = s.interiorDetection
        paletteID = s.paletteId
        colorDensity = s.colorDensity
        colorOffset = s.colorOffset
        logColoring = s.logColoring
        smoothColoring = s.smoothColoring
        interiorColor = SIMD3(s.interiorRed, s.interiorGreen, s.interiorBlue)
        shading = s.shading
        lightAngle = s.lightAngle
        outline = s.outline
        outlineWidth = s.outlineWidth
        edgeColor = SIMD3(s.edgeRed, s.edgeGreen, s.edgeBlue)
        specular = s.specular
        shininess = s.shininess
        lightHeight = s.lightHeight
        glow = s.glow
        glowColor = SIMD3(s.glowRed, s.glowGreen, s.glowBlue)
        glowWidth = s.glowWidth
        paletteFilter = s.paletteFilter
        cycleSpeed = s.cycleSpeed
        depthTint = s.depthTint
        supersampling = Supersampling(rawValue: Int(s.supersampling)) ?? .off
        interactivePreview = s.interactivePreview
    }

    private var settings: ExplorerSettings {
        ExplorerSettings(
            autoIterations: autoIterations, manualIterations: UInt64(max(1, manualIterations)),
            maxIterationsCap: UInt64(max(1, maxIterationsCap)), interiorDetection: interiorDetection,
            paletteId: paletteID, colorDensity: colorDensity, colorOffset: colorOffset,
            logColoring: logColoring, smoothColoring: smoothColoring,
            interiorRed: interiorColor.x, interiorGreen: interiorColor.y, interiorBlue: interiorColor.z,
            shading: shading, lightAngle: lightAngle, outline: outline, outlineWidth: outlineWidth,
            edgeRed: edgeColor.x, edgeGreen: edgeColor.y, edgeBlue: edgeColor.z,
            specular: specular, shininess: shininess, lightHeight: lightHeight,
            glow: glow, glowRed: glowColor.x, glowGreen: glowColor.y, glowBlue: glowColor.z, glowWidth: glowWidth,
            paletteFilter: paletteFilter, cycleSpeed: cycleSpeed, depthTint: depthTint,
            supersampling: UInt8(supersampling.rawValue), interactivePreview: interactivePreview)
    }

    /// Applies a look (palette and colouring) in Rust and takes over the result.
    public func applyLook(_ id: String) {
        guard let explorer else { return }
        lookID = id
        pull(explorer.applyLook(id: id))
        host?.requestFrames()
    }

    private func push() {
        guard !pulling, let explorer else { return }
        do {
            try explorer.setSettings(settings: settings)
        } catch {
            NSLog("Almondbread: settings rejected: \(error)")
            pull(explorer.settings())
        }
        host?.requestFrames()
    }

    // MARK: Actions (menu commands, inspector buttons)

    /// Sends an event as if it came from the view (menu zoom commands).
    @discardableResult
    func send(_ event: InputEvent) -> Bool {
        guard let explorer else { return false }
        let r = explorer.input(event: event)
        if r.redraw { host?.requestFrames() }
        return r.handled
    }

    /// Menu zoom: x2 at the zoom target (the centre unless the keyboard crosshair is still
    /// gliding; the pointer is on the menu bar).
    public func zoomIn() { send(.zoomBy(factor: 2)) }
    public func zoomOut() { send(.zoomBy(factor: 0.5)) }

    /// Autopilot on / off (the Z key's action; menu and toolbar).
    public func toggleAutopilot() {
        send(.keyDown(key: .autopilot))
        send(.keyUp(key: .autopilot))
    }

    /// Stops the autopilot or a flight (eased), like Esc.
    public func stopMotion() {
        send(.keyDown(key: .stop))
        send(.keyUp(key: .stop))
    }

    /// Flies to a view (any depth); throws if the text is not a view.
    public func flyTo(re: String, im: String, scale: String) throws {
        guard let explorer else { return }
        try explorer.flyTo(re: re, im: im, scale: scale)
        host?.requestFrames()
    }

    /// Flies to coordinates on the clipboard (the text Copy Coordinates writes). Returns false
    /// if the clipboard holds no view.
    @discardableResult
    public func flyToCopiedCoordinates() -> Bool {
        guard let text = Clipboard.text(), let explorer, explorer.flyToCoordinates(text: text) else { return false }
        host?.requestFrames()
        return true
    }

    public func reset() {
        explorer?.resetViewport()
        host?.requestFrames()
    }

    /// Centre and scale for the clipboard, full precision (as many digits as the depth needs).
    public var coordinatesText: String { explorer?.coordinatesText() ?? "" }

    /// Jumps to coordinates on the clipboard (the "re / im / scale" text Copy writes).
    /// Returns false if the clipboard holds no view.
    @discardableResult
    public func pasteCoordinates() -> Bool {
        guard let text = Clipboard.text(), let explorer, explorer.pasteCoordinates(text: text) else { return false }
        host?.requestFrames()
        return true
    }

    // MARK: Stats

    /// Reads the renderer's stats (called by the view at ~10 Hz while animating, and once
    /// when it stops). Assigns only on change, so idle views do not re-render.
    func refreshStats() {
        guard let new = explorer?.stats(), new != stats else { return }
        if let old = stats, old.zoomSpeed != new.zoomSpeed || old.autopilot.speed != new.autopilot.speed
            || old.flight.speed != new.flight.speed {
            zoomSpeedChanged = Date()
        }
        stats = new
    }

    /// Zoom speed as a magnification factor per second, for display.
    public var zoomFactorPerSecond: Double { exp(stats?.zoomSpeed ?? 1.6) }

    public var palette: PaletteInfo {
        Self.palettes.first { $0.id == paletteID } ?? Self.palettes[0]
    }
}
