import AlmondbreadCore
import QuartzCore
import SwiftUI

// The platform views host the Rust renderer (kb: platform/render-architecture, "Hosting in
// the app"): each owns a CAMetalLayer whose pointer goes to Rust once, runs a display link
// that makes one `Explorer.frame` call per refresh and pauses while nothing animates, and
// decodes platform events into `InputEvent`s. Motion policy (pan speed, zoom target, zoom
// speed, idle timing) is Rust's `Navigator`; nothing here integrates or interprets motion.

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

    init(model: ExplorerModel, scale: CGFloat) {
        self.model = model
        #if os(macOS)
        // The kernel writes sRGB-encoded values; tag the layer so they are colour-matched to
        // wide-gamut (P3) displays instead of being shown as raw display values.
        metalLayer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        #endif
        metalLayer.isOpaque = true
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
        guard let explorer else { return InputResult(handled: false, redraw: false) }
        let r = explorer.input(event: event)
        if r.redraw { host.requestFrames() }
        return r
    }

    func resize(width: CGFloat, height: CGFloat, scale: CGFloat, host: ExplorerHost) {
        explorer?.resize(width: Double(width), height: Double(height), scale: Double(scale))
        host.requestFrames()
    }

    /// One refresh; returns false when the display link should pause.
    func step(timestamp: CFTimeInterval) -> Bool {
        guard let explorer else { return false }
        let t0 = CACurrentMediaTime()
        let animating = explorer.frame(timestamp: timestamp)
        let t1 = CACurrentMediaTime()
        frameTimes?.append(t1 - t0)
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
    private var tracking: NSTrackingArea?

    init(model: ExplorerModel) {
        base = FractalHostBase(model: model, scale: NSScreen.main?.backingScaleFactor ?? 2)
        super.init(frame: .zero)
        wantsLayer = true
        layerContentsRedrawPolicy = .never
        base.attach(host: self)
        if InteractionTest.enabled { InteractionTest.shared.attach(view: self) }
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func makeBackingLayer() -> CALayer { base.metalLayer }
    override var isOpaque: Bool { true }
    override var acceptsFirstResponder: Bool { true }

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

    private func layoutExplorer() {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        base.resize(width: bounds.width, height: bounds.height, scale: scale, host: self)
    }

    func requestFrames() { link?.isPaused = false }

    func step(_ link: CADisplayLink) {
        if !base.step(timestamp: link.timestamp) { link.isPaused = true }
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
    }

    /// The navigation meaning of a key (kb: decisions/controls), or nil.
    static func navKey(_ event: NSEvent) -> NavKey? {
        switch event.specialKey {
        case .leftArrow?: return .left
        case .rightArrow?: return .right
        case .upArrow?: return .up
        case .downArrow?: return .down
        default: break
        }
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
        guard let key = Self.navKey(event) else { return false }
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
        let o = offset(event.locationInWindow)
        send(.pointer(x: o.x, y: o.y))
    }

    override func mouseEntered(with event: NSEvent) { mouseMoved(with: event) }
    override func mouseExited(with event: NSEvent) { send(.pointerExited) }

    override func scrollWheel(with event: NSEvent) {
        let zoomModifier = event.modifierFlags.contains(.command) || event.modifierFlags.contains(.option)
        if event.hasPreciseScrollingDeltas && !zoomModifier {
            // Trackpad: two-finger scroll pans; content follows the fingers.
            send(.pan(dx: Double(event.scrollingDeltaX), dy: -Double(event.scrollingDeltaY)))
        } else {
            let delta = event.hasPreciseScrollingDeltas ? Double(event.scrollingDeltaY) / 40 : Double(event.scrollingDeltaY)
            let o = offset(event.locationInWindow)
            send(.wheelZoom(delta: delta, x: o.x, y: o.y))
        }
    }

    override func magnify(with event: NSEvent) {
        let o = offset(event.locationInWindow)
        send(.magnify(magnification: Double(event.magnification), x: o.x, y: o.y))
    }

    override func mouseDragged(with event: NSEvent) {
        send(.pan(dx: Double(event.deltaX), dy: -Double(event.deltaY)))
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
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

    // The base creates the layer before the view exists; UIKit asks the class for its layer,
    // so the view's own layer is a plain one hosting the Metal layer as a sublayer.
    init(model: ExplorerModel) {
        base = FractalHostBase(model: model, scale: UIScreen.main.scale)
        super.init(frame: .zero)
        layer.addSublayer(base.metalLayer)
        isMultipleTouchEnabled = true
        base.attach(host: self)

        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(handlePinch(_:)))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
        pan.maximumNumberOfTouches = 1
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(handleDoubleTap(_:)))
        doubleTap.numberOfTapsRequired = 2
        let twoFingerTap = UITapGestureRecognizer(target: self, action: #selector(handleTwoFingerTap(_:)))
        twoFingerTap.numberOfTouchesRequired = 2
        pinch.delegate = self
        pan.delegate = self
        [pinch, pan, doubleTap, twoFingerTap].forEach(addGestureRecognizer)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        link?.invalidate()
        link = nil
        guard let window else {
            send(.focusLost)
            return
        }
        becomeFirstResponder()
        let l = CADisplayLink(target: DisplayLinkProxy(self), selector: #selector(DisplayLinkProxy.step(_:)))
        let fps = Float(window.screen.maximumFramesPerSecond)
        l.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, fps), maximum: fps, preferred: fps)
        l.add(to: .main, forMode: .common)
        link = l
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let scale = window?.screen.scale ?? UIScreen.main.scale
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        base.metalLayer.frame = bounds
        CATransaction.commit()
        base.resize(width: bounds.width, height: bounds.height, scale: scale, host: self)
    }

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
        default: return nil
        }
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var unhandled = Set<UIPress>()
        for press in presses {
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
        default: break
        }
    }

    @objc private func handlePan(_ g: UIPanGestureRecognizer) {
        let t = g.translation(in: self)
        send(.pan(dx: Double(t.x), dy: -Double(t.y)))
        g.setTranslation(.zero, in: self)
    }

    @objc private func handleDoubleTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        send(.doubleTap(x: o.x, y: o.y, out: false))
    }

    @objc private func handleTwoFingerTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        send(.doubleTap(x: o.x, y: o.y, out: true))
    }
}

extension FractalUIView: UIGestureRecognizerDelegate {
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        true
    }
}

struct FractalCanvas: UIViewRepresentable {
    let model: ExplorerModel
    func makeUIView(context: Context) -> FractalUIView { FractalUIView(model: model) }
    func updateUIView(_ view: FractalUIView, context: Context) {}
}
#endif
