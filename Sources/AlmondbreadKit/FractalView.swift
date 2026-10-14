import MetalKit
import SwiftUI

/// Shared behaviour of the platform Metal views: owns the renderer, wires redraw requests,
/// runs continuous-zoom animation.
@MainActor
protocol FractalHostView: MTKView {
    var renderer: Renderer? { get }
}

extension FractalHostView {
    func configure(renderer: Renderer) {
        device = renderer.device
        colorPixelFormat = .bgra8Unorm
        framebufferOnly = false           // the colorize kernel writes the drawable directly
        #if os(macOS)
        // The kernel writes sRGB-encoded values; tag the layer so they are colour-matched to
        // wide-gamut (P3) displays instead of being shown as raw display values.
        colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        #endif
        // Default is 60: ask for the display's maximum so gestures run at 120 Hz on ProMotion.
        #if os(macOS)
        preferredFramesPerSecond = NSScreen.main?.maximumFramesPerSecond ?? 60
        #else
        preferredFramesPerSecond = UIScreen.main.maximumFramesPerSecond
        #endif
        isPaused = true
        enableSetNeedsDisplay = true
        autoResizeDrawable = true
        delegate = renderer
        renderer.requestRedraw = { [weak self] in self?.redraw() }
        renderer.setAnimating = { [weak self] on in
            guard let self, self.isPaused == on else { return }
            self.isPaused = !on
            self.enableSetNeedsDisplay = !on
        }
    }

    func redraw() {
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }

    func startAnimating() {
        renderer?.setAnimating?(true)
    }
}

#if os(macOS)
import AppKit

final class FractalNSView: MTKView, FractalHostView {
    var renderer: Renderer?
    let keys = KeyControls()
    private var keyMonitor: Any?
    private var resignObserver: NSObjectProtocol?
    private var model: FractalModel? { renderer?.model }

    init(renderer: Renderer) {
        self.renderer = renderer
        super.init(frame: .zero, device: renderer.device)
        configure(renderer: renderer)
        renderer.onFrame = { [weak self] dt in self?.tick(dt) ?? false }
        if InteractionTest.enabled { InteractionTest.shared.attach(view: self) }
    }

    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var acceptsFirstResponder: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
        window?.acceptsMouseMovedEvents = true
        if let window { TestLaunch.placeInBackground(window) }
        installKeyHandling()
    }

    /// Offset of a window location from the view centre, in view points, y up.
    private func offset(_ locationInWindow: NSPoint) -> (Double, Double) {
        let p = convert(locationInWindow, from: nil)
        return (Double(p.x - bounds.midX), Double(p.y - bounds.midY))
    }

    private func mouseOffset() -> (Double, Double) {
        guard let window else { return (0, 0) }
        let p = window.mouseLocationOutsideOfEventStream
        let (dx, dy) = offset(p)
        // Outside the view: zoom around the centre.
        if abs(dx) > bounds.width / 2 || abs(dy) > bounds.height / 2 { return (0, 0) }
        return (dx, dy)
    }

    func zoom(_ factor: Double, at o: (Double, Double)) {   // internal: also driven by InteractionTest
        renderer?.noteInteraction()
        model?.viewport.zoom(by: factor, aroundOffset: o.0, o.1)
    }

    func pan(_ dx: Double, _ dy: Double) {
        renderer?.noteInteraction()
        model?.viewport.pan(byPoints: dx, dy)
    }

    private func tick(_ dt: Double) -> Bool {
        guard keys.isMoving, let model else { return false }
        let m = keys.step(dt: dt, viewWidth: Double(bounds.width), zoomSpeed: model.zoomSpeed)
        if m.dx != 0 || m.dy != 0 { pan(m.dx, m.dy) }
        if m.zoom != 1 { zoom(m.zoom, at: mouseOffset()) }
        return true
    }

    // MARK: Keys

    /// Keys are taken from a window-local monitor rather than keyDown, so navigation works while
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
            MainActor.assumeIsolated { self?.keys.releaseAll() }
        }
    }

    /// Returns true if the event was consumed.
    func handleKey(_ event: NSEvent) -> Bool {
        guard let model else { return false }
        if event.type == .flagsChanged {
            keys.shift = event.modifierFlags.contains(.shift)
            return false
        }
        // Menu shortcuts and text fields keep their keys.
        if event.modifierFlags.contains(.command) { return false }
        if window?.firstResponder is NSText { return false }
        let chars = event.charactersIgnoringModifiers ?? ""
        var arrow: KeyControls.Key?
        switch event.specialKey {
        case .leftArrow?: arrow = .left
        case .rightArrow?: arrow = .right
        case .upArrow?: arrow = .up
        case .downArrow?: arrow = .down
        default: break
        }
        if event.type == .keyUp { return keys.release(chars, arrow: arrow) }
        keys.shift = event.modifierFlags.contains(.shift)
        if keys.press(chars, arrow: arrow, isRepeat: event.isARepeat, model: model) {
            if keys.isMoving { startAnimating() }
            return true
        }
        switch chars {
        case "+", "=": zoom(1.5, at: (0, 0))
        case "-", "_": zoom(1 / 1.5, at: (0, 0))
        case "r", "R": model.reset()
        default: return false
        }
        return true
    }

    // MARK: Input

    override func scrollWheel(with event: NSEvent) {
        let zoomModifier = event.modifierFlags.contains(.command) || event.modifierFlags.contains(.option)
        if event.hasPreciseScrollingDeltas && !zoomModifier {
            // Trackpad: two-finger scroll pans; content follows the fingers.
            pan(Double(event.scrollingDeltaX), -Double(event.scrollingDeltaY))
        } else {
            let delta = event.hasPreciseScrollingDeltas ? Double(event.scrollingDeltaY) / 40 : Double(event.scrollingDeltaY)
            zoom(pow(1.15, delta), at: offset(event.locationInWindow))
        }
    }

    override func magnify(with event: NSEvent) {
        zoom(1 + Double(event.magnification), at: offset(event.locationInWindow))
    }

    override func mouseDragged(with event: NSEvent) {
        pan(Double(event.deltaX), -Double(event.deltaY))
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        if event.clickCount == 2 {
            let out = event.modifierFlags.contains(.option)
            zoom(out ? 0.5 : 2, at: offset(event.locationInWindow))
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        if event.clickCount == 2 { zoom(0.5, at: offset(event.locationInWindow)) }
    }

    override func keyDown(with event: NSEvent) {
        // Keys arrive through the local monitor; swallow ours here too (no beep).
        if !handleKey(event) { super.keyDown(with: event) }
    }
}

struct FractalCanvas: NSViewRepresentable {
    let renderer: Renderer
    func makeNSView(context: Context) -> FractalNSView { FractalNSView(renderer: renderer) }
    func updateNSView(_ view: FractalNSView, context: Context) {}
}

#else
import UIKit

final class FractalUIView: MTKView, FractalHostView {
    var renderer: Renderer?
    let keys = KeyControls()
    private var model: FractalModel? { renderer?.model }
    private var lastPinchScale: CGFloat = 1
    private var lastPinchCenter: CGPoint = .zero

    init(renderer: Renderer) {
        self.renderer = renderer
        super.init(frame: .zero, device: renderer.device)
        configure(renderer: renderer)
        isMultipleTouchEnabled = true

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
        renderer.onFrame = { [weak self] dt in self?.tick(dt) ?? false }
    }

    // MARK: Hardware keyboard (iPad): same keys as on the Mac

    override var canBecomeFirstResponder: Bool { true }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil { becomeFirstResponder() } else { keys.releaseAll() }
    }

    override func resignFirstResponder() -> Bool {
        keys.releaseAll()
        return super.resignFirstResponder()
    }

    private func decode(_ press: UIPress) -> (String, KeyControls.Key?)? {
        guard let key = press.key else { return nil }
        switch key.keyCode {
        case .keyboardLeftArrow: return ("", .left)
        case .keyboardRightArrow: return ("", .right)
        case .keyboardUpArrow: return ("", .up)
        case .keyboardDownArrow: return ("", .down)
        default: return (key.charactersIgnoringModifiers, nil)
        }
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var unhandled = Set<UIPress>()
        for press in presses {
            guard let model, let (chars, arrow) = decode(press) else { unhandled.insert(press); continue }
            keys.shift = press.key?.modifierFlags.contains(.shift) ?? false
            if keys.press(chars, arrow: arrow, isRepeat: false, model: model) {
                if keys.isMoving { startAnimating() }
            } else {
                unhandled.insert(press)
            }
        }
        if !unhandled.isEmpty { super.pressesBegan(unhandled, with: event) }
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var unhandled = Set<UIPress>()
        for press in presses {
            guard let (chars, arrow) = decode(press), keys.release(chars, arrow: arrow) else {
                unhandled.insert(press)
                continue
            }
        }
        if !unhandled.isEmpty { super.pressesEnded(unhandled, with: event) }
    }

    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        keys.releaseAll()
        super.pressesCancelled(presses, with: event)
    }

    private func tick(_ dt: Double) -> Bool {
        guard keys.isMoving, let model else { return false }
        let m = keys.step(dt: dt, viewWidth: Double(bounds.width), zoomSpeed: model.zoomSpeed)
        renderer?.noteInteraction()
        if m.dx != 0 || m.dy != 0 { model.viewport.pan(byPoints: m.dx, m.dy) }
        if m.zoom != 1 { model.viewport.zoom(by: m.zoom, aroundOffset: 0, 0) }
        return true
    }

    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    /// Offset of a view location from the centre, in points, y up.
    private func offset(_ p: CGPoint) -> (Double, Double) {
        (Double(p.x - bounds.midX), Double(bounds.midY - p.y))
    }

    @objc private func handlePinch(_ g: UIPinchGestureRecognizer) {
        let center = g.location(in: self)
        switch g.state {
        case .began:
            lastPinchScale = g.scale
            lastPinchCenter = center
        case .changed:
            renderer?.noteInteraction()
            // Two-finger drag pans while pinching.
            model?.viewport.pan(byPoints: Double(center.x - lastPinchCenter.x), -Double(center.y - lastPinchCenter.y))
            let o = offset(center)
            model?.viewport.zoom(by: Double(g.scale / lastPinchScale), aroundOffset: o.0, o.1)
            lastPinchScale = g.scale
            lastPinchCenter = center
        default: break
        }
    }

    @objc private func handlePan(_ g: UIPanGestureRecognizer) {
        let t = g.translation(in: self)
        renderer?.noteInteraction()
        model?.viewport.pan(byPoints: Double(t.x), -Double(t.y))
        g.setTranslation(.zero, in: self)
    }

    @objc private func handleDoubleTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        renderer?.noteInteraction()
        model?.viewport.zoom(by: 2, aroundOffset: o.0, o.1)
    }

    @objc private func handleTwoFingerTap(_ g: UITapGestureRecognizer) {
        let o = offset(g.location(in: self))
        renderer?.noteInteraction()
        model?.viewport.zoom(by: 0.5, aroundOffset: o.0, o.1)
    }
}

extension FractalUIView: UIGestureRecognizerDelegate {
    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        true
    }
}

struct FractalCanvas: UIViewRepresentable {
    let renderer: Renderer
    func makeUIView(context: Context) -> FractalUIView { FractalUIView(renderer: renderer) }
    func updateUIView(_ view: FractalUIView, context: Context) {}
}
#endif
