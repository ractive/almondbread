#if os(iOS)
import SwiftUI
import UIKit
import UIKit.UIGestureRecognizerSubclass

// First-run touch hints (kb: decisions/controls "Touch", "First run"): one layer over the picture
// the first time the app starts on iPhone or iPad, a few words each: drag, pinch, hold, the
// throttle and the cockpit's plane (the autopilot). It takes no touches: the first touch anywhere (or a key)
// dismisses it and does what it would have done anyway. Help shows it again.

/// Whether the hints show. Change-only (the layer reads only `shown`).
@MainActor @Observable
final class TouchHints {
    static let shared = TouchHints()
    static let seenKey = "touchHints.seen"
    /// `--touch-hints`: show them whatever was seen (UI tests, screenshots).
    static let forced = CommandLine.arguments.contains("--touch-hints")

    /// Not seen yet (window tests and the UI tests count them as seen: settings are ephemeral).
    var unseen: Bool { !TestLaunch.ephemeral && !UserDefaults.standard.bool(forKey: Self.seenKey) }

    /// Unless forced, they wait while the walkthrough is offered (kb: decisions/controls
    /// "Walkthrough"): "Just explore" shows them, the walkthrough says the same.
    var shown = forced || (!TestLaunch.ephemeral && !UserDefaults.standard.bool(forKey: seenKey)
                           && !WalkthroughOffer.shared.shown)

    func show() {
        withAnimation(.easeOut(duration: 0.25)) { shown = true }
    }

    func dismiss() {
        guard shown else { return }
        withAnimation(.easeOut(duration: 0.3)) { shown = false }
        if !TestLaunch.ephemeral { UserDefaults.standard.set(true, forKey: Self.seenKey) }
    }
}

/// Sees every touch in the window without taking part in it (it fails at once, cancels and
/// delays nothing): the hints go on the first touch, wherever it lands.
final class TouchObserver: UIGestureRecognizer {
    private let onTouch: () -> Void

    init(_ onTouch: @escaping () -> Void) {
        self.onTouch = onTouch
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        onTouch()
        state = .failed
    }
}

/// Where the SwiftUI controls the hints point at are (the throttle). The cockpit is drawn by
/// Rust; its plane comes from `cockpitPlane`.
struct TouchHintAnchors: PreferenceKey {
    static let defaultValue: [String: Anchor<CGRect>] = [:]
    static func reduce(value: inout [String: Anchor<CGRect>], nextValue: () -> [String: Anchor<CGRect>]) {
        value.merge(nextValue()) { $1 }
    }
}

extension View {
    /// Marks a control the first-run hints point at.
    func touchHintAnchor(_ id: String) -> some View {
        anchorPreference(key: TouchHintAnchors.self, value: .bounds) { [id: $0] }
    }

    /// The hints layer over this view, pointing at the controls marked with `touchHintAnchor`
    /// and at the cockpit's plane icon (`cockpitPlane`: its rect in global coordinates, if drawn).
    func touchHints(cockpitPlane: @escaping () -> CGRect?) -> some View {
        overlayPreferenceValue(TouchHintAnchors.self) { anchors in
            TouchHintsOverlay(anchors: anchors, cockpitPlane: cockpitPlane)
        }
    }
}

/// Reads only `TouchHints.shared.shown`.
struct TouchHintsOverlay: View {
    let anchors: [String: Anchor<CGRect>]
    let cockpitPlane: () -> CGRect?
    private var hints = TouchHints.shared

    init(anchors: [String: Anchor<CGRect>], cockpitPlane: @escaping () -> CGRect?) {
        self.anchors = anchors
        self.cockpitPlane = cockpitPlane
    }

    var body: some View {
        if hints.shown {
            // The cockpit is laid out by Rust on its frames: read its plane again twice a
            // second while the hints show (they show once, at the first start).
            TimelineView(.periodic(from: .now, by: 0.5)) { _ in
                TouchHintsLayer(anchors: anchors, plane: cockpitPlane())
            }
            .transition(.opacity)
        }
    }
}

/// The hints: the gestures in the middle, a label beside the throttle and one above the
/// cockpit's plane icon. Matches the app's overlays (material capsules, white type); no hit
/// testing.
struct TouchHintsLayer: View {
    let anchors: [String: Anchor<CGRect>]
    /// The cockpit's plane icon in global coordinates.
    let plane: CGRect?

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            let throttle = anchors["throttle"].map { proxy[$0] }
            let origin = proxy.frame(in: .global).origin
            let plane = plane.map { $0.offsetBy(dx: -origin.x, dy: -origin.y) }
            ZStack(alignment: .topLeading) {
                Color.black.opacity(0.4)
                    .ignoresSafeArea()
                VStack(spacing: 10) {
                    Self.chip("hand.draw.fill", "Drag to move")
                    Self.chip("hand.pinch.fill", "Pinch to zoom")
                    Self.chip("hand.point.up.left.fill", "Hold for more")
                    Text("Touch anywhere to start")
                        .font(.footnote.weight(.medium))
                        .foregroundStyle(.white.opacity(0.75))
                        .padding(.top, 4)
                }
                .fixedSize()
                .position(Self.middle(size: size, throttle: throttle, plane: plane))
                if let throttle {
                    Self.beside(throttle, Self.chip("arrow.up.and.down", "Up: in · Down: out · M: stop"))
                }
                if let plane {
                    Self.above(plane, width: size.width, Self.chip("airplane", "Flies by itself"))
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("touch-hints")
        .accessibilityLabel("Hints: drag to move, pinch to zoom, hold for more, slide the throttle up to fly in, down to fly out, to the middle to stop, the cockpit's plane flies by itself; while flying, rest a finger to steer, swipe to move the picture")
    }

    /// Where the gestures group is centred: the middle, unless the throttle's label (left of the
    /// throttle, at its middle; about 320 points wide) would run into it. Then left of that label
    /// where the screen is wide enough (iPad landscape, a phone on its side), else higher, ending
    /// above it (phone portrait, iPad portrait). Always above the plane's label (about 60 points
    /// over the cockpit's plane; a phone on its side has little height).
    static func middle(size: CGSize, throttle: CGRect?, plane: CGRect? = nil) -> CGPoint {
        let (halfW, halfH): (CGFloat, CGFloat) = (150, 110)
        let centre = CGPoint(x: size.width / 2, y: size.height / 2)
        var p = centre
        if let throttle {
            let labelLeft = throttle.minX - 330
            if centre.x + halfW <= labelLeft || abs(throttle.midY - centre.y) > halfH + 40 {
                p = centre
            } else if labelLeft - halfW - 20 >= halfW + 12 {
                p = CGPoint(x: min(centre.x, labelLeft - halfW - 20), y: centre.y)
            } else {
                p = CGPoint(x: centre.x, y: max(throttle.midY - 40 - halfH, min(centre.y, halfH + 60)))
            }
        }
        if let plane {
            p.y = max(halfH + 12, min(p.y, plane.minY - 70 - halfH))
        }
        return p
    }

    static func chip(_ symbol: String, _ text: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.title3)
                .frame(width: 28)
            Text(text)
                .font(.callout.weight(.semibold))
                .lineLimit(1)
                .fixedSize()
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: Capsule(style: .continuous))
        .overlay(Capsule(style: .continuous).strokeBorder(.white.opacity(0.18)))
        .shadow(color: .black.opacity(0.35), radius: 6, y: 2)
    }

    /// The label left of a control (vertically centred), pointing at it.
    static func beside(_ rect: CGRect, _ label: some View) -> some View {
        HStack(spacing: 4) {
            label
            Image(systemName: "arrowtriangle.right.fill")
                .font(.caption)
                .foregroundStyle(.white)
        }
        .frame(width: max(0, rect.minX - 4), height: rect.height, alignment: .trailing)
        .offset(y: rect.minY)
    }

    /// The label above a control, centred on it (kept on the screen), pointing down at it.
    static func above(_ rect: CGRect, width: CGFloat, _ label: some View) -> some View {
        let half = max(0, min(rect.midX, width - rect.midX))
        return VStack(spacing: 2) {
            label
            Image(systemName: "arrowtriangle.down.fill")
                .font(.caption)
                .foregroundStyle(.white)
        }
        .fixedSize()
        .frame(width: 2 * half, height: max(0, rect.minY - 2), alignment: .bottom)
        .offset(x: rect.midX - half)
    }
}
#endif
