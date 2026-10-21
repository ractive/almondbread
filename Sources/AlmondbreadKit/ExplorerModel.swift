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
    public static let hudDetailsKey = "hudDeveloperDetails"
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
    var juliaEnabled: Bool?
    var cosmicScale: Bool?
    var compassAlways: Bool?
    var targetMarker: Bool?
    /// The fractal (F2): "mandelbrot".
    var formula: String?
}

/// The UI's view of one window's explorer: a mirror of the Rust settings that SwiftUI binds
/// to (every change is forwarded, an infrequent control-plane call) and a ~10 Hz snapshot
/// of the renderer's stats for the HUD and control center. Rendering, motion and all policy
/// live in Rust (`Explorer`, kb: platform/render-architecture); nothing here runs per frame.
/// Settings persist in user defaults (restored at launch unless the preference is off).
@MainActor
@Observable
public final class ExplorerModel {
    /// The fractal (F2, kb: plans/f2-pluggable-formulas): "mandelbrot" until F3 adds more.
    public var formula = "mandelbrot" {
        didSet {
            push()
            if !pulling, let c = explorer?.capabilities(), c != capabilities { capabilities = c }
        }
    }
    /// What the formula supports (Rust's `capabilities()`, taken over with the settings); the
    /// UI hides what it lacks. Nil before the renderer exists: everything (Mandelbrot).
    public private(set) var capabilities: FormulaCapabilities?

    /// Whether the current formula has a feature (true before the renderer exists).
    public func has(_ feature: KeyPath<FormulaCapabilities, Bool>) -> Bool {
        capabilities?[keyPath: feature] ?? true
    }

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
    /// The Julia companion panel: the Julia set of the orbit's point (kb: plans/f1-julia-view).
    public var juliaEnabled = false { didSet { pushOverlay() } }
    /// The π meter (kb: techniques/pi-in-the-mandelbrot-set): at the neck -3/4 + iε or the
    /// cusp 1/4 + ε, the escape count times ε (√ε) closing in on π, drawn by Rust. Only the
    /// tour "π in the Mandelbrot set" switches it on (mirrored here); not saved.
    public var piMeterEnabled = false { didSet { pushPiMeter() } }
    /// Extra right margin of the π panel (points): iPhone portrait keeps its buttons clear.
    /// The HUD's zoom at a place in this window minus the place's catalogue zoom (log10;
    /// `Explorer.placeZoomOffset`), cached at each resize. Not observed: the place pills read it
    /// when Places draws, and a live window resize does not redraw 79 tiles per frame.
    @ObservationIgnored var placeZoomOffset = 0.0
    public var piMeterInsetRight = 0.0 { didSet { if piMeterInsetRight != oldValue { pushPiMeter() } } }
    /// Bottom and top margins of the π panel (points): a phone's bottom button row, the iPad's
    /// top-right buttons (the panel moves up on a narrow canvas but never above them).
    public var piMeterInsetBottom = 0.0 { didSet { if piMeterInsetBottom != oldValue { pushPiMeter() } } }
    public var piMeterInsetTop = 0.0 { didSet { if piMeterInsetTop != oldValue { pushPiMeter() } } }
    /// The tour card's frame (global coordinates) while it shows: the π panel keeps clear of it
    /// (kb: decisions/controls "π meter on narrow canvases").
    @ObservationIgnored var tourCardFrame: CGRect? { didSet { if tourCardFrame != oldValue { pushTourCard() } } }
    /// The canvas's origin in global coordinates (the tour card's frame is converted with it).
    @ObservationIgnored var canvasOrigin: CGPoint = .zero { didSet { if canvasOrigin != oldValue { pushTourCard() } } }
    /// The Mac control center's width (points; it reports a width while hidden too, so the
    /// content view applies it only while it shows).
    public var controlCenterWidth = 0.0
    /// Right margin of the cosmic scale (points): the iPhone's button column, the throttle.
    public var cosmicInsetRight = 0.0 { didSet { if cosmicInsetRight != oldValue { pushCosmic() } } }
    /// Cosmic size scale (kb: decisions/controls, "Cosmic size scale"): off by default.
    public var cosmicScale = false { didSet { pushCosmic() } }
    /// The compass (kb: decisions/controls, "Compass"): shown while the view is turned, or
    /// always.
    public var compassAlways = false { didSet { pushCompass() } }
    /// The autopilot's target marker (kb: decisions/controls, "Target marker"): a ring where
    /// the autopilot or a journey leg is heading, drawn by Rust. On by default.
    public var targetMarker = true { didSet { pushCompass() } }
    /// Extra top margin of the compass (points): iPad's top-right buttons.
    public var compassInsetTop = 0.0 { didSet { if compassInsetTop != oldValue { pushCompass() } } }
    /// Physical size of a view point (the platform view measures its screen; 0: unknown).
    var metresPerPoint = 0.0 { didSet { if metresPerPoint != oldValue { pushCosmic() } } }
    /// The status overlay (zoom, centre, speed) in the top-left corner.
    public var showHUD: Bool = TestLaunch.ephemeral || UserDefaults.standard.object(forKey: Preferences.hudKey) as? Bool ?? true {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(showHUD, forKey: Preferences.hudKey) }
            if showHUD != oldValue { pushCockpit() }
        }
    }
    /// Extra right margin of the HUD cockpit (points; drawn by Rust at the bottom of the canvas):
    /// iPhone portrait keeps its buttons clear.
    public var cockpitInsetRight = 0.0 { didSet { if cockpitInsetRight != oldValue { pushCockpit() } } }
    /// The HUD's developer details (iterations, GPU, reference, kernel, state): off by default.
    /// A demo window does not store it (it would change the user's preference).
    public var hudDetails: Bool = !TestLaunch.ephemeral && UserDefaults.standard.bool(forKey: Preferences.hudDetailsKey) {
        didSet { if !TestLaunch.ephemeral { UserDefaults.standard.set(hudDetails, forKey: Preferences.hudDetailsKey) } }
    }

    // Navigation (an app-wide preference; the navigator in Rust applies it)
    public var edgePanning: EdgePanning = EdgePanning(rawValue: UserDefaults.standard.string(forKey: Preferences.edgePanKey) ?? "")
        ?? .whileZooming {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(edgePanning.rawValue, forKey: Preferences.edgePanKey) }
            explorer?.setEdgePan(mode: edgePanning.ffi)
        }
    }

    /// Autopilot target lock: while the pointer rests the autopilot aims at boundary targets
    /// and backs out of voids (persisted; on by default; off: a pure joystick; kb:
    /// decisions/controls, autopilot).
    public var autopilotLock: Bool = UserDefaults.standard.object(forKey: ExplorerModel.lockKey) as? Bool ?? true {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(autopilotLock, forKey: Self.lockKey) }
            explorer?.setAutopilotLock(on: autopilotLock)
        }
    }
    static let lockKey = "autopilotLock"

    /// Lane assist (key L; persisted, off by default; active in cruise only; kb:
    /// decisions/controls "Driving levels"): keeps a cruise out of featureless areas.
    /// Window tests start from (and never store) the default.
    public var laneAssist: Bool = !TestLaunch.ephemeral && UserDefaults.standard.bool(forKey: ExplorerModel.assistKey) {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(laneAssist, forKey: Self.assistKey) }
            explorer?.setAssist(on: laneAssist)
            host?.requestFrames()
        }
    }
    static let assistKey = "laneAssist"

    /// Two-finger scroll on a trackpad: false pans (default), true zooms (up and down; click
    /// and drag pans). Persisted; window tests start from (and never store) the default.
    public var scrollZooms: Bool = !TestLaunch.ephemeral && UserDefaults.standard.bool(forKey: ExplorerModel.scrollZoomsKey) {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(scrollZooms, forKey: Self.scrollZoomsKey) }
            explorer?.setScrollZooms(on: scrollZooms)
        }
    }
    static let scrollZoomsKey = "scrollZooms"

    /// ⌘F: the control center's search field takes focus (counted, so each press is a change;
    /// pending until a field that has just appeared takes it).
    var searchFocusRequest = 0
    @ObservationIgnored var searchFocusPending = false
    func requestSearchFocus() {
        searchFocusPending = true
        searchFocusRequest += 1
    }

    /// "Journey: continuous / stop at each find" (persisted; off: continuous; kb:
    /// decisions/controls "Journey with stops"): J and the cockpit's journey icon start this mode, ⇧J
    /// the other.
    /// Window tests start from (and never store) the default.
    public var journeyStops: Bool = !TestLaunch.ephemeral && UserDefaults.standard.bool(forKey: ExplorerModel.journeyStopsKey) {
        didSet {
            if !TestLaunch.ephemeral { UserDefaults.standard.set(journeyStops, forKey: Self.journeyStopsKey) }
            explorer?.setJourneyStops(on: journeyStops)
        }
    }
    static let journeyStopsKey = "journeyStops"

    /// The HUD's frame in SwiftUI's global space (the canvas shows the arrow cursor there).
    /// The window is in (or entering) full screen (macOS): the control center closes then and
    /// comes back as it was on leaving (ContentView; kb: decisions/controls "Full screen").
    public var fullScreen = false

    /// The popover of a Rust-drawn help badge the user pressed (kb: decisions/controls "Help");
    /// set on the press, nil when the popover closes. Nothing else changes it.
    var canvasHelp: CanvasHelp? {
        didSet { if let canvasHelp { lastCanvasHelp = canvasHelp } }
    }
    /// The last badge pressed: the popover's anchor while it closes.
    @ObservationIgnored var lastCanvasHelp: CanvasHelp?
    #if os(iOS)
    /// The long-press menu at a point of the canvas (kb: decisions/controls "Touch"), and the
    /// last one (its anchor while it closes).
    var canvasMenu: CanvasMenu? {
        didSet { if let canvasMenu { lastCanvasMenu = canvasMenu } }
    }
    @ObservationIgnored var lastCanvasMenu: CanvasMenu?
    #endif

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
    /// More change-only state for the control center's buttons (the sidebar shows no live
    /// values: kb platform/swiftui-performance): a flight runs, the journey is paused, its
    /// seed, the view is turned.
    public private(set) var flightActive = false
    public private(set) var journeySuspended = false
    public private(set) var journeySeed: UInt64 = 0
    /// The journey rests at a landmark and waits for a choice (J, Z, X; iOS: buttons).
    public private(set) var journeyAtStop = false
    /// The driving level (kb: decisions/controls "Driving levels"), change-only.
    public private(set) var drivingLevel = DrivingLevel.manual
    /// The set speed's gear (1–26, negative in reverse, 0: none), change-only: the iOS throttle.
    public private(set) var cruiseGear = 0
    public private(set) var viewTurned = false
    /// The orbit overlay's point: its fate (refreshed with the stats while the overlay is on).
    public private(set) var orbit: OrbitInfo?
    /// The π meter's reading (refreshed with the stats while it is on).
    public private(set) var piMeter: PiMeterInfo?
    /// The HUD's render progress line ("refining · 2.1 M of 10 M steps · 3 % of pixels left ·
    /// ~40 s"; empty: none), change-only so only its leaf view re-renders (kb:
    /// decisions/controls "Render progress").
    public private(set) var progressText = ""
    /// Compute further stopped at its time limit: the HUD offers "Continue" (G).
    public private(set) var canContinue = false
    /// The progress line is the settled "12 % not finished" one: points left as fog (the HUD
    /// links the tour of topic `fog` there).
    public private(set) var progressUnfinished = false
    /// When the zoom speed (Q / E, handled in Rust) last changed, for the HUD highlight.
    public private(set) var zoomSpeedChanged = Date.distantPast
    /// The zoom speed (log of the factor per second), change-only like `autopilotActive`: views
    /// showing it (control center, controls help) do not depend on the 10 Hz `stats`.
    public private(set) var zoomSpeed = 1.6
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
        explorer.setJourneyStops(on: journeyStops)
        explorer.setAssist(on: laneAssist)
        explorer.setScrollZooms(on: scrollZooms)
        if !TestLaunch.ephemeral { restoreUserPalettes() }
        pull(explorer.settings())
        if Preferences.restoreSettings && !TestLaunch.ephemeral { restore() }
        pushOverlay()
        pushPiMeter()
        pushCosmic()
        pushCompass()
        pushCockpit()
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
        let details = d.bool(forKey: Preferences.hudDetailsKey)
        if !UIDemo.enabled, details != hudDetails { hudDetails = details }
    }

    // MARK: Settings

    private func pull(_ s: ExplorerSettings) {
        pulling = true
        defer { pulling = false }
        formula = s.formula
        if let c = explorer?.capabilities(), c != capabilities { capabilities = c }
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
            formula: formula,
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
                        mapSize: mapSize, mapCorner: mapPlacement.ffi, julia: juliaEnabled)
    }

    // MARK: Density edits

    /// The pivot of the density drag in progress (kb: techniques/coloring-methods, "Density
    /// pivot"): taken once when the drag starts, so one drag keeps one anchor. `.some(nil)`:
    /// a drag without a pivot (no escaped pixels yet).
    @ObservationIgnored private var densityDragPivot: Double??

    /// The density control's drag starts / ends (the slider's editing state).
    public func densityEditing(_ editing: Bool) {
        densityDragPivot = editing ? .some(explorer?.densityPivot()) : nil
    }

    /// A user edit of the density (the control center's slider, its keyboard steps): the
    /// offset moves with it so the colour at the view's median escape count stays (Rust's
    /// `pivotedColorOffset`); bands widen or narrow around it instead of scrambling at depth.
    /// Programmatic changes (looks, places, tours, reset) set `colorDensity` directly.
    public func editColorDensity(_ density: Double) {
        guard density != colorDensity, let explorer else {
            colorDensity = density
            return
        }
        let pivot = densityDragPivot ?? explorer.densityPivot()
        let offset = pivotedColorOffset(offset: colorOffset, from: colorDensity, to: density,
                                        pivot: pivot, logScale: logColoring)
        // One push for both.
        pulling = true
        colorOffset = offset
        pulling = false
        colorDensity = density
    }

    /// Applies a look (palette and colouring) in Rust and takes over the result.
    public func applyLook(_ id: String) {
        guard let explorer else { return }
        lookID = id
        pull(explorer.applyLook(id: id))
        host?.requestFrames()
        save()
    }

    private func push(_ origin: StaticString = #function) {
        guard !pulling, let explorer else { return }
        HangWatchdog.note("settings", "push from \(origin)")
        do {
            try explorer.setSettings(settings: settings)
        } catch {
            NSLog("Almondbread: settings rejected: \(error)")
            pull(explorer.settings())
        }
        host?.requestFrames()
        save()
    }

    private func pushOverlay(_ origin: StaticString = #function) {
        guard !pulling, let explorer else { return }
        HangWatchdog.note("settings", "pushOverlay from \(origin)")
        explorer.setOverlay(settings: overlaySettings)
        host?.requestFrames()
        save()
    }

    private func pushPiMeter(_ origin: StaticString = #function) {
        guard !pulling, let explorer else { return }
        HangWatchdog.note("settings", "pushPiMeter from \(origin)")
        explorer.setPiMeter(on: piMeterEnabled, insetRight: piMeterInsetRight, insetBottom: piMeterInsetBottom,
                            insetTop: piMeterInsetTop)
        if !piMeterEnabled { piMeter = nil }
        host?.requestFrames()
        save()
    }

    private func pushCosmic(_ origin: StaticString = #function) {
        guard !pulling, let explorer else { return }
        HangWatchdog.note("settings", "pushCosmic from \(origin)")
        explorer.setCosmicScale(settings: CosmicScaleSettings(enabled: cosmicScale,
                                                              metresPerPoint: metresPerPoint))
        explorer.setCosmicInset(right: cosmicInsetRight)
        host?.requestFrames()
        save()
    }

    private func pushTourCard() {
        guard let explorer else { return }
        if let f = tourCardFrame {
            explorer.setTourCard(x: f.minX - canvasOrigin.x, y: f.minY - canvasOrigin.y, width: f.width, height: f.height)
        } else {
            explorer.setTourCard(x: 0, y: 0, width: 0, height: 0)
        }
        host?.requestFrames()
    }

    private func pushCockpit() {
        guard let explorer else { return }
        explorer.setCockpit(on: showHUD, insetRight: cockpitInsetRight, insetBottom: 0)
        host?.requestFrames()
    }

    private func pushCompass(_ origin: StaticString = #function) {
        guard !pulling, let explorer else { return }
        HangWatchdog.note("settings", "pushCompass from \(origin)")
        explorer.setCompass(always: compassAlways, insetTop: compassInsetTop)
        explorer.setTargetMarker(on: targetMarker)
        host?.requestFrames()
        save()
    }

    // MARK: Persistence

    /// Writes the settings to user defaults (coalesced: at most every half second).
    private func save() {
        guard !pulling, explorer != nil, !TestLaunch.ephemeral else { return }
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
            mapLevels: mapLevels, mapSize: mapSize, mapPlacement: mapPlacement, juliaEnabled: juliaEnabled,
            cosmicScale: cosmicScale, compassAlways: compassAlways, targetMarker: targetMarker,
            formula: formula)
    }

    private func writeSaved() {
        // During a tour the user's own settings persist, not the tour's.
        if let data = try? JSONEncoder().encode(tourSaved ?? snapshot()) {
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
        formula = s.formula ?? formula
        orbitEnabled = s.orbitEnabled ?? orbitEnabled
        orbitLength = s.orbitLength ?? orbitLength
        mapEnabled = s.mapEnabled ?? mapEnabled
        mapLevels = s.mapLevels ?? mapLevels
        mapSize = s.mapSize ?? mapSize
        mapPlacement = s.mapPlacement ?? mapPlacement
        juliaEnabled = s.juliaEnabled ?? juliaEnabled
        cosmicScale = s.cosmicScale ?? cosmicScale
        compassAlways = s.compassAlways ?? compassAlways
        targetMarker = s.targetMarker ?? targetMarker
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
            juliaEnabled = false
            piMeterEnabled = false
            cosmicScale = false
            compassAlways = false
            targetMarker = true
        }
        pulling = false
        if scope == .all || scope == .overlays { showHUD = true }
        if scope == .all || scope == .view {
            edgePanning = .whileZooming
            autopilotLock = true
            laneAssist = false
            scrollZooms = false
        }
        if scope == .all || scope == .cinematic { journeyStops = false }
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
        juliaEnabled = false
        piMeterEnabled = false
        cosmicScale = false
        compassAlways = false
        targetMarker = true
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

    /// The pedals (E in, Q out; kb: decisions/controls "Driving levels"): one rung each.
    public func pedalIn() {
        send(.keyDown(key: .fasterZoom))
        send(.keyUp(key: .fasterZoom))
    }

    public func pedalOut() {
        send(.keyDown(key: .slowerZoom))
        send(.keyUp(key: .slowerZoom))
    }

    /// L: lane assist on / off.
    public func toggleLaneAssist() { laneAssist.toggle() }

    /// At a journey stop: Z (deeper here) and X (somewhere else); J is `toggleJourney`.
    public func journeyDeeper() {
        send(.keyDown(key: .autopilot))
        send(.keyUp(key: .autopilot))
    }

    public func journeyElsewhere() {
        send(.keyDown(key: .elsewhere))
        send(.keyUp(key: .elsewhere))
    }

    // MARK: Journey and rotation (kb: decisions/controls, "Journey", "View rotation")

    /// J: a journey with a random seed; while one runs, stops it; after manual input, resumes.
    public func toggleJourney() {
        send(.keyDown(key: .journey))
        send(.keyUp(key: .journey))
    }

    /// Seek minibrot (N; kb: decisions/controls "Seek minibrot"): the lowest-period minibrot in
    /// or near the view, or the next smaller one from a minibrot's framing; flies there.
    public func seekMinibrot() {
        send(.keyDown(key: .seekMinibrot))
        send(.keyUp(key: .seekMinibrot))
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
    /// A click on a cockpit icon (they replace the toolbar's autopilot and journey buttons):
    /// manual stops everything (Esc), cruise starts cruising in (E) unless already cruising,
    /// autopilot and journey toggle like Z and J.
    /// The tooltip of a cockpit control (`cockpitHit`'s id).
    static func cockpitTooltip(_ id: String) -> String? {
        switch id {
        case "manual": "Manual: you steer and zoom yourself. Click (or Esc) stops all motion"
        case "cruise": "Cruise (E / Q): zoom in or out in one of 26 gears, you steer"
        case "autopilot": "Autopilot (Z): zooms on and steers to interesting places; the mouse suggests a direction"
        case "journey": "Journey (J): flies from landmark to landmark"
        case "lane": "Lane assist (L): in cruise, nudges you away from featureless areas"
        default: nil
        }
    }

    func cockpitClick(_ level: String) {
        switch level {
        case "manual": stopMotion()
        case "cruise":
            if stats?.cruise.level != .cruise {
                send(.keyDown(key: .fasterZoom))
                send(.keyUp(key: .fasterZoom))
            }
        case "autopilot": toggleAutopilot()
        case "journey": toggleJourney()
        case "lane": toggleLaneAssist()
        default: break
        }
        host?.requestFrames()
    }

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

    // MARK: Guided tours (kb: decisions/controls "Guided tours"; Rust plays them)

    /// Every bundled tour, the hidden walkthrough included, as it plays here (steps, length).
    public static let allTours: [TourInfo] = AlmondbreadCore.toursOn(platform: TourPlatform.current)
    /// The tours the lists show (not the hidden ones, like the walkthrough).
    public static let tours: [TourInfo] = allTours.filter { !$0.hidden }
    /// The first-launch walkthrough (`tours/welcome.md`; Help and the Tours section offer it).
    public static let walkthroughID = "welcome"
    public static var walkthrough: TourInfo? { allTours.first { $0.id == walkthroughID } }

    /// A bundled tour by id (nil: not bundled, e.g. a series' tour not written yet).
    static func tourInfo(_ id: String) -> TourInfo? { allTours.first { $0.id == id } }
    /// The listed tour that covers `topic` (its `topics:` header; kb: decisions/controls
    /// "Finding the tours"): a contextual link starts it. Nil: none bundled yet, no link.
    static func tour(forTopic topic: String) -> TourInfo? { tours.first { $0.topics.contains(topic) } }
    /// What the last step of tour `id` offers next ("Next tour"; Rust resolves `next:` and the
    /// list order, missing and hidden ones skipped).
    static func nextTour(after id: String) -> TourInfo? {
        guard let next = tourInfo(id)?.next, !next.isEmpty else { return nil }
        return tours.first { $0.id == next }
    }

    static let toursDoneKey = "tours.done"
    /// The tours that reached their last step once (a checkmark in the lists). In user defaults,
    /// except in tests and demos (`TestLaunch.ephemeral`), which start empty.
    public private(set) var toursDone: Set<String> = TestLaunch.ephemeral ? []
        : Set(UserDefaults.standard.stringArray(forKey: ExplorerModel.toursDoneKey) ?? [])

    func markTourDone(_ id: String) {
        guard !toursDone.contains(id) else { return }
        toursDone.insert(id)
        if !TestLaunch.ephemeral { UserDefaults.standard.set(toursDone.sorted(), forKey: Self.toursDoneKey) }
    }

    /// The running step's goal (`goal:`) is done: the card says so. Polled with the stats.
    public private(set) var tourGoalMet = false
    /// Where the UI element the running step rings is (global coordinates; the tour card keeps
    /// clear of it). Set by the ringed element itself (`tourTarget`).
    var tourHighlightFrame: CGRect?
    /// Where the cockpit is while a tour runs (global; the card keeps clear of it too).
    var tourCockpitFrame: CGRect?
    /// Where the mini-map row is while a tour runs (global; the card keeps clear of it too).
    var tourMapsFrame: CGRect?

    /// The running tour's step (nil: no tour); set only by the tour calls below.
    public private(set) var tour: TourStepInfo?
    /// The settings before the tour (what persists meanwhile; the look id is ours alone).
    @ObservationIgnored private var tourSaved: SavedSettings?
    /// Esc in a tour (kb: decisions/controls "Guided tours"): one press stops motion like any Esc
    /// and arms this for 2 s; the card then says "Press Esc again to end the tour".
    public private(set) var tourEscapeArmed = false
    @ObservationIgnored private var tourEscapeDisarm: DispatchWorkItem?

    /// Starts a tour at its first step: other motion stops, the settings and view are saved.
    public func startTour(_ id: String) {
        guard let explorer else { return }
        let before = tourSaved ?? snapshot()
        guard let info = try? explorer.startTourOn(id: id, platform: TourPlatform.current) else { return }
        tourSaved = before
        takeTour(info)
    }

    /// Esc in a tour: the first press arms, a second within 2 s ends the tour. Returns whether
    /// it ended (the caller stops motion only when it did not: ending flies the view back).
    @discardableResult public func escapeTour() -> Bool {
        guard tour != nil else { return false }
        if tourEscapeArmed {
            endTour()
            return true
        }
        tourEscapeArmed = true
        let disarm = DispatchWorkItem { [weak self] in self?.tourEscapeArmed = false }
        tourEscapeDisarm?.cancel()
        tourEscapeDisarm = disarm
        DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: disarm)
        return false
    }

    private func disarmTourEscape() {
        tourEscapeDisarm?.cancel()
        tourEscapeDisarm = nil
        if tourEscapeArmed { tourEscapeArmed = false }
    }

    /// "Next ›" (→): the next step; on the last one the tour ends.
    public func tourNext() {
        guard let t = tour else { return }
        if Int(t.index) + 1 >= Int(t.count) { endTour() } else { tourGo(Int(t.index) + 1) }
    }

    /// "‹ Back" (←).
    public func tourBack() {
        guard let t = tour, t.index > 0 else { return }
        tourGo(Int(t.index) - 1)
    }

    public func tourGo(_ index: Int) {
        disarmTourEscape()
        guard let explorer, let info = explorer.tourGo(index: UInt32(max(0, index))) else { return }
        takeTour(info)
    }

    /// "End tour" (Esc twice): the settings before the tour come back and the view flies back.
    public func endTour() {
        disarmTourEscape()
        guard let explorer else { return }
        let saved = tourSaved
        tourSaved = nil
        if let info = explorer.endTour() { takeTour(info) }
        tour = nil
        if let look = saved?.lookID { lookID = look }
        save()
    }

    /// Mirrors what Rust applied for a step (or restored at the end), without pushing it back.
    private func takeTour(_ info: TourStepInfo) {
        pull(info.settings)
        pulling = true
        orbitEnabled = info.orbit
        orbitLength = Int(info.orbitLength)
        mapEnabled = info.map
        mapLevels = Int(info.mapLevels)
        juliaEnabled = info.julia
        cosmicScale = info.cosmic
        piMeterEnabled = info.piMeter
        compassAlways = info.compass
        if !info.look.isEmpty { lookID = info.look }
        pulling = false
        if !piMeterEnabled { piMeter = nil }
        let next = info.active ? info : nil
        if tour?.index != next?.index || tour?.tourId != next?.tourId {
            tourHighlightFrame = nil
            if tourGoalMet { tourGoalMet = false }
        }
        if tour != next { tour = next }
        // Reaching the last step counts as done (Finish or End tour there are the same).
        if let t = next, t.count > 0, t.index + 1 >= t.count { markTourDone(t.tourId) }
        host?.requestFrames()
    }

    public func toggleOrbit() { orbitEnabled.toggle() }
    public func toggleMap() { mapEnabled.toggle() }
    public func toggleJulia() { juliaEnabled.toggle() }
    public func toggleCosmicScale() { cosmicScale.toggle() }

    /// A click on the canvas at (x, y) points from the centre, y up: true if a mini-map panel
    /// took it (a jump follows).
    func mapClick(x: Double, y: Double, fly: Bool) -> Bool {
        guard let explorer, mapEnabled || orbitEnabled || juliaEnabled else { return false }
        let taken = explorer.mapClick(x: x, y: y, fly: fly)
        if taken { host?.requestFrames() }
        return taken
    }

    /// A press on the canvas at (x, y) points from the centre, y up, in a canvas of `size`
    /// points: true if it hit a help badge Rust drew (the press then goes nowhere else);
    /// `show`: open its popover (a first click or a tap; not the second click of a double click).
    func helpPress(x: Double, y: Double, size: CGSize, show: Bool = true) -> Bool {
        guard let explorer, let hit = explorer.helpHit(x: x, y: y) else { return false }
        if show, let topic = HelpTopic(rawValue: hit.topic) {
            showCanvasHelp(topic, x: hit.x, y: hit.y, radius: hit.radius, size: size)
        }
        return true
    }

    /// Opens the popover of `topic` at a badge at (x, y) points from the centre, y up.
    func showCanvasHelp(_ topic: HelpTopic, x: Double, y: Double, radius: Double, size: CGSize) {
        let rect = CGRect(x: size.width / 2 + x - radius, y: size.height / 2 - y - radius,
                          width: 2 * radius, height: 2 * radius)
        canvasHelp = CanvasHelp(topic: topic, rect: rect, edge: y < 0 ? .top : .bottom)
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
        pasteCoordinates(text: Clipboard.text())
    }

    /// Jumps to coordinates in `text` (the system paste control hands the clipboard's text over
    /// without the "Allow Paste?" prompt on iOS).
    @discardableResult
    public func pasteCoordinates(text: String?) -> Bool {
        guard let text, let explorer, explorer.pasteCoordinates(text: text) else { return false }
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
        if let t = tour, !t.goal.isEmpty, !tourGoalMet, explorer?.tourGoalMet() == true { tourGoalMet = true }
        if orbitEnabled || mapEnabled, let o = explorer?.orbitInfo(), o != orbit { orbit = o }
        if piMeterEnabled, let m = explorer?.piMeter(), m != piMeter { piMeter = m }
        guard let new = explorer?.stats(), new != stats else { return }
        #if os(iOS)
        Haptics.stats(old: stats, new: new)
        #endif
        if let old = stats, old.zoomSpeed != new.zoomSpeed || old.autopilot.speed != new.autopilot.speed
            || old.flight.speed != new.flight.speed || old.journey.speed != new.journey.speed
            || old.cruise.set != new.cruise.set {
            zoomSpeedChanged = Date()
        }
        stats = new
        if zoomSpeed != new.zoomSpeed { zoomSpeed = new.zoomSpeed }
        if autopilotActive != new.autopilot.active { autopilotActive = new.autopilot.active }
        if journeyActive != new.journey.active { journeyActive = new.journey.active }
        if flightActive != new.flight.active { flightActive = new.flight.active }
        if journeySuspended != new.journey.suspended { journeySuspended = new.journey.suspended }
        if journeySeed != new.journey.seed { journeySeed = new.journey.seed }
        if journeyAtStop != new.journey.atStop { journeyAtStop = new.journey.atStop }
        if drivingLevel != new.cruise.level { drivingLevel = new.cruise.level }
        if cruiseGear != Int(new.cruise.gear) { cruiseGear = Int(new.cruise.gear) }
        let turned = new.shown.angleDegrees != 0
        if viewTurned != turned { viewTurned = turned }
        if progressText != new.progress.text { progressText = new.progress.text }
        if canContinue != new.progress.canContinue { canContinue = new.progress.canContinue }
        let unfinished = !new.progress.visible && !new.progress.text.isEmpty
        if progressUnfinished != unfinished { progressUnfinished = unfinished }
    }

    /// "Continue" (HUD button, G): another time budget for compute further on this view.
    public func continueComputing() {
        guard canContinue, let explorer else { return }
        if explorer.continueComputing() { host?.requestFrames() }
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
