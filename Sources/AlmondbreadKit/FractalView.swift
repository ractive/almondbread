import AlmondbreadCore
import QuartzCore
import SwiftUI

// The platform views host the Rust renderer (kb: platform/render-architecture, "Hosting in
// the app"): each owns a CAMetalLayer whose pointer goes to Rust once, runs a display link
// that makes one `Explorer.frame` call per refresh and pauses while nothing animates, and
// decodes platform events into `InputEvent`s. Motion policy (pan speed, zoom target, zoom
// speed, idle timing, pointer/keyboard mode, the crosshair's glide, edge panning) is Rust's
// `Navigator`; nothing here integrates or interprets motion. The views only present what each
// `frame` result says: the cursor (system crosshair in pointer mode, hidden in keyboard
// mode) and the keyboard crosshair overlay (kb: decisions/controls, revision 2026-10-14).

/// The keyboard-mode crosshair: a Core Animation overlay above the Metal layer, moved to the
/// position Rust returns from each `frame` call. Not drawn into the Metal frame: the glide
/// then costs no GPU work or re-presentation of an idle image, the shown-frame checks stay
/// pure, and the layer is vector-sharp at any backing scale. It is composited with the same
/// display commit as the frame call that positioned it (the image can trail it by the frame
/// in flight, invisible for a marker the image moves under). Thin white arms with a centre
/// dot over a dark halo with a soft shadow: visible on any palette.
final class CrosshairLayer: CALayer {
    private let halo = CAShapeLayer()
    private let line = CAShapeLayer()
    private var shownAt: CGPoint?

    override init() {
        super.init()
        let size: CGFloat = 36
        bounds = CGRect(x: 0, y: 0, width: size, height: size)
        let c = size / 2, gap: CGFloat = 4.5, arm: CGFloat = 13.5
        let path = CGMutablePath()
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] as [(CGFloat, CGFloat)] {
            path.move(to: CGPoint(x: c + dx * gap, y: c + dy * gap))
            path.addLine(to: CGPoint(x: c + dx * arm, y: c + dy * arm))
        }
        let dot = CGPath(ellipseIn: CGRect(x: c - 0.9, y: c - 0.9, width: 1.8, height: 1.8), transform: nil)
        path.addPath(dot)
        for l in [halo, line] {
            l.frame = bounds
            l.path = path
            l.fillColor = nil
            l.lineCap = .round
            addSublayer(l)
        }
        halo.strokeColor = CGColor(gray: 0, alpha: 0.5)
        halo.lineWidth = 3.2
        halo.shadowColor = CGColor(gray: 0, alpha: 1)
        halo.shadowOpacity = 0.35
        halo.shadowRadius = 2.5
        halo.shadowOffset = .zero
        halo.shadowPath = path.copy(strokingWithWidth: 3.2, lineCap: .round, lineJoin: .round, miterLimit: 1)
        line.strokeColor = CGColor(gray: 1, alpha: 0.95)
        line.fillColor = CGColor(gray: 1, alpha: 0.95)
        line.lineWidth = 1.25
        halo.fillColor = CGColor(gray: 0, alpha: 0.5)
        isHidden = true
        zPosition = 1
    }

    override init(layer: Any) { super.init(layer: layer) }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    /// Shows the crosshair at (x, y) view points from the centre (y up) of `container`, or
    /// hides it. Snapped to the pixel grid so the 1.25 pt lines stay crisp; no implicit
    /// animation (the glide is Rust's).
    func update(visible: Bool, x: Double, y: Double, in container: CALayer, scale: CGFloat) {
        var p: CGPoint?
        if visible {
            let b = container.bounds
            #if os(macOS)
            let raw = CGPoint(x: b.midX + x, y: b.midY + y)
            #else
            let raw = CGPoint(x: b.midX + x, y: b.midY - y)
            #endif
            let s = max(scale, 1)
            p = CGPoint(x: (raw.x * s).rounded() / s, y: (raw.y * s).rounded() / s)
        }
        guard p != shownAt else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        if contentsScale != scale {
            for l in [self, halo, line] { l.contentsScale = scale }
        }
        if let p { position = p }
        isHidden = p == nil
        CATransaction.commit()
        shownAt = p
    }
}

/// Bits of `Explorer.frame`'s result (crates/almondbread-ffi explorer.rs `FRAME_*`; UniFFI
/// exports no constants; core-check verifies them).
let frameAnimating: UInt8 = 1
let frameKeyboard: UInt8 = 2
let frameFingerSteers: UInt8 = 4

/// Display links retain their target; this one holds the view weakly so views can go away.
@MainActor
final class DisplayLinkProxy: NSObject {
    weak var target: FractalHostView?
    init(_ target: FractalHostView) { self.target = target }
    @objc func step(_ link: CADisplayLink) { target?.step(link) }
}

/// The part shared by the macOS and iOS views.
@MainActor
class FractalHostBase {
    let model: ExplorerModel
    let metalLayer = CAMetalLayer()
    private(set) var explorer: Explorer?
    private var lastStats: CFTimeInterval = 0
    private var selfTestPending = SelfTest.enabled
    /// Duration of each `frame` call as Swift sees it (interaction test: FFI overhead).
    var frameTimes: [Double]?
    let crosshair = CrosshairLayer()
    /// The input mode as of the last frame or input.
    private(set) var mode: InputMode = .pointer
    /// The platform view's cursor presentation for a mode.
    var onModeChange: ((InputMode) -> Void)?
    /// The overlay state after the last `frame` call (interaction test: crosshair positions).
    struct Overlay { let keyboard: Bool; let x: Double; let y: Double }
    private(set) var lastFrame: Overlay?
    /// A finger steers rather than pans (A, C, a journey's flight; as of the last frame; iOS).
    private(set) var fingerSteers = false

    init(model: ExplorerModel, scale: CGFloat) {
        self.model = model
        #if os(macOS)
        // The kernel writes sRGB-encoded values; tag the layer so they are colour-matched to
        // wide-gamut (P3) displays instead of being shown as raw display values.
        metalLayer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        #endif
        metalLayer.isOpaque = true
        metalLayer.addSublayer(crosshair)
        SelfTest.start()
        do {
            let address = UInt64(UInt(bitPattern: Unmanaged.passUnretained(metalLayer).toOpaque()))
            explorer = try Explorer(layer: address, scale: Double(scale))
        } catch {
            NSLog("Almondbread: renderer could not be created: \(error)")
            model.failure = "\(error)"
            if SelfTest.enabled { SelfTest.finish(ok: false, "renderer FAILED: \(error)") }
        }
    }

    func attach(host: ExplorerHost) {
        if let explorer { model.attach(explorer, host: host) }
    }

    @discardableResult
    func send(_ event: InputEvent, host: ExplorerHost) -> InputResult {
        guard let explorer else { return InputResult(handled: false, redraw: false, mode: mode) }
        HangWatchdog.note("input", "\(event)")
        let r = explorer.input(event: event)
        if r.redraw { host.requestFrames() }
        setMode(r.mode)
        return r
    }

    private func setMode(_ m: InputMode) {
        guard m != mode else { return }
        mode = m
        onModeChange?(m)
    }

    func resize(width: CGFloat, height: CGFloat, scale: CGFloat, host: ExplorerHost) {
        explorer?.resize(width: Double(width), height: Double(height), scale: Double(scale))
        // Cached for the place pills: a view body never calls into Rust (kb:
        // platform/swiftui-performance).
        model.placeZoomOffset = explorer?.placeZoomOffset() ?? 0
        host.requestFrames()
    }

    /// One refresh; returns false when the display link should pause.
    func step(timestamp: CFTimeInterval) -> Bool {
        guard let explorer else { return false }
        let t0 = CACurrentMediaTime()
        HangWatchdog.frame(begin: true)
        let bits = explorer.frame(timestamp: timestamp)
        HangWatchdog.frame(begin: false)
        let t1 = CACurrentMediaTime()
        frameTimes?.append(t1 - t0)
        let animating = bits & frameAnimating != 0
        let keyboard = bits & frameKeyboard != 0
        fingerSteers = bits & frameFingerSteers != 0
        // Two scalar calls, only while the crosshair shows.
        let o = Overlay(keyboard: keyboard, x: keyboard ? explorer.crosshairX() : 0, y: keyboard ? explorer.crosshairY() : 0)
        lastFrame = o
        crosshair.update(visible: keyboard, x: o.x, y: o.y, in: metalLayer, scale: metalLayer.contentsScale)
        setMode(keyboard ? .keyboard : .pointer)
        if !animating || t1 - lastStats > 0.1 {
            lastStats = t1
            model.refreshStats()
        }
        if selfTestPending, let s = model.stats, s.settled {
            selfTestPending = false
            SelfTest.report(explorer: explorer, stats: s, viewWidth: Double(metalLayer.bounds.width))
        }
        return animating
    }
}

/// What the display link proxy calls.
@MainActor
protocol FractalHostView: AnyObject {
    func step(_ link: CADisplayLink)
}

#if os(macOS)
import AppKit

final class FractalNSView: NSView, ExplorerHost, FractalHostView {
    let base: FractalHostBase
    var explorer: Explorer? { base.explorer }
    var model: ExplorerModel { base.model }
    private var link: CADisplayLink?
    private var keyMonitor: Any?
    private var resignObserver: NSObjectProtocol?
    private var fullScreenObservers: [NSObjectProtocol] = []
    private var layoutRectObservation: NSKeyValueObservation?
    private var tracking: NSTrackingArea?
    private var sentInsets: NSEdgeInsets?
    /// The pointer is over the HUD (the canvas's arrow-cursor hole): reported as exited.
    private var overHUD = false
    /// A mouse drag is in progress (its release may fling).
    private var dragging = false
    /// The current mouse press landed on a mini-map (no drag, no double-click zoom).
    private var mapClicked = false
    /// The current mouse press landed on the compass: its drag and release go there (Rust
    /// hit-tests, turns and eases; kb: decisions/controls "Compass").
    private var compassPressed = false
    /// The current press is on a help badge (its popover opened on the press): its drag and
    /// release go nowhere.
    private var helpPressed = false
    /// The pointer is over a help badge (the cursor shows a pointing hand).
    private var overBadge = false
    private var gamepad: GamepadInput?

    init(model: ExplorerModel) {
        base = FractalHostBase(model: model, scale: NSScreen.main?.backingScaleFactor ?? 2)
        super.init(frame: .zero)
        wantsLayer = true
        layerContentsRedrawPolicy = .never
        base.attach(host: self)
        base.onModeChange = { [weak self] mode in self?.modeChanged(mode) }
        if InteractionTest.enabled { InteractionTest.shared.attach(view: self) }
        if FlightBench.enabled { FlightBench.shared.attach(view: self) }
        if let explorer = base.explorer, UIDemo.enabled { UIDemo.configure(model, explorer: explorer) }
        // Window tests never read the user's controller.
        if !TestLaunch.enabled && Preferences.gamepadEnabled { gamepad = GamepadInput { [weak self] e in self?.send(e) } }
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func makeBackingLayer() -> CALayer { base.metalLayer }
    override var isOpaque: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    // ⌘C / ⌘V on the canvas (Edit > Copy / Paste reach the focused view): copy the view's
    // coordinates, go to copied ones. Text fields keep ordinary copy and paste.
    @objc func copy(_ sender: Any?) {
        Clipboard.copy(model.coordinatesText)
    }

    @objc func paste(_ sender: Any?) {
        if !model.pasteCoordinates() { NSSound.beep() }
    }

    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(copy(_:)): true
        case #selector(paste(_:)): Clipboard.text() != nil
        default: true
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        link?.invalidate()
        link = nil
        guard let window else {
            send(.focusLost)
            return
        }
        window.makeFirstResponder(self)
        window.acceptsMouseMovedEvents = true
        TestLaunch.placeInBackground(window)
        installKeyHandling()
        let l = displayLink(target: DisplayLinkProxy(self), selector: #selector(DisplayLinkProxy.step(_:)))
        // Ask for the panel's full rate: without a preferred range a ProMotion Mac may drive
        // the link at 60 Hz (measured FL: 179 calls in 3 s on a 120 Hz panel).
        let fps = Float(window.screen?.maximumFramesPerSecond ?? 60)
        l.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, fps), maximum: fps, preferred: fps)
        l.add(to: .main, forMode: .common)
        link = l
        layoutExplorer()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        layoutExplorer()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        layoutExplorer()
    }

    override func layout() {
        super.layout()
        sendInsets()
    }

    private func layoutExplorer() {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        base.resize(width: bounds.width, height: bounds.height, scale: scale, host: self)
        base.model.metresPerPoint = ScreenMetrics.metresPerPoint(window?.screen)
        sendInsets()
    }

    /// The title bar / toolbar overlaps the canvas (it extends under them): Rust measures the
    /// pointer's canvas and the edge band from the uncovered part.
    private func sendInsets() {
        let i = safeAreaInsets
        if let s = sentInsets, s.top == i.top, s.left == i.left, s.bottom == i.bottom, s.right == i.right { return }
        sentInsets = i
        send(.insets(top: Double(i.top), left: Double(i.left), bottom: Double(i.bottom), right: Double(i.right)))
        window?.invalidateCursorRects(for: self)
    }

    // MARK: Cursor

    /// The canvas part of the view: inside the safe area, minus the HUD.
    private var hudRect: NSRect {
        guard let window, let content = window.contentView, let g = model.hudFrame, !g.isEmpty else { return .zero }
        // SwiftUI's global space is the content view's, top-left origin.
        let r = content.isFlipped ? g : NSRect(x: g.minX, y: content.bounds.height - g.maxY, width: g.width, height: g.height)
        return convert(r, from: content)
    }

    func hudFrameChanged() {
        window?.invalidateCursorRects(for: self)
    }

    /// Pointer mode: the crosshair cursor over the canvas only (cursor rects: the arrow
    /// stays over the title bar, the HUD and the inspector).
    override func resetCursorRects() {
        let i = safeAreaInsets
        let canvas = NSRect(x: bounds.minX + i.left, y: bounds.minY + i.bottom,
                            width: bounds.width - i.left - i.right, height: bounds.height - i.top - i.bottom)
            .intersection(visibleRect)
        for r in Self.subtract(hudRect, from: canvas) where !r.isEmpty {
            addCursorRect(r, cursor: .crosshair)
        }
    }

    /// `rect` minus `hole`, as up to four rectangles.
    static func subtract(_ hole: NSRect, from rect: NSRect) -> [NSRect] {
        let h = hole.intersection(rect)
        guard !h.isEmpty else { return [rect] }
        return [NSRect(x: rect.minX, y: rect.minY, width: rect.width, height: h.minY - rect.minY),
                NSRect(x: rect.minX, y: h.maxY, width: rect.width, height: rect.maxY - h.maxY),
                NSRect(x: rect.minX, y: h.minY, width: h.minX - rect.minX, height: h.height),
                NSRect(x: h.maxX, y: h.minY, width: rect.maxX - h.maxX, height: h.height)]
    }

    /// Keyboard mode hides the cursor until the mouse moves (the overlay crosshair takes
    /// over); pointer mode shows it again through the cursor rects. Window tests leave the
    /// user's cursor alone.
    private func modeChanged(_ mode: InputMode) {
        guard !TestLaunch.enabled else { return }
        if mode == .keyboard { NSCursor.setHiddenUntilMouseMoves(true) }
    }

    func requestFrames() { link?.isPaused = false }

    /// The window moved to another screen (interaction test): re-request its full rate.
    func setPreferredFrameRate(_ fps: Float) {
        link?.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, fps), maximum: fps, preferred: fps)
    }

    func step(_ link: CADisplayLink) {
        if externalClock { return }
        if !base.step(timestamp: link.timestamp) { link.isPaused = true }
        hidePointerWhileFlying()
    }

    /// When the pointer last moved over the canvas (`mouseMoved`).
    private var pointerMovedAt = Date()

    /// While the autopilot or a journey flies, the pointer hides after `pointerHideDelay` s
    /// without moving and comes back as soon as the mouse moves (user's request; the
    /// pointer is a direction wish in autopilot, so it shows whenever it is being used).
    private func hidePointerWhileFlying() {
        let level = model.drivingLevel
        guard level == .autopilot || level == .journey, !TestLaunch.enabled,
              window?.isKeyWindow == true,
              Date().timeIntervalSince(pointerMovedAt) > Self.pointerHideDelay else { return }
        // Only over the canvas (not the HUD, the control center or outside the window).
        guard let w = window else { return }
        let p = convert(w.mouseLocationOutsideOfEventStream, from: nil)
        guard bounds.contains(p), !hudRect.contains(p) else { return }
        NSCursor.setHiddenUntilMouseMoves(true)
        pointerMovedAt = .distantFuture
    }
    private static let pointerHideDelay = 1.5

    /// The interaction test drives frames from its own CVDisplayLink (`ALMONDBREAD_TEST_LINK=cv`).
    var externalClock = false

    func stepExternal(timestamp: CFTimeInterval) {
        _ = base.step(timestamp: timestamp)
    }

    @discardableResult
    func send(_ event: InputEvent) -> InputResult { base.send(event, host: self) }

    /// Offset of a window location from the view centre, in view points, y up.
    private func offset(_ locationInWindow: NSPoint) -> (x: Double, y: Double) {
        let p = convert(locationInWindow, from: nil)
        return (Double(p.x - bounds.midX), Double(p.y - bounds.midY))
    }

    // MARK: Keys

    /// Keys come from a window-local monitor rather than keyDown, so navigation works while
    /// the inspector (a slider, a picker) has focus; only text editing keeps its keys. Handled
    /// keys are swallowed, which also avoids the system beep.
    private func installKeyHandling() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        if let resignObserver { NotificationCenter.default.removeObserver(resignObserver) }
        keyMonitor = nil
        guard let window else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
            guard let self, event.window === self.window else { return event }
            return self.handleKey(event) ? nil : event
        }
        // Key-up never arrives once the window loses focus: stop all motion then.
        resignObserver = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
                                                                object: window, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { _ = self?.send(.focusLost) }
        }
        observeFullScreen(window)
    }

    // MARK: Full screen (kb: decisions/controls "Full screen")

    /// Window tests never enter full screen: they replace the window's state and its toggle
    /// with this probe (`--interaction-test` scenario "esc full screen").
    var fullScreenProbe: (isOn: () -> Bool, toggle: () -> Void)?

    var isFullScreen: Bool {
        fullScreenProbe?.isOn() ?? window?.styleMask.contains(.fullScreen) ?? false
    }

    /// F and an Esc with nothing moving (the standard ⌃⌘F is AppKit's View menu item).
    func toggleFullScreen() {
        if let fullScreenProbe { fullScreenProbe.toggle(); return }
        guard !TestLaunch.enabled else { return }
        window?.toggleFullScreen(nil)
    }

    /// The model hears of full screen as the transition starts (the control center closes
    /// before the animation, and comes back as the window leaves). The insets are sent again
    /// at the end of each transition and whenever the window's content layout rect changes
    /// (the title bar and toolbar hide in full screen and show when the pointer goes to the
    /// top edge), so the compass and mini-maps sit right below whatever covers the canvas.
    private func observeFullScreen(_ window: NSWindow) {
        for o in fullScreenObservers { NotificationCenter.default.removeObserver(o) }
        let nc = NotificationCenter.default
        // will*: the state the transition goes to; did*: the window's actual state (a failed
        // transition ends where it started).
        let changes: [(Notification.Name, Bool?)] = [
            (NSWindow.willEnterFullScreenNotification, true), (NSWindow.willExitFullScreenNotification, false),
            (NSWindow.didEnterFullScreenNotification, nil), (NSWindow.didExitFullScreenNotification, nil),
        ]
        fullScreenObservers = changes.map { name, on in
            nc.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    let state = on ?? self.window?.styleMask.contains(.fullScreen) ?? false
                    if self.model.fullScreen != state { self.model.fullScreen = state }
                    self.insetsMayHaveChanged()
                }
            }
        }
        // ⌃⌘F: make sure the View menu has AppKit's full-screen item (below).
        fullScreenObservers.append(nc.addObserver(forName: NSWindow.didBecomeKeyNotification, object: window,
                                                  queue: .main) { _ in
            MainActor.assumeIsolated { FullScreenMenuItem.ensure() }
        })
        DispatchQueue.main.async { FullScreenMenuItem.ensure() }
        // A background test window (accessory app) reports `fullScreenNone`; the app's window
        // is a primary full-screen window, said explicitly so F and ⌃⌘F always work. Window
        // tests never enter full screen, so they leave it alone.
        if !TestLaunch.enabled {
            window.collectionBehavior.remove(.fullScreenNone)
            window.collectionBehavior.insert(.fullScreenPrimary)
        }
        layoutRectObservation = window.observe(\.contentLayoutRect, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async { [weak self] in self?.insetsMayHaveChanged() }
        }
    }

    /// The standard full-screen item in the View menu. AppKit normally adds "Enter Full
    /// Screen" (⌃⌘F) itself, but only to a menu bar it shows and without telling us when; an
    /// item with the `toggleFullScreen:` action is what it looks for before adding its own, so
    /// this adds one only where none is (AppKit titles it Enter / Exit Full Screen when it
    /// validates it against the key window). SwiftUI may rebuild the menu: checked again
    /// whenever the window becomes key.
    enum FullScreenMenuItem {
        static func ensure() {
            guard let view = NSApplication.shared.mainMenu?.items.first(where: { $0.title == "View" })?.submenu,
                  !view.items.contains(where: { $0.action == #selector(NSWindow.toggleFullScreen(_:)) }) else { return }
            let item = NSMenuItem(title: "Enter Full Screen", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
            item.keyEquivalentModifierMask = [.control, .command]
            view.addItem(.separator())
            view.addItem(item)
        }
    }

    private func insetsMayHaveChanged() {
        needsLayout = true
        sendInsets()
    }

    /// The navigation meaning of a key (kb: decisions/controls), or nil.
    static func navKey(_ event: NSEvent) -> NavKey? {
        // Esc stops the autopilot or a flight (Rust leaves it unhandled otherwise).
        if event.keyCode == 53 { return .stop }
        switch event.specialKey {
        case .leftArrow?: return .left
        case .rightArrow?: return .right
        case .upArrow?: return .up
        case .downArrow?: return .down
        default: break
        }
        let shift = event.modifierFlags.contains(.shift)
        switch (event.charactersIgnoringModifiers ?? "").lowercased() {
        case "w": return .up
        case "s": return .down
        case "a": return .left
        case "d": return .right
        case " ": return .zoom
        case "q": return .slowerZoom
        case "e": return .fasterZoom
        case "+", "=": return .stepIn
        case "-", "_": return .stepOut
        case "r": return .reset
        // Q / E: the pedals (kb: decisions/controls "Driving levels"); ⇧J: a journey in the
        // other mode; X: somewhere else at a journey stop ("Journey with stops"); N: seek a
        // minibrot ("Seek minibrot").
        case "z": return .autopilot
        case "j": return shift ? .journeyAlt : .journey
        case "x": return .elsewhere
        case "n": return .seekMinibrot
        case ",", "<": return .rotateCcw
        case ".", ">": return .rotateCw
        case "/": return .straighten
        default: return nil
        }
    }

    /// Returns true if the event was consumed.
    func handleKey(_ event: NSEvent) -> Bool {
        let shift = event.modifierFlags.contains(.shift)
        if event.type == .flagsChanged {
            send(.shift(down: shift))
            return false
        }
        // Menu shortcuts and text fields keep their keys.
        if event.modifierFlags.contains(.command) { return false }
        if window?.firstResponder is NSText { return false }
        // A guided tour (kb: decisions/controls "Guided tours"): → next, ← back, Esc ends it.
        // Only the key-downs: a release still reaches Rust (harmless if nothing was held).
        if model.tour != nil, event.type == .keyDown, !event.modifierFlags.contains(.option) {
            let tourKey: (() -> Void)? = switch (event.keyCode, event.specialKey) {
            case (53, _): { self.model.endTour() }
            case (_, .rightArrow?): { self.model.tourNext() }
            case (_, .leftArrow?): { self.model.tourBack() }
            default: nil
            }
            if let tourKey {
                if !event.isARepeat { tourKey() }
                return true
            }
        }
        // O / M / C / P / L: orbit overlay, mini-map, cosmic size scale, π meter, lane assist (toggled on
        // key-down; their key-ups are swallowed).
        switch (event.charactersIgnoringModifiers ?? "").lowercased() {
        case "o":
            if event.type == .keyDown && !event.isARepeat { model.toggleOrbit() }
            return true
        case "m":
            if event.type == .keyDown && !event.isARepeat { model.toggleMap() }
            return true
        case "c":
            if event.type == .keyDown && !event.isARepeat { model.toggleCosmicScale() }
            return true
        case "p":
            if event.type == .keyDown && !event.isARepeat { model.togglePiMeter() }
            return true
        case "l":
            // Lane assist (kb: decisions/controls "Driving levels").
            if event.type == .keyDown && !event.isARepeat { model.toggleLaneAssist() }
            return true
        case "f":
            if event.type == .keyDown && !event.isARepeat { toggleFullScreen() }
            return true
        case "g":
            // Compute further at its time limit: another round (the HUD's "Continue").
            if event.type == .keyDown && !event.isARepeat && model.canContinue { model.continueComputing() }
            return true
        default: break
        }
        guard let key = Self.navKey(event) else { return false }
        // Esc in full screen: stops motion first; only an Esc that stopped nothing leaves full
        // screen. Every Esc is swallowed there, so AppKit never sees one, and a held Esc
        // (repeats) never leaves: the press that stopped the autopilot must not also exit.
        if key == .stop && isFullScreen {
            guard event.type == .keyDown else { return true }
            if event.isARepeat { return true }
            send(.shift(down: shift))
            if !send(.keyDown(key: .stop)).handled { toggleFullScreen() }
            return true
        }
        // Z toggles: a held key's repeats must not toggle again. Q / E: Rust steps a held pedal
        // itself (a repeat would count as a new press and cross the detent at zero).
        if [.autopilot, .journey, .journeyAlt, .elsewhere, .seekMinibrot, .straighten, .slowerZoom, .fasterZoom].contains(key)
            && event.isARepeat {
            return true
        }
        if event.type == .keyUp { return send(.keyUp(key: key)).handled }
        send(.shift(down: shift))
        return send(.keyDown(key: key)).handled
    }

    override func keyDown(with event: NSEvent) {
        // Keys arrive through the local monitor; swallow ours here too (no beep).
        if !handleKey(event) { super.keyDown(with: event) }
    }

    // MARK: Pointer

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                               owner: self, userInfo: nil)
        addTrackingArea(t)
        tracking = t
    }

    override func mouseMoved(with event: NSEvent) {
        pointerMovedAt = Date()
        // The interaction test drives the pointer itself; the user's real mouse over the
        // background test window must not interfere.
        guard !InteractionTest.enabled, !UIDemo.enabled, !FlightBench.enabled else { return }
        let p = convert(event.locationInWindow, from: nil)
        // Over the HUD the pointer is off the canvas (zooms go to the centre, no edge pan).
        if hudRect.contains(p) {
            if !overHUD { overHUD = true; send(.pointerExited) }
            return
        }
        overHUD = false
        let o = offset(event.locationInWindow)
        // A pointing hand over a help badge (the "?" Rust draws): it is a button.
        let control = model.explorer?.cockpitHit(x: o.x, y: o.y)
        let tip = control.flatMap(ExplorerModel.cockpitTooltip)
        if toolTip != tip { toolTip = tip }
        let onBadge = model.explorer?.helpHit(x: o.x, y: o.y) != nil || control != nil
        if onBadge != overBadge {
            overBadge = onBadge
            (onBadge ? NSCursor.pointingHand : NSCursor.crosshair).set()
        }
        let r = send(.pointer(x: o.x, y: o.y))
        // A nudge below the switch threshold keeps keyboard mode: keep the cursor hidden.
        if r.mode == .keyboard && !TestLaunch.enabled { NSCursor.setHiddenUntilMouseMoves(true) }
    }

    override func mouseEntered(with event: NSEvent) { mouseMoved(with: event) }
    override func mouseExited(with event: NSEvent) {
        guard !InteractionTest.enabled, !UIDemo.enabled, !FlightBench.enabled else { return }
        overHUD = false
        send(.pointerExited)
    }

    override func scrollWheel(with event: NSEvent) {
        let zoomModifier = event.modifierFlags.contains(.command) || event.modifierFlags.contains(.option)
        let o = offset(event.locationInWindow)
        if event.hasPreciseScrollingDeltas && !zoomModifier {
            // Trackpad: two-finger scroll pans 1:1; content follows the fingers. After the
            // fingers lift, the OS keeps sending momentum-phase scrolls: that is the fling.
            // (While the autopilot or a flight runs, Rust takes scrolling as its speed.)
            send(.scroll(dx: Double(event.scrollingDeltaX), dy: -Double(event.scrollingDeltaY),
                         momentum: !event.momentumPhase.isEmpty))
        } else if event.hasPreciseScrollingDeltas {
            // Precise scroll zoom (trackpad or Magic Mouse with a modifier): 1:1.
            send(.wheelZoom(delta: Double(event.scrollingDeltaY) / 40, x: o.x, y: o.y))
        } else {
            // A wheel notch: an animated step (Rust eases it in over ~0.1 s).
            send(.wheelNotch(delta: Double(event.scrollingDeltaY), x: o.x, y: o.y))
        }
    }

    override func rotate(with event: NSEvent) {
        // Trackpad rotation (degrees, counter-clockwise positive): the image follows the fingers.
        let o = offset(event.locationInWindow)
        send(.rotate(radians: Double(event.rotation) * .pi / 180, x: o.x, y: o.y))
    }

    override func magnify(with event: NSEvent) {
        let o = offset(event.locationInWindow)
        send(.magnify(magnification: Double(event.magnification), x: o.x, y: o.y))
        // Fingers lifted while zooming: a zoom fling (no OS momentum for magnify).
        if event.phase == .ended { send(.gestureEnd) }
    }

    override func mouseUp(with event: NSEvent) {
        if helpPressed {
            helpPressed = false
            return
        }
        if compassPressed {
            // A click (or the second click of a double click) eases the view back to north.
            compassPressed = false
            model.compassRelease()
            return
        }
        // A drag released while moving flings on (mice have no OS momentum).
        if dragging {
            send(.gestureEnd)
        } else if event.clickCount == 1 && !mapClicked {
            // A plain click pauses / resumes the autopilot (unused otherwise; a click on a
            // mini-map jumps instead).
            let o = offset(event.locationInWindow)
            send(.click(x: o.x, y: o.y))
        }
        dragging = false
        mapClicked = false
    }

    override func mouseDragged(with event: NSEvent) {
        if helpPressed { return }
        if compassPressed {
            let o = offset(event.locationInWindow)
            model.compassDrag(x: o.x, y: o.y)
            return
        }
        // A press on a mini-map jumps; it does not drag the view.
        if mapClicked { return }
        dragging = true
        send(.pan(dx: Double(event.deltaX), dy: -Double(event.deltaY)))
        // The pointer moves with the drag: it is the zoom target afterwards.
        if !InteractionTest.enabled {
            let o = offset(event.locationInWindow)
            send(.pointer(x: o.x, y: o.y))
        }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        // The compass takes single and double clicks (no double-click zoom under it).
        let p = offset(event.locationInWindow)
        // A help badge (the "?" Rust draws on a panel) opens its popover; its double click
        // does not zoom (kb: decisions/controls "Help").
        if model.helpPress(x: p.x, y: p.y, size: bounds.size, show: event.clickCount == 1) {
            helpPressed = true
            return
        }
        // A cockpit icon switches the driving level (its drag and release go nowhere).
        if let level = model.explorer?.cockpitHit(x: p.x, y: p.y) {
            if event.clickCount == 1 { model.cockpitClick(level) }
            helpPressed = true
            return
        }
        if model.compassPress(x: p.x, y: p.y) {
            compassPressed = true
            return
        }
        if event.clickCount == 1 {
            // A click on a mini-map panel jumps there (Option: flies there).
            let o = offset(event.locationInWindow)
            mapClicked = model.mapClick(x: o.x, y: o.y, fly: event.modifierFlags.contains(.option))
            if mapClicked { return }
        }
        if mapClicked { return }
        if event.clickCount == 2 {
            let o = offset(event.locationInWindow)
            send(.doubleTap(x: o.x, y: o.y, out: event.modifierFlags.contains(.option)))
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        if event.clickCount == 2 {
            let o = offset(event.locationInWindow)
            send(.doubleTap(x: o.x, y: o.y, out: true))
        }
    }
}

struct FractalCanvas: NSViewRepresentable {
    let model: ExplorerModel
    func makeNSView(context: Context) -> FractalNSView { FractalNSView(model: model) }
    func updateNSView(_ view: FractalNSView, context: Context) {}
}

#else
import UIKit

final class FractalUIView: UIView, ExplorerHost, FractalHostView {
    let base: FractalHostBase
    private var link: CADisplayLink?
    private var lastPinchScale: CGFloat = 1
    private var lastPinchCenter: CGPoint = .zero
    private var pointerInteraction: UIPointerInteraction?
    private var sentInsets: UIEdgeInsets?
    private var gamepad: GamepadInput?
    /// The current one-finger pan started on the compass: it turns the view.
    private var compassPanning = false
    /// Touch (kb: decisions/controls "Touch"): the one-finger pan steers (A, C) rather than
    /// pans; a long press is on (its drag moves the orbit probe, its release opens the menu),
    /// where it began and how far it moved; when the last long press ended (its tap is not a
    /// tap); the long-press menu.
    private var steering = false
    private var longPressing = false
    private var longPressStart: CGPoint = .zero
    private var longPressMoved: CGFloat = 0
    private var longPressEnded = Date.distantPast
    /// What the current long press does (Rust's `touch_hold` when it began).
    private var hold = TouchHold.nothing
    /// Memory, thermal and background signals for the renderer (DeviceSignals.swift).
    private var signals: DeviceSignals?

    // The base creates the layer before the view exists; UIKit asks the class for its layer,
    // so the view's own layer is a plain one hosting the Metal layer as a sublayer.
    init(model: ExplorerModel) {
        base = FractalHostBase(model: model, scale: UIScreen.main.scale)
        super.init(frame: .zero)
        layer.addSublayer(base.metalLayer)
        isMultipleTouchEnabled = true
        base.attach(host: self)
        signals = DeviceSignals(host: self)

        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(handlePinch(_:)))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
        pan.maximumNumberOfTouches = 1
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(handleDoubleTap(_:)))
        doubleTap.numberOfTapsRequired = 2
        let twoFingerTap = UITapGestureRecognizer(target: self, action: #selector(handleTwoFingerTap(_:)))
        twoFingerTap.numberOfTouchesRequired = 2
        // A single tap pauses / resumes the autopilot (it waits for a double tap to fail).
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap(_:)))
        tap.require(toFail: doubleTap)
        pinch.delegate = self
        pan.delegate = self
        // Two-finger rotation turns the view (kb: decisions/controls, "View rotation").
        let rotation = UIRotationGestureRecognizer(target: self, action: #selector(handleRotation(_:)))
        rotation.delegate = self
        // iPad trackpad / mouse: pointer mode (zoom target = pointer, crosshair pointer).
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(handleHover(_:)))
        // A long press: the menu at that point; its drag moves the orbit probe (touch has no hover).
        let longPress = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))
        longPress.minimumPressDuration = Self.menuHold
        longPress.delegate = self
        doubleTap.delegate = self
        tap.delegate = self
        [pinch, rotation, pan, doubleTap, twoFingerTap, tap, hover, longPress].forEach(addGestureRecognizer)
        let pointer = UIPointerInteraction(delegate: self)
        addInteraction(pointer)
        pointerInteraction = pointer
        base.onModeChange = { [weak self] _ in self?.pointerInteraction?.invalidate() }
        if Preferences.gamepadEnabled && !TestLaunch.enabled { gamepad = GamepadInput { [weak self] e in self?.send(e) } }
        if let explorer = base.explorer, UIDemo.enabled { UIDemo.configure(model, explorer: explorer) }
        // Touch-sized cockpit cells and help badges (kb: decisions/controls "Touch").
        base.explorer?.setCockpitTouch(on: true)
        // VoiceOver (and the UI tests): one element, touches pass through to the gestures,
        // its value is the view (`accessibilityValue`).
        isAccessibilityElement = true
        accessibilityIdentifier = "fractal"
        accessibilityLabel = "Fractal"
        accessibilityTraits = .allowsDirectInteraction
    }

    /// The view as text: "re im scale angle° autopilot on|off journey on|off level m|c|a|j gear
    /// n orbit on|off seek phase" (UI tests read it to check that a gesture moved, zoomed or
    /// turned the view, and what the throttle and the long-press menu did).
    override var accessibilityValue: String? {
        get {
            guard let explorer = base.explorer else { return nil }
            let v = explorer.viewport()
            let s = explorer.stats()
            let level = switch s.cruise.level {
            case .manual: "m"
            case .cruise: "c"
            case .autopilot: "a"
            case .journey: "j"
            }
            return "\(v.re) \(v.im) \(v.scaleText) \(String(format: "%.2f", v.angleDegrees))° "
                + "autopilot \(s.autopilot.active ? "on" : "off") journey \(s.journey.active ? "on" : "off") "
                + "level \(level) gear \(s.cruise.gear) orbit \(base.model.orbitEnabled ? "on" : "off") "
                + "seek \(s.seek.phase.isEmpty ? "idle" : s.seek.phase.replacingOccurrences(of: " ", with: "-")) assist \(s.cruise.assist ? "on" : "off")"
                + uiTestTargets(explorer)
        }
        set {}
    }

    /// UI tests only (launch environment `ALMONDBREAD_UITEST=1`): where Rust drew the cockpit's
    /// controls and the help badges, " cockpit id:x,y ..." in view points from the top-left, so a
    /// test taps them through the real hit tests (VoiceOver never hears this).
    private static var uiTest: Bool { TestLaunch.uiTest }
    private func uiTestTargets(_ explorer: Explorer) -> String {
        guard Self.uiTest else { return "" }
        let (w, h) = (Double(bounds.width), Double(bounds.height))
        let at = { (x: Double, y: Double) in "\(Int((w / 2 + x).rounded())),\(Int((h / 2 - y).rounded()))" }
        let cells = explorer.cockpitControls().map { "\($0.id):" + at($0.x, $0.y) }
        let badges = explorer.helpBadges().map { "help-\($0.topic):" + at($0.x, $0.y) }
        return " cockpit " + (cells + badges).joined(separator: " ")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        link?.invalidate()
        link = nil
        signals?.attach(scene: window?.windowScene)
        guard window != nil else {
            send(.focusLost)
            return
        }
        becomeFirstResponder()
        // The first-run hints go on the first touch anywhere in the window (TouchHints.swift).
        if let window, !(window.gestureRecognizers ?? []).contains(where: { $0 is TouchObserver }) {
            window.addGestureRecognizer(TouchObserver { TouchHints.shared.dismiss() })
        }
        let l = CADisplayLink(target: DisplayLinkProxy(self), selector: #selector(DisplayLinkProxy.step(_:)))
        link = l
        applyFrameRate()
        l.add(to: .main, forMode: .common)
        setNeedsLayout()
    }

    /// The display link's rate: the screen's maximum (120 Hz on ProMotion) under the thermal
    /// ceiling Rust returns.
    func applyFrameRate() {
        guard let link, let screen = window?.screen else { return }
        link.preferredFrameRateRange = signals?.frameRateRange(screenMax: screen.maximumFramesPerSecond)
            ?? CAFrameRateRange(minimum: 60, maximum: Float(screen.maximumFramesPerSecond),
                                preferred: Float(screen.maximumFramesPerSecond))
    }

    /// The scene went to the background (no GPU work: the link stops, Rust waits for the work
    /// in flight) or came back.
    func setRendering(active: Bool) {
        base.explorer?.setActive(active: active)
        link?.isPaused = !active
        if active { setNeedsLayout() }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let scale = window?.screen.scale ?? UIScreen.main.scale
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        base.metalLayer.frame = bounds
        CATransaction.commit()
        base.resize(width: bounds.width, height: bounds.height, scale: scale, host: self)
        base.model.metresPerPoint = ScreenMetrics.metresPerPoint(window?.screen)
        sendInsets()
    }

    override func safeAreaInsetsDidChange() {
        super.safeAreaInsetsDidChange()
        sendInsets()
    }

    /// Notch, home indicator and status bar areas are not canvas for the pointer and the
    /// edge band.
    private func sendInsets() {
        let i = safeAreaInsets
        guard i != sentInsets else { return }
        sentInsets = i
        send(.insets(top: Double(i.top), left: Double(i.left), bottom: Double(i.bottom), right: Double(i.right)))
    }

    /// The HUD in this view's coordinates (SwiftUI's global space is the window's).
    fileprivate var hudRect: CGRect {
        guard let window, let g = base.model.hudFrame, !g.isEmpty else { return .zero }
        return convert(g, from: window)
    }

    func hudFrameChanged() { pointerInteraction?.invalidate() }

    func requestFrames() { link?.isPaused = false }

    func step(_ link: CADisplayLink) {
        if !base.step(timestamp: link.timestamp) { link.isPaused = true }
    }

    @discardableResult
    func send(_ event: InputEvent) -> InputResult { base.send(event, host: self) }

    /// Offset of a view location from the centre, in points, y up.
    private func offset(_ p: CGPoint) -> (x: Double, y: Double) {
        (Double(p.x - bounds.midX), Double(bounds.midY - p.y))
    }

    // MARK: Hardware keyboard (iPad): the same keys as on the Mac

    override var canBecomeFirstResponder: Bool { true }

    override func resignFirstResponder() -> Bool {
        send(.focusLost)
        return super.resignFirstResponder()
    }

    private static func navKey(_ key: UIKey) -> NavKey? {
        switch key.keyCode {
        case .keyboardLeftArrow: return .left
        case .keyboardRightArrow: return .right
        case .keyboardUpArrow: return .up
        case .keyboardDownArrow: return .down
        default: break
        }
        let shift = key.modifierFlags.contains(.shift)
        switch key.charactersIgnoringModifiers.lowercased() {
        case "w": return .up
        case "s": return .down
        case "a": return .left
        case "d": return .right
        case " ": return .zoom
        case "q": return .slowerZoom
        case "e": return .fasterZoom
        case "+", "=": return .stepIn
        case "-", "_": return .stepOut
        case "r": return .reset
        case "z": return .autopilot
        case "j": return shift ? .journeyAlt : .journey
        case "x": return .elsewhere
        case "n": return .seekMinibrot
        case ",", "<": return .rotateCcw
        case ".", ">": return .rotateCw
        case "/": return .straighten
        default: return key.keyCode == .keyboardEscape ? .stop : nil
        }
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        TouchHints.shared.dismiss()
        var unhandled = Set<UIPress>()
        for press in presses {
            if let key = press.key, key.modifierFlags.isDisjoint(with: [.command, .control, .alternate]) {
                // A guided tour: → next, ← back, Esc ends it (kb: decisions/controls "Guided tours").
                if base.model.tour != nil {
                    switch key.keyCode {
                    case .keyboardRightArrow: base.model.tourNext(); continue
                    case .keyboardLeftArrow: base.model.tourBack(); continue
                    case .keyboardEscape: base.model.endTour(); continue
                    default: break
                    }
                }
                switch key.charactersIgnoringModifiers.lowercased() {
                case "o": base.model.toggleOrbit(); continue
                case "m": base.model.toggleMap(); continue
                case "p": base.model.togglePiMeter(); continue
                case "c": base.model.toggleCosmicScale(); continue
                case "l": base.model.toggleLaneAssist(); continue
                case "g":
                    // Compute further at its time limit (the HUD's "Continue").
                    if base.model.canContinue { base.model.continueComputing() }
                    continue
                default: break
                }
            }
            guard let key = press.key, let nav = Self.navKey(key) else { unhandled.insert(press); continue }
            send(.shift(down: key.modifierFlags.contains(.shift)))
            if !send(.keyDown(key: nav)).handled { unhandled.insert(press) }
        }
        if !unhandled.isEmpty { super.pressesBegan(unhandled, with: event) }
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var unhandled = Set<UIPress>()
        for press in presses {
            guard let key = press.key, let nav = Self.navKey(key), send(.keyUp(key: nav)).handled else {
                unhandled.insert(press)
                continue
            }
        }
        if !unhandled.isEmpty { super.pressesEnded(unhandled, with: event) }
    }

    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        send(.focusLost)
        super.pressesCancelled(presses, with: event)
    }

    // MARK: Touch

    @objc private func handlePinch(_ g: UIPinchGestureRecognizer) {
        let center = g.location(in: self)
        switch g.state {
        case .began:
            lastPinchScale = g.scale
            lastPinchCenter = center
        case .changed:
            let o = offset(center)
            // Two-finger drag pans while pinching.
            send(.pinch(ratio: Double(g.scale / lastPinchScale), x: o.x, y: o.y,
                        dx: Double(center.x - lastPinchCenter.x), dy: -Double(center.y - lastPinchCenter.y)))
            lastPinchScale = g.scale
            lastPinchCenter = center
        case .ended:
            send(.gestureEnd)
        default: break
        }
    }

    @objc private func handleRotation(_ g: UIRotationGestureRecognizer) {
        guard g.state == .changed || g.state == .ended else { return }
        let o = offset(g.location(in: self))
        // UIKit's rotation is clockwise-positive on screen; Rust takes counter-clockwise.
        send(.rotate(radians: -Double(g.rotation), x: o.x, y: o.y))
        g.rotation = 0
    }

    @objc private func handleHover(_ g: UIHoverGestureRecognizer) {
        switch g.state {
        case .began, .changed:
            let p = g.location(in: self)
            if hudRect.contains(p) {
                send(.pointerExited)
            } else {
                let o = offset(p)
                send(.pointer(x: o.x, y: o.y))
            }
        default:
            send(.pointerExited)
        }
    }

    @objc private func handlePan(_ g: UIPanGestureRecognizer) {
        // A long press's drag moves the orbit probe, not the view.
        if longPressing {
            g.setTranslation(.zero, in: self)
            return
        }
        // A drag that starts on the compass turns the view (the needle follows the finger).
        if g.state == .began {
            let p = g.location(in: self), t = g.translation(in: self)
            let o = offset(CGPoint(x: p.x - t.x, y: p.y - t.y))
            compassPanning = base.model.compassPress(x: o.x, y: o.y)
        }
        if compassPanning {
            let o = offset(g.location(in: self))
            switch g.state {
            case .began, .changed:
                base.model.compassDrag(x: o.x, y: o.y)
            default:
                compassPanning = false
                base.model.compassRelease()
            }
            g.setTranslation(.zero, in: self)
            return
        }
        // Autopilot and cruise: the finger steers (kb: decisions/controls "Touch"): in C like the
        // Mac pointer while it is down, in A its direction is the wish, kept after it lifts.
        if g.state == .began { steering = base.fingerSteers }
        if steering {
            switch g.state {
            case .began, .changed:
                let o = offset(g.location(in: self))
                send(.touchSteer(x: o.x, y: o.y))
            default:
                steering = false
                send(.touchSteerEnd)
            }
            g.setTranslation(.zero, in: self)
            return
        }
        let t = g.translation(in: self)
        send(.pan(dx: Double(t.x), dy: -Double(t.y)))
        g.setTranslation(.zero, in: self)
        // Lifted while moving: the view flings on (Rust measures the velocity).
        if g.state == .ended { send(.gestureEnd) }
    }

    @objc private func handleDoubleTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        // On a help badge a double tap does nothing more; on the compass it turns to north,
        // like a tap; no zoom. On the cockpit nothing (its taps are single).
        if base.model.helpPress(x: o.x, y: o.y, size: bounds.size, show: false) { return }
        if base.model.explorer?.cockpitHit(x: o.x, y: o.y) != nil { return }
        if base.model.compassTap(x: o.x, y: o.y) { return }
        send(.doubleTap(x: o.x, y: o.y, out: false))
    }

    @objc private func handleTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        // A tap on the compass turns to north; on a mini-map it jumps there; elsewhere it
        // pauses / resumes the autopilot. A help badge opens its popover first.
        if Date().timeIntervalSince(longPressEnded) < 0.3 { return }
        if base.model.helpPress(x: o.x, y: o.y, size: bounds.size) { return }
        // A cockpit icon switches the driving level, the lane icon toggles lane assist.
        if let control = base.model.explorer?.cockpitHit(x: o.x, y: o.y) {
            base.model.cockpitClick(control)
            return
        }
        if base.model.compassTap(x: o.x, y: o.y) { return }
        if base.model.mapClick(x: o.x, y: o.y, fly: false) { return }
        send(.click(x: o.x, y: o.y))
    }

    /// A press on a control Rust draws (help badge, cockpit, compass): no double-tap zoom, no
    /// long-press menu there.
    fileprivate func onControl(_ p: CGPoint) -> Bool {
        let o = offset(p)
        guard let explorer = base.explorer else { return false }
        return explorer.helpHit(x: o.x, y: o.y) != nil || explorer.cockpitHit(x: o.x, y: o.y) != nil
    }

    /// A finger held still: Rust says what it means where it rests and in this level (kb:
    /// decisions/controls "Touch", `touch_hold`). In A, C and a journey's flight it steers from
    /// where it rests and on as it moves (a resting finger before a drag used to open the menu
    /// instead); on a mini-map it flies there (the Mac's ⌥-click); otherwise its release in place
    /// opens the menu, and with the orbit overlay on its drag moves the probe.
    @objc private func handleLongPress(_ g: UILongPressGestureRecognizer) {
        let p = g.location(in: self)
        let o = offset(p)
        switch g.state {
        case .began:
            longPressing = true
            longPressStart = p
            longPressMoved = 0
            hold = base.explorer?.touchHold(x: o.x, y: o.y) ?? .nothing
            switch hold {
            case .steer:
                send(.touchSteer(x: o.x, y: o.y))
            case .flyToMap:
                if base.model.mapClick(x: o.x, y: o.y, fly: true) { Haptics.press() }
            case .menu:
                // With the orbit overlay on, c jumps under the finger and follows it.
                if base.model.orbitEnabled { send(.probe(x: o.x, y: o.y)) }
            case .nothing:
                break
            }
        case .changed:
            longPressMoved = max(longPressMoved, hypot(p.x - longPressStart.x, p.y - longPressStart.y))
            if hold == .steer {
                send(.touchSteer(x: o.x, y: o.y))
            } else if hold == .menu && base.model.orbitEnabled {
                send(.probe(x: o.x, y: o.y))
            }
        case .ended:
            longPressing = false
            longPressEnded = Date()
            if hold == .steer { send(.touchSteerEnd) }
            // A press that stayed put opens the menu there; a drag only moved the probe.
            if hold == .menu && longPressMoved < 12 {
                let p = longPressStart, o = offset(p)
                base.model.canvasMenu = CanvasMenu(x: o.x, y: o.y, rect: CGRect(x: p.x - 2, y: p.y - 2, width: 4, height: 4),
                                                   edge: p.y > bounds.midY ? .top : .bottom)
            }
            hold = .nothing
        default:
            longPressing = false
            longPressEnded = Date()
            if hold == .steer { send(.touchSteerEnd) }
            hold = .nothing
        }
    }

    @objc private func handleTwoFingerTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        send(.doubleTap(x: o.x, y: o.y, out: true))
    }
}

extension FractalUIView: UIPointerInteractionDelegate {
    /// Over the canvas: a crosshair pointer (pointer mode) or none (keyboard mode, the overlay
    /// crosshair shows the target); over the HUD the system pointer.
    func pointerInteraction(_ interaction: UIPointerInteraction, regionFor request: UIPointerRegionRequest,
                            defaultRegion: UIPointerRegion) -> UIPointerRegion? {
        hudRect.contains(request.location) ? nil : defaultRegion
    }

    func pointerInteraction(_ interaction: UIPointerInteraction, styleFor region: UIPointerRegion) -> UIPointerStyle? {
        if base.mode == .keyboard { return .hidden() }
        let path = UIBezierPath()
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] as [(CGFloat, CGFloat)] {
            path.move(to: CGPoint(x: dx * 3, y: dy * 3))
            path.addLine(to: CGPoint(x: dx * 10, y: dy * 10))
        }
        let shape = UIBezierPath(cgPath: path.cgPath.copy(strokingWithWidth: 1.5, lineCap: .round, lineJoin: .round,
                                                           miterLimit: 1))
        return UIPointerStyle(shape: .path(shape), constrainedAxes: [])
    }
}

extension FractalUIView: UIGestureRecognizerDelegate {
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        true
    }

    /// On a control Rust draws (help badge, cockpit) a double tap is not waited for (the tap
    /// fires at once) and a long press opens no menu. Where a finger steers (A, C, a journey's
    /// flight) a finger resting 0.2 s already steers; the menu's long press is 0.45 s.
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if let press = g as? UILongPressGestureRecognizer {
            press.minimumPressDuration = base.fingerSteers ? Self.steerHold : Self.menuHold
        }
        if g is UILongPressGestureRecognizer || (g as? UITapGestureRecognizer)?.numberOfTapsRequired == 2 {
            return !onControl(touch.location(in: self))
        }
        return true
    }

    static let menuHold: TimeInterval = 0.45
    static let steerHold: TimeInterval = 0.2
}

struct FractalCanvas: UIViewRepresentable {
    let model: ExplorerModel
    func makeUIView(context: Context) -> FractalUIView { FractalUIView(model: model) }
    func updateUIView(_ view: FractalUIView, context: Context) {}
}
#endif
