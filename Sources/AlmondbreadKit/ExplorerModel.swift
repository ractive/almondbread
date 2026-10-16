import AlmondbreadCore
import Foundation
import Observation
import simd

/// Anti-aliasing choices of the control center (samples per axis; the Rust settings carry the
/// factor). The view accumulates factor² jittered samples progressively while idle.
public enum Supersampling: Int, CaseIterable, Identifiable, Sendable, Codable {
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

/// Edge panning choices (pointer mode; kb: decisions/controls). An app-wide preference.
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

/// Interior colourings (kb: techniques/coloring-methods, "Interior colouring as built").
public enum InteriorColoring: String, CaseIterable, Identifiable, Sendable, Codable {
    case flat, bubbles, period, angle, distance
    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .flat: "Flat"
        case .bubbles: "Bubbles"
        case .period: "Period"
        case .angle: "Internal angle"
        case .distance: "Interior distance"
        }
    }
    public var help: String {
        switch self {
        case .flat: "One colour."
        case .bubbles: "Rings of the attracting cycle's closest approach to 0, centred on each nucleus."
        case .period: "One colour per period of the attracting cycle, brighter towards each component's centre."
        case .angle: "The multiplier's internal angle: stripes along the internal rays, rings of its radius."
        case .distance: "Interior distance estimate: bright inside, dark towards the boundary."
        }
    }
    var ffi: InteriorMode {
        switch self {
        case .flat: .flat
        case .bubbles: .bubbles
        case .period: .period
        case .angle: .angle
        case .distance: .distance
        }
    }
    init(_ m: InteriorMode) {
        switch m {
        case .flat: self = .flat
        case .bubbles: self = .bubbles
        case .period: self = .period
        case .angle: self = .angle
        case .distance: self = .distance
        }
    }
}

/// Corner of the mini-map.
public enum MapPlacement: String, CaseIterable, Identifiable, Sendable, Codable {
    case bottomLeft, bottomRight, topLeft, topRight
    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .bottomLeft: "Bottom left"
        case .bottomRight: "Bottom right"
        case .topLeft: "Top left"
        case .topRight: "Top right"
        }
    }
    var ffi: MapCorner {
        switch self {
        case .bottomLeft: .bottomLeft
        case .bottomRight: .bottomRight
        case .topLeft: .topLeft
        case .topRight: .topRight
        }
    }
}

/// How the zoom is written in the HUD and the control center (an app-wide preference).
public enum ZoomUnits: String, CaseIterable, Identifiable, Sendable {
    case magnification, powerOfTen
    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .magnification: "Magnification (1.2e9×)"
        case .powerOfTen: "Power of ten (10^9.1)"
        }
    }
}

/// App-wide preferences (the Settings window, ⌘,), in user defaults.
public enum Preferences {
    public static let edgePanKey = "edgePanning"
    public static let gamepadKey = "gamepadEnabled"
    public static let zoomUnitsKey = "zoomUnits"
    public static let restoreKey = "restoreSettings"
    public static let hudKey = "showHUD"
    static let savedSettingsKey = "almondbread.settings.v1"

    static var gamepadEnabled: Bool { UserDefaults.standard.object(forKey: gamepadKey) as? Bool ?? true }
    static var restoreSettings: Bool { UserDefaults.standard.object(forKey: restoreKey) as? Bool ?? true }
    static var zoomUnits: ZoomUnits {
        ZoomUnits(rawValue: UserDefaults.standard.string(forKey: zoomUnitsKey) ?? "") ?? .magnification
    }
}

/// What persists between launches (look, colour, interior, quality and overlays; not the view).
struct SavedSettings: Codable {
    var autoIterations: Bool?
    var manualIterations: Int?
    var maxIterationsCap: Int?
    var interiorDetection: Bool?
    var paletteID: String?
    var colorDensity: Double?
    var colorOffset: Double?
    var logColoring: Bool?
    var smoothColoring: Bool?
    var interiorColor: [Float]?
    var shading: Double?
    var lightAngle: Double?
    var outline: Double?
    var outlineWidth: Double?
    var edgeColor: [Float]?
    var specular: Double?
    var shininess: Double?
    var lightHeight: Double?
    var glow: Double?
    var glowColor: [Float]?
    var glowWidth: Double?
    var paletteFilter: Bool?
    var cycleSpeed: Double?
    var depthTint: Double?
    var cinematic: Bool?
    var cinematicSpin: Double?
    var cinematicHueDrift: Double?
    var cinematicPhaseDrift: Double?
    var cinematicFlyCycle: Double?
    var lookID: String?
    var supersampling: Supersampling?
    var interactivePreview: Bool?
    var interiorColoring: InteriorColoring?
    var interiorDensity: Double?
    var interiorStripes: Double?
    var denseCalm: Bool?
    var orbitEnabled: Bool?
    var orbitLength: Int?
    var mapEnabled: Bool?
    var mapLevels: Int?
    var mapSize: Double?
    var mapPlacement: MapPlacement?
    var piMeterEnabled: Bool?
    var cosmicScale: Bool?
    var cosmicAnchor: String?
    var compassAlways: Bool?
}

/// The UI's view of one window's explorer: a mirror of the Rust settings that SwiftUI binds
/// to (every change is forwarded, an infrequent control-plane call) and a ~10 Hz snapshot
/// of the renderer's stats for the HUD and control center. Rendering, motion and all policy
/// live in Rust (`Explorer`, kb: platform/render-architecture); nothing here runs per frame.
/// Settings persist in user defaults (restored at launch unless the preference is off).
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
    /// Cinematic effects (kb: decisions/controls "Cinematic effects"): off by default; the
    /// view spins with automatic zooms (degrees per e-fold), the palette's hue and phase drift
    /// with depth (per e-fold), colours cycle while flying (cycles per second).
    public var cinematic = false { didSet { push() } }
    public var cinematicSpin = 4.0 { didSet { push() } }
    public var cinematicHueDrift = 1.0 { didSet { push() } }
    public var cinematicPhaseDrift = 0.01 { didSet { push() } }
    public var cinematicFlyCycle = 0.02 { didSet { push() } }
    /// Dense regions: calm dark colour with thin filaments (off: the palette's mean grey).
    public var denseCalm = true { didSet { push() } }
    /// The look last applied (the control center's picker); edits keep it selected.
    public var lookID = "aurora" { didSet { save() } }

    // Interior
    public var interiorColoring = InteriorColoring.flat { didSet { push() } }
    public var interiorDensity = 0.25 { didSet { push() } }
    public var interiorStripes = 8.0 { didSet { push() } }

    // Quality
    public var supersampling = Supersampling.off { didSet { push() } }
    public var interactivePreview = true { didSet { push() } }

    // Overlays (drawn by Rust over the fractal; kb: decisions/controls)
    public var orbitEnabled = false { didSet { pushOverlay() } }
    public var orbitLength = 50 { didSet { pushOverlay() } }
    public var mapEnabled = false { didSet { pushOverlay() } }
    public var mapLevels = 3 { didSet { pushOverlay() } }
    public var mapSize = 180.0 { didSet { pushOverlay() } }
    public var mapPlacement = MapPlacement.bottomLeft { didSet { pushOverlay() } }
    /// The π meter (kb: techniques/pi-in-the-mandelbrot-set): at the neck -3/4 + iε or the
    /// cusp 1/4 + ε, the escape count times ε (√ε) closing in on π, drawn by Rust.
    public var piMeterEnabled = false { didSet { pushPiMeter() } }
    /// Extra right margin of the π panel (points): iPhone portrait keeps its buttons clear.
    public var piMeterInsetRight = 0.0 { didSet { if piMeterInsetRight != oldValue { pushPiMeter() } } }
    /// Cosmic size scale (kb: decisions/controls, "Cosmic size scale"): off by default.
    public var cosmicScale = false { didSet { pushCosmic() } }
    /// "screen" or a thing the whole set is as big as (`cosmicAnchors()`).
    public var cosmicAnchor = "screen" { didSet { pushCosmic() } }
    /// The compass (kb: decisions/controls, "Compass"): shown while the view is turned, or
    /// always.
    public var compassAlways = false { didSet { pushCompass() } }
    /// Extra top margin of the compass (points): iPad's top-right buttons.
    public var compassInsetTop = 0.0 { didSet { if compassInsetTop != oldValue { pushCompass() } } }
    /// Physical size of a view point (the platform view measures its screen; 0: unknown).
    var metresPerPoint = 0.0 { didSet { if metresPerPoint != oldValue { pushCosmic() } } }
    /// The status overlay (zoom, iterations, GPU time) in the top-left corner.
    public var showHUD: Bool = UserDefaults.standard.object(forKey: Preferences.hudKey) as? Bool ?? true {
        didSet { UserDefaults.standard.set(showHUD, forKey: Preferences.hudKey) }
    }

    // Navigation (an app-wide preference; the navigator in Rust applies it)
    public var edgePanning: EdgePanning = EdgePanning(rawValue: UserDefaults.standard.string(forKey: Preferences.edgePanKey) ?? "")
        ?? .whileZooming {
        didSet {
            UserDefaults.standard.set(edgePanning.rawValue, forKey: Preferences.edgePanKey)
            explorer?.setEdgePan(mode: edgePanning.ffi)
        }
    }

    /// Autopilot target lock: while the pointer rests the autopilot aims at boundary targets
    /// and backs out of voids (persisted; on by default; off: a pure joystick;
    /// `ALMONDBREAD_AUTOPILOT_LOCK=0|1` overrides it; kb: decisions/controls, autopilot).
    public var autopilotLock: Bool = UserDefaults.standard.object(forKey: ExplorerModel.lockKey) as? Bool ?? true {
        didSet {
            UserDefaults.standard.set(autopilotLock, forKey: Self.lockKey)
            explorer?.setAutopilotLock(on: autopilotLock)
        }
    }
    static let lockKey = "autopilotLock"

    /// The HUD's frame in SwiftUI's global space (the canvas shows the arrow cursor there).
    @ObservationIgnored public var hudFrame: CGRect? {
        didSet { if hudFrame != oldValue { host?.hudFrameChanged() } }
    }

    /// Renderer stats, refreshed at ~10 Hz while the view animates (views reading these
    /// re-render at that rate, never per input event: kb synthesis/benchmarks, M2 findings).
    public private(set) var stats: ExplorerStats?
    /// Autopilot / journey state alone, assigned only on change: the toolbar reads these, not
    /// `stats`, so it is not rebuilt at 10 Hz (that reset the hover timer and no tooltip showed).
    public private(set) var autopilotActive = false
    public private(set) var journeyActive = false
    /// The orbit overlay's point: its fate (refreshed with the stats while the overlay is on).
    public private(set) var orbit: OrbitInfo?
    /// The π meter's reading (refreshed with the stats while it is on).
    public private(set) var piMeter: PiMeterInfo?
    /// When the zoom speed (Q / E, handled in Rust) last changed, for the HUD highlight.
    public private(set) var zoomSpeedChanged = Date.distantPast
    /// The zoom speed (log of the factor per second), change-only like `autopilotActive`: views
    /// showing it (control center, controls help) do not depend on the 10 Hz `stats`.
    public private(set) var zoomSpeed = 1.6
    /// `stats` for the control center's readouts, at most 4 Hz (a discrete change, e.g. the
    /// autopilot stopping or the view settling, passes at once; the latest value always lands).
    /// Every update of a sidebar row re-lays out the whole open grouped form (~10 ms per update
    /// with one section open; kb: platform/swiftui-performance), so it should not run at 10 Hz.
    public private(set) var panelStats: ExplorerStats?
    @ObservationIgnored private var panelUpdated = 0.0
    @ObservationIgnored private var panelPending = false
    /// Why the renderer could not be created, if it could not.
    public internal(set) var failure: String?

    public static let palettes: [PaletteInfo] = AlmondbreadCore.palettes()
    /// The palette library (built-in by category, then imported ones; refreshed on import).
    public private(set) var library: [LibraryPaletteInfo] = AlmondbreadCore.paletteLibrary()
    static let userPalettesKey = "almondbread.userPalettes.v1"
    public static let looks: [LookInfo] = AlmondbreadCore.looks()

    @ObservationIgnored private(set) var explorer: Explorer?
    @ObservationIgnored private weak var host: ExplorerHost?
    @ObservationIgnored private var pulling = false
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var defaultsObserver: NSObjectProtocol?

    public init() {}

    /// Connects the view's explorer; the model takes over Rust's settings, then restores the
    /// saved ones (window tests start from the defaults).
    func attach(_ explorer: Explorer, host: ExplorerHost) {
        self.explorer = explorer
        self.host = host
        explorer.setEdgePan(mode: edgePanning.ffi)
        explorer.setAutopilotLock(on: autopilotLock)
        if !TestLaunch.enabled { restoreUserPalettes() }
        pull(explorer.settings())
        if Preferences.restoreSettings && !TestLaunch.enabled { restore() }
        pushOverlay()
        pushPiMeter()
        pushCosmic()
        pushCompass()
        refreshStats()
        // The Places grid's thumbnails, decoded in the background once the renderer exists, so
        // the control center shows them at once (kb: platform/swiftui-performance).
        PlaceThumbnails.preload()
        // The Settings window writes user defaults: follow edge panning and the HUD.
        defaultsObserver = NotificationCenter.default.addObserver(forName: UserDefaults.didChangeNotification,
                                                                  object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.followPreferences() }
        }
    }

    private func followPreferences() {
        let d = UserDefaults.standard
        if let e = EdgePanning(rawValue: d.string(forKey: Preferences.edgePanKey) ?? ""), e != edgePanning {
            edgePanning = e
        }
        let hud = d.object(forKey: Preferences.hudKey) as? Bool ?? true
        if hud != showHUD { showHUD = hud }
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
        cinematic = s.cinematic
        cinematicSpin = s.cinematicSpin
        cinematicHueDrift = s.cinematicHueDrift
        cinematicPhaseDrift = s.cinematicPhaseDrift
        cinematicFlyCycle = s.cinematicFlyCycle
        supersampling = Supersampling(rawValue: Int(s.supersampling)) ?? .off
        interactivePreview = s.interactivePreview
        interiorColoring = InteriorColoring(s.interiorMode)
        interiorDensity = s.interiorDensity
        interiorStripes = s.interiorStripes
        denseCalm = s.denseCalm
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
            supersampling: UInt8(supersampling.rawValue), interactivePreview: interactivePreview,
            interiorMode: interiorColoring.ffi, interiorDensity: interiorDensity, interiorStripes: interiorStripes,
            denseCalm: denseCalm, cinematic: cinematic, cinematicSpin: cinematicSpin,
            cinematicHueDrift: cinematicHueDrift, cinematicPhaseDrift: cinematicPhaseDrift,
            cinematicFlyCycle: cinematicFlyCycle)
    }

    private var overlaySettings: OverlaySettings {
        OverlaySettings(orbit: orbitEnabled, orbitLength: UInt32(min(max(orbitLength, 1), 1000)),
                        map: mapEnabled, mapLevels: UInt32(min(max(mapLevels, 1), 4)),
                        mapSize: mapSize, mapCorner: mapPlacement.ffi)
    }

    /// Applies a look (palette and colouring) in Rust and takes over the result.
    public func applyLook(_ id: String) {
        guard let explorer else { return }
        lookID = id
        pull(explorer.applyLook(id: id))
        host?.requestFrames()
        save()
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
        save()
    }

    private func pushOverlay() {
        guard !pulling, let explorer else { return }
        explorer.setOverlay(settings: overlaySettings)
        host?.requestFrames()
        save()
    }

    private func pushPiMeter() {
        guard !pulling, let explorer else { return }
        explorer.setPiMeter(on: piMeterEnabled, insetRight: piMeterInsetRight, insetBottom: 0)
        if !piMeterEnabled { piMeter = nil }
        host?.requestFrames()
        save()
    }

    private func pushCosmic() {
        guard !pulling, let explorer else { return }
        explorer.setCosmicScale(settings: CosmicScaleSettings(enabled: cosmicScale, anchor: cosmicAnchor,
                                                              metresPerPoint: metresPerPoint))
        host?.requestFrames()
        save()
    }

    private func pushCompass() {
        guard !pulling, let explorer else { return }
        explorer.setCompass(always: compassAlways, insetTop: compassInsetTop)
        host?.requestFrames()
        save()
    }

    // MARK: Persistence

    /// Writes the settings to user defaults (coalesced: at most every half second).
    private func save() {
        guard !pulling, explorer != nil, !TestLaunch.enabled else { return }
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, let self else { return }
            self.writeSaved()
        }
    }

    /// Everything that persists, as a record (also the undo state of the resets).
    func snapshot() -> SavedSettings {
        SavedSettings(
            autoIterations: autoIterations, manualIterations: manualIterations, maxIterationsCap: maxIterationsCap,
            interiorDetection: interiorDetection, paletteID: paletteID, colorDensity: colorDensity,
            colorOffset: colorOffset, logColoring: logColoring, smoothColoring: smoothColoring,
            interiorColor: [interiorColor.x, interiorColor.y, interiorColor.z], shading: shading,
            lightAngle: lightAngle, outline: outline, outlineWidth: outlineWidth,
            edgeColor: [edgeColor.x, edgeColor.y, edgeColor.z], specular: specular, shininess: shininess,
            lightHeight: lightHeight, glow: glow, glowColor: [glowColor.x, glowColor.y, glowColor.z],
            glowWidth: glowWidth, paletteFilter: paletteFilter, cycleSpeed: cycleSpeed, depthTint: depthTint,
            cinematic: cinematic, cinematicSpin: cinematicSpin, cinematicHueDrift: cinematicHueDrift,
            cinematicPhaseDrift: cinematicPhaseDrift, cinematicFlyCycle: cinematicFlyCycle,
            lookID: lookID, supersampling: supersampling, interactivePreview: interactivePreview,
            interiorColoring: interiorColoring, interiorDensity: interiorDensity, interiorStripes: interiorStripes,
            denseCalm: denseCalm, orbitEnabled: orbitEnabled, orbitLength: orbitLength, mapEnabled: mapEnabled,
            mapLevels: mapLevels, mapSize: mapSize, mapPlacement: mapPlacement, piMeterEnabled: piMeterEnabled,
            cosmicScale: cosmicScale, cosmicAnchor: cosmicAnchor, compassAlways: compassAlways)
    }

    private func writeSaved() {
        if let data = try? JSONEncoder().encode(snapshot()) {
            UserDefaults.standard.set(data, forKey: Preferences.savedSettingsKey)
        }
    }

    /// Takes over the saved settings (missing keys keep the current values) and forwards them.
    private func restore() {
        guard let data = UserDefaults.standard.data(forKey: Preferences.savedSettingsKey),
              let s = try? JSONDecoder().decode(SavedSettings.self, from: data) else { return }
        apply(s)
    }

    /// Takes over a settings record (missing keys keep the current values) and forwards it.
    func apply(_ s: SavedSettings) {
        pulling = true
        func rgb(_ v: [Float]?) -> SIMD3<Float>? { v.flatMap { $0.count == 3 ? SIMD3($0[0], $0[1], $0[2]) : nil } }
        autoIterations = s.autoIterations ?? autoIterations
        manualIterations = s.manualIterations ?? manualIterations
        maxIterationsCap = s.maxIterationsCap ?? maxIterationsCap
        interiorDetection = s.interiorDetection ?? interiorDetection
        paletteID = s.paletteID ?? paletteID
        colorDensity = s.colorDensity ?? colorDensity
        colorOffset = s.colorOffset ?? colorOffset
        logColoring = s.logColoring ?? logColoring
        smoothColoring = s.smoothColoring ?? smoothColoring
        interiorColor = rgb(s.interiorColor) ?? interiorColor
        shading = s.shading ?? shading
        lightAngle = s.lightAngle ?? lightAngle
        outline = s.outline ?? outline
        outlineWidth = s.outlineWidth ?? outlineWidth
        edgeColor = rgb(s.edgeColor) ?? edgeColor
        specular = s.specular ?? specular
        shininess = s.shininess ?? shininess
        lightHeight = s.lightHeight ?? lightHeight
        glow = s.glow ?? glow
        glowColor = rgb(s.glowColor) ?? glowColor
        glowWidth = s.glowWidth ?? glowWidth
        paletteFilter = s.paletteFilter ?? paletteFilter
        cycleSpeed = s.cycleSpeed ?? cycleSpeed
        depthTint = s.depthTint ?? depthTint
        cinematic = s.cinematic ?? cinematic
        cinematicSpin = s.cinematicSpin ?? cinematicSpin
        cinematicHueDrift = s.cinematicHueDrift ?? cinematicHueDrift
        cinematicPhaseDrift = s.cinematicPhaseDrift ?? cinematicPhaseDrift
        cinematicFlyCycle = s.cinematicFlyCycle ?? cinematicFlyCycle
        lookID = s.lookID ?? lookID
        supersampling = s.supersampling ?? supersampling
        interactivePreview = s.interactivePreview ?? interactivePreview
        interiorColoring = s.interiorColoring ?? interiorColoring
        interiorDensity = s.interiorDensity ?? interiorDensity
        interiorStripes = s.interiorStripes ?? interiorStripes
        denseCalm = s.denseCalm ?? denseCalm
        orbitEnabled = s.orbitEnabled ?? orbitEnabled
        orbitLength = s.orbitLength ?? orbitLength
        mapEnabled = s.mapEnabled ?? mapEnabled
        mapLevels = s.mapLevels ?? mapLevels
        mapSize = s.mapSize ?? mapSize
        mapPlacement = s.mapPlacement ?? mapPlacement
        piMeterEnabled = s.piMeterEnabled ?? piMeterEnabled
        cosmicScale = s.cosmicScale ?? cosmicScale
        cosmicAnchor = s.cosmicAnchor ?? cosmicAnchor
        compassAlways = s.compassAlways ?? compassAlways
        pulling = false
        push()
        pushOverlay()
        pushCosmic()
        pushPiMeter()
        pushCompass()
    }

    // MARK: Reset (control center: "Reset all" and per section)

    /// What a reset covers.
    public enum ResetScope: Sendable {
        case all, view, look, lighting, interior, overlays, quality, cinematic
    }

    /// Resets `scope` to the defaults (`withView`: also the view, for "all"). Undoable: the
    /// previous settings (and view) go on `undo` (⌘Z).
    public func reset(_ scope: ResetScope, withView: Bool = false, undo: UndoManager? = nil) {
        guard let explorer else { return }
        let before = snapshot()
        let view = explorer.viewport()
        let d = explorer.defaultSettings()
        pulling = true
        func rgb(_ r: Float, _ g: Float, _ b: Float) -> SIMD3<Float> { SIMD3(r, g, b) }
        let look = scope == .all || scope == .look
        let light = scope == .all || scope == .lighting
        if look {
            paletteID = d.paletteId
            colorDensity = d.colorDensity
            colorOffset = d.colorOffset
            logColoring = d.logColoring
            smoothColoring = d.smoothColoring
            paletteFilter = d.paletteFilter
            denseCalm = d.denseCalm
            cycleSpeed = d.cycleSpeed
            depthTint = d.depthTint
            lookID = Self.looks.first?.id ?? lookID
        }
        if light {
            shading = d.shading
            lightAngle = d.lightAngle
            lightHeight = d.lightHeight
            specular = d.specular
            shininess = d.shininess
            outline = d.outline
            outlineWidth = d.outlineWidth
            edgeColor = rgb(d.edgeRed, d.edgeGreen, d.edgeBlue)
            glow = d.glow
            glowColor = rgb(d.glowRed, d.glowGreen, d.glowBlue)
            glowWidth = d.glowWidth
        }
        if scope == .all || scope == .interior {
            interiorColoring = InteriorColoring(d.interiorMode)
            interiorDensity = d.interiorDensity
            interiorStripes = d.interiorStripes
            interiorColor = rgb(d.interiorRed, d.interiorGreen, d.interiorBlue)
            interiorDetection = d.interiorDetection
        }
        if scope == .all || scope == .quality {
            autoIterations = d.autoIterations
            manualIterations = Int(d.manualIterations)
            maxIterationsCap = Int(d.maxIterationsCap)
            supersampling = Supersampling(rawValue: Int(d.supersampling)) ?? .x9
            interactivePreview = d.interactivePreview
        }
        if scope == .all || scope == .overlays {
            orbitEnabled = false
            orbitLength = 50
            mapEnabled = false
            mapLevels = 3
            mapSize = 180
            mapPlacement = .bottomLeft
            piMeterEnabled = false
            cosmicScale = false
            cosmicAnchor = "screen"
            compassAlways = false
        }
        pulling = false
        if scope == .all || scope == .overlays { showHUD = true }
        if scope == .all || scope == .view {
            edgePanning = .whileZooming
            autopilotLock = true
        }
        if scope == .all || scope == .view || scope == .cinematic {
            pulling = true
            cinematic = d.cinematic
            cinematicSpin = d.cinematicSpin
            cinematicHueDrift = d.cinematicHueDrift
            cinematicPhaseDrift = d.cinematicPhaseDrift
            cinematicFlyCycle = d.cinematicFlyCycle
            pulling = false
        }
        push()
        pushOverlay()
        pushPiMeter()
        pushCosmic()
        pushCompass()
        if scope == .view || (scope == .all && withView) { reset() }
        undo?.registerUndo(withTarget: self) { m in
            MainActor.assumeIsolated {
                m.apply(before)
                if scope == .view || (scope == .all && withView) {
                    try? m.explorer?.setViewportText(re: view.re, im: view.im, scale: view.scaleText)
                    m.host?.requestFrames()
                }
            }
        }
        undo?.setActionName(scope == .all ? "Reset All" : "Reset Section")
    }

    /// Back to the defaults (the default look, overlays off) and forgets the saved settings.
    public func resetSettings() {
        UserDefaults.standard.removeObject(forKey: Preferences.savedSettingsKey)
        pulling = true
        autoIterations = true
        manualIterations = 1000
        maxIterationsCap = 200_000
        interiorDetection = true
        interiorColoring = .flat
        interiorDensity = 0.25
        interiorStripes = 8
        supersampling = .x9
        interactivePreview = true
        cycleSpeed = 0
        depthTint = 0
        cinematic = false
        cinematicSpin = 4
        cinematicHueDrift = 1
        cinematicPhaseDrift = 0.01
        cinematicFlyCycle = 0.02
        orbitEnabled = false
        orbitLength = 50
        mapEnabled = false
        mapLevels = 3
        mapSize = 180
        mapPlacement = .bottomLeft
        piMeterEnabled = false
        cosmicScale = false
        cosmicAnchor = "screen"
        compassAlways = false
        pulling = false
        push()
        pushOverlay()
        pushCosmic()
        pushPiMeter()
        pushCompass()
        applyLook("aurora")
    }

    // MARK: Actions (menu commands, control center buttons)

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

    // MARK: Journey and rotation (kb: decisions/controls, "Journey", "View rotation")

    /// J: a journey with a random seed; while one runs, stops it; after manual input, resumes.
    public func toggleJourney() {
        send(.keyDown(key: .journey))
        send(.keyUp(key: .journey))
    }

    /// A journey with a given seed ("Journey #4711" is reproducible from the same start view).
    public func startJourney(seed: UInt64) {
        explorer?.startJourney(seed: seed)
        host?.requestFrames()
    }

    /// Today's journey: the same for everyone on this date.
    public func startDailyJourney() {
        let c = Calendar.current.dateComponents([.year, .month, .day], from: Date())
        explorer?.startDailyJourney(year: Int32(c.year ?? 2026), month: UInt32(c.month ?? 1), day: UInt32(c.day ?? 1))
        host?.requestFrames()
    }

    public func resumeJourney() {
        explorer?.resumeJourney()
        host?.requestFrames()
    }

    public func stopJourney() {
        explorer?.stopJourney()
        host?.requestFrames()
    }

    /// The running journey as shareable text ("Almondbread journey #4711"), or nil.
    public var journeyShareText: String? {
        guard let j = stats?.journey, j.active else { return nil }
        return j.daily ? "Almondbread daily journey #\(j.seed)" : "Almondbread journey #\(j.seed)"
    }

    /// Copies the journey's seed text to the clipboard.
    public func copyJourney() {
        if let t = journeyShareText { Clipboard.copy(t) }
    }

    /// Parses "#4711", "journey 4711" or "4711" into a seed.
    public static func journeySeed(from text: String) -> UInt64? {
        let digits = text.split(whereSeparator: { !$0.isNumber }).last.map(String.init) ?? ""
        guard let v = UInt64(digits), v > 0 else { return nil }
        return v
    }

    /// Turns the view upright (animated), like `/`.
    public func straighten() {
        send(.keyDown(key: .straighten))
        send(.keyUp(key: .straighten))
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

    // MARK: Places (the catalogue in the Rust core; kb: synthesis/places)

    public static let places: [PlaceInfo] = AlmondbreadCore.places()

    /// Jumps to a place (its view, look and iteration limit).
    public func goToPlace(_ id: String) {
        guard let explorer, let s = try? explorer.goToPlace(id: id) else { return }
        lookID = ExplorerModel.places.first { $0.id == id }?.look ?? lookID
        pull(s)
        showPlaceInterior()
        host?.requestFrames()
    }

    /// Flies to a place (the fly-to path) and takes its look.
    public func flyToPlace(_ id: String) {
        guard let explorer, let s = try? explorer.flyToPlace(id: id) else { return }
        lookID = ExplorerModel.places.first { $0.id == id }?.look ?? lookID
        pull(s)
        showPlaceInterior()
        host?.requestFrames()
    }

    /// The place thumbnails show the interior in the bubbles colouring (a flat black interior
    /// hid most of the landmarks): a place visited with the flat interior switches to it, so
    /// the view looks like its thumbnail. Any other interior colouring the user chose stays.
    private func showPlaceInterior() {
        if interiorColoring == .flat { interiorColoring = .bubbles }
    }

    /// A random place (optionally of one category); `fly` flies there instead of jumping.
    @discardableResult
    public func randomPlace(category: PlaceCategory? = nil, fly: Bool = false) -> PlaceInfo {
        let p = AlmondbreadCore.randomPlace(seed: UInt64.random(in: 0...UInt64.max), category: category)
        if fly { flyToPlace(p.id) } else { goToPlace(p.id) }
        return p
    }

    public func toggleOrbit() { orbitEnabled.toggle() }
    public func toggleMap() { mapEnabled.toggle() }
    public func toggleCosmicScale() { cosmicScale.toggle() }
    public func togglePiMeter() { piMeterEnabled.toggle() }

    /// The guided π flight ("neck" or "cusp"): to the place, meter on, then down the line of
    /// approach (Rust flies it; Esc stops).
    public func piFlight(_ point: String) {
        guard let explorer, let s = try? explorer.piFlight(point: point) else { return }
        let id = point == "cusp" ? "pi-at-the-cusp" : "pi-at-the-neck"
        lookID = ExplorerModel.places.first { $0.id == id }?.look ?? lookID
        pull(s)
        showPlaceInterior()
        if !piMeterEnabled { piMeterEnabled = true }
        host?.requestFrames()
    }

    /// A click on the canvas at (x, y) points from the centre, y up: true if a mini-map panel
    /// took it (a jump follows).
    func mapClick(x: Double, y: Double, fly: Bool) -> Bool {
        guard let explorer, mapEnabled || orbitEnabled else { return false }
        let taken = explorer.mapClick(x: x, y: y, fly: fly)
        if taken { host?.requestFrames() }
        return taken
    }

    /// A press on the canvas at (x, y) points from the centre, y up: true if the compass took it
    /// (send its drag and release here too; kb: decisions/controls "Compass").
    func compassPress(x: Double, y: Double) -> Bool {
        guard let explorer else { return false }
        let taken = explorer.compassPress(x: x, y: y)
        if taken { host?.requestFrames() }
        return taken
    }

    func compassDrag(x: Double, y: Double) {
        explorer?.compassDrag(x: x, y: y)
        host?.requestFrames()
    }

    func compassRelease() {
        explorer?.compassRelease()
        host?.requestFrames()
    }

    /// A tap (iOS): on the compass it turns the view to north and returns true.
    func compassTap(x: Double, y: Double) -> Bool {
        guard compassPress(x: x, y: y) else { return false }
        compassRelease()
        return true
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

    /// Uses a palette given as stops (e.g. from a palette library); empty: the built-in one.
    public func setCustomPalette(_ stops: [PaletteStop]) {
        explorer?.setCustomPalette(stops: stops)
        host?.requestFrames()
    }

    // MARK: Stats

    /// Reads the renderer's stats (called by the view at ~10 Hz while animating, and once
    /// when it stops). Assigns only on change, so idle views do not re-render.
    func refreshStats() {
        if orbitEnabled || mapEnabled, let o = explorer?.orbitInfo(), o != orbit { orbit = o }
        if piMeterEnabled, let m = explorer?.piMeter(), m != piMeter { piMeter = m }
        guard let new = explorer?.stats(), new != stats else { return }
        if let old = stats, old.zoomSpeed != new.zoomSpeed || old.autopilot.speed != new.autopilot.speed
            || old.flight.speed != new.flight.speed || old.journey.speed != new.journey.speed {
            zoomSpeedChanged = Date()
        }
        stats = new
        updatePanel(new)
        if zoomSpeed != new.zoomSpeed { zoomSpeed = new.zoomSpeed }
        if autopilotActive != new.autopilot.active { autopilotActive = new.autopilot.active }
        if journeyActive != new.journey.active { journeyActive = new.journey.active }
    }

    private func updatePanel(_ new: ExplorerStats) {
        let now = Date.timeIntervalSinceReferenceDate
        let discrete = panelStats.map {
            $0.autopilot.active != new.autopilot.active || $0.flight.active != new.flight.active
                || $0.journey.active != new.journey.active || $0.journey.suspended != new.journey.suspended
                || $0.settled != new.settled
        } ?? true
        if discrete || now - panelUpdated >= 0.25 {
            panelStats = new
            panelUpdated = now
            return
        }
        guard !panelPending else { return }
        panelPending = true
        DispatchQueue.main.asyncAfter(deadline: .now() + max(0.01, 0.25 - (now - panelUpdated))) { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.panelPending = false
                if let s = self.stats, s != self.panelStats {
                    self.panelStats = s
                    self.panelUpdated = Date.timeIntervalSinceReferenceDate
                }
            }
        }
    }

    /// Zoom speed as a magnification factor per second, for display.
    public var zoomFactorPerSecond: Double { exp(zoomSpeed) }

    public var palette: PaletteInfo {
        if let p = Self.palettes.first(where: { $0.id == paletteID }) { return p }
        if let l = library.first(where: { $0.id == paletteID }) {
            return PaletteInfo(id: l.id, name: l.name, stops: l.stops)
        }
        return Self.palettes[0]
    }

    // MARK: Palette library (kb: techniques/palette-design)

    /// Applies a library palette with its recommended mapping and deep colour.
    public func applyPalette(_ id: String) {
        guard let explorer else { return }
        pull(explorer.applyPalette(id: id))
        host?.requestFrames()
        save()
    }

    /// Imports palette files (.ugr, .map, almondbread JSON): registers them in Rust, keeps
    /// their JSON in user defaults (registered again at launch) and returns how many came in.
    public func importPalettes(from url: URL) throws -> Int {
        let access = url.startAccessingSecurityScopedResource()
        defer { if access { url.stopAccessingSecurityScopedResource() } }
        let data = try Data(contentsOf: url)
        let text = String(decoding: data, as: UTF8.self)
        let jsons = try AlmondbreadCore.importPalettes(text: text, fileName: url.lastPathComponent)
        var saved = UserDefaults.standard.stringArray(forKey: Self.userPalettesKey) ?? []
        saved.append(contentsOf: jsons)
        UserDefaults.standard.set(saved, forKey: Self.userPalettesKey)
        library = AlmondbreadCore.paletteLibrary()
        return jsons.count
    }

    /// Registers the palettes imported in earlier sessions.
    private func restoreUserPalettes() {
        for json in UserDefaults.standard.stringArray(forKey: Self.userPalettesKey) ?? [] {
            _ = try? AlmondbreadCore.registerPaletteJson(json: json)
        }
        library = AlmondbreadCore.paletteLibrary()
    }
}
