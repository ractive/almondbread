#if os(iOS)
import AlmondbreadCore
import SwiftUI
import UIKit

// Touch controls (kb: decisions/controls "Touch"): the throttle lever, the long-press actions
// and the haptics. Swift only decodes gestures; what a detent, a steering finger or a probe
// means is the Rust navigator's (`InputEvent::Throttle`, `TouchSteer`, `Probe`, `SeekAt`,
// `FlyHere`).

/// Haptics: light and rare. A selection tick per gear, a firmer one at the zero detent, a light
/// tap when a flight lands (a journey stop, a fly-to, a minibrot) and a success when a minibrot
/// search found one. Nothing for continuous motion.
@MainActor
enum Haptics {
    private static let selection = UISelectionFeedbackGenerator()
    private static let soft = UIImpactFeedbackGenerator(style: .light)
    private static let rigid = UIImpactFeedbackGenerator(style: .rigid)
    private static let notification = UINotificationFeedbackGenerator()

    static func gear() { selection.selectionChanged() }
    static func zeroDetent() { rigid.impactOccurred(intensity: 0.7) }
    static func landed() { soft.impactOccurred(intensity: 0.6) }
    /// A long press that acted at once (the mini-map flies there).
    static func press() { soft.impactOccurred(intensity: 0.8) }
    static func found() { notification.notificationOccurred(.success) }

    /// What changed between two stats readings (~10 Hz): a minibrot found, a landing.
    static func stats(old: ExplorerStats?, new: ExplorerStats) {
        guard let old else { return }
        if old.seek.phase == "searching" && new.seek.phase == "flying" {
            found()
        } else if old.seek.phase == "flying" && new.seek.phase == "arrived" {
            landed()
        } else if !old.journey.atStop && new.journey.atStop {
            landed()
        } else if old.flight.active && !new.flight.active && !new.journey.active && !old.flight.stopping
                    && new.seek.phase != "arrived" {
            // A fly-to (Fly here, Fly to…, the cruise's soft landing on the whole set).
            landed()
        }
    }
}

extension ExplorerModel {
    /// One detent of the throttle lever: the pedals' path in Rust (E up, Q down). Returns the
    /// cockpit's state right after it (for the lever and its haptics).
    @discardableResult
    func throttle(up: Bool) -> CruiseStatus? {
        send(.throttle(up: up))
        return explorer?.cruiseStatus()
    }

    /// The finger left the lever: the next drag may cross zero.
    func throttleEnd() { send(.throttleEnd) }

    /// Long-press "Orbit here": the orbit overlay on, its point c here (a long-press drag moves
    /// it; turning the overlay off gives c back to the zoom target).
    func orbitHere(x: Double, y: Double) {
        if !orbitEnabled { orbitEnabled = true }
        send(.probe(x: x, y: y))
    }

    /// Long-press "Seek Minibrot Here": the search about the point.
    func seekMinibrot(x: Double, y: Double) { send(.seekAt(x: x, y: y)) }

    /// Long-press "Fly Here": the fly-to path to the point, centred and deeper.
    func flyHere(x: Double, y: Double) { send(.flyHere(x: x, y: y)) }

    /// Long-press "Copy Coordinates": the view centred at the point, at this zoom.
    func copyCoordinates(x: Double, y: Double) {
        guard let explorer else { return }
        Clipboard.copy(explorer.coordinatesTextAt(x: x, y: y))
    }
}

/// A long press on the canvas: its point (view points from the centre, y up) and the popover's
/// anchor (canvas coordinates) and side.
struct CanvasMenu: Identifiable, Equatable {
    let id = UUID()
    let x: Double
    let y: Double
    let rect: CGRect
    let edge: Edge
}

/// The long-press menu (kb: decisions/controls "Touch"): what the Mac does at the pointer (N, the
/// orbit under the pointer, ⌥-click fly, ⇧⌘C / ⇧⌘V), as a popover list at the finger. A
/// popover, not the system edit menu: that is a horizontal strip which pages after two items on
/// a phone. Reads only `canvasMenu`.
struct CanvasMenuPopover: ViewModifier {
    @Bindable var model: ExplorerModel

    func body(content: Content) -> some View {
        let anchor = model.canvasMenu ?? model.lastCanvasMenu
        content.popover(item: $model.canvasMenu, attachmentAnchor: .rect(.rect(anchor?.rect ?? .zero)),
                        arrowEdge: anchor?.edge ?? .top) { menu in
            CanvasMenuContent(model: model, menu: menu)
                .presentationCompactAdaptation(.popover)
        }
    }
}

struct CanvasMenuContent: View {
    let model: ExplorerModel
    let menu: CanvasMenu

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            item("Seek Minibrot Here", "scope") { model.seekMinibrot(x: menu.x, y: menu.y) }
            Divider()
            item("Orbit Here", "point.topleft.down.to.point.bottomright.curvepath") { model.orbitHere(x: menu.x, y: menu.y) }
            Divider()
            item("Fly Here", "location.north.line") { model.flyHere(x: menu.x, y: menu.y) }
            Divider()
            item("Copy Coordinates", "doc.on.doc") { model.copyCoordinates(x: menu.x, y: menu.y) }
            if UIPasteboard.general.hasStrings {
                Divider()
                goToCopied
            }
        }
        .frame(width: 250)
    }

    /// "Go to Copied" with the system paste control beside it: reading the clipboard ourselves
    /// asks "Allow Paste?" on every tap (text copied in another app or on the Mac); the paste
    /// control hands it over without asking. The row's title still works (with the prompt).
    private var goToCopied: some View {
        HStack(spacing: 0) {
            item("Go to Copied", "arrow.right.doc.on.clipboard") { model.pasteCoordinates() }
            PasteButton(payloadType: String.self) { strings in
                Task { @MainActor in
                    model.canvasMenu = nil
                    model.pasteCoordinates(text: strings.first)
                }
            }
            .labelStyle(.titleOnly)
            .buttonBorderShape(.capsule)
            .controlSize(.small)
            .padding(.trailing, 12)
        }
    }

    private func item(_ title: String, _ symbol: String, action: @escaping () -> Void) -> some View {
        Button {
            model.canvasMenu = nil
            action()
        } label: {
            HStack(spacing: 12) {
                Image(systemName: symbol).frame(width: 24)
                Text(title)
            }
            .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            .padding(.horizontal, 16)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
    }
}

/// The throttle (kb: decisions/controls "Touch"): a vertical lever at the trailing edge. Drag up
/// for forward gears, down for reverse: one gear per detent ([`detent`] points of finger travel,
/// a selection tick each); the knob follows the finger and springs back when it lifts, the gear
/// stays (a jog lever, so the finger never has to find a position). Within one drag the speed
/// stops at zero (a firmer tick); a new drag crosses. A double tap goes to manual (Esc). In the
/// autopilot and a journey it sets their speed, as E / Q do. The track shows the gear set.
struct ThrottleLever: View {
    let model: ExplorerModel
    var height: CGFloat = 200

    static let width: CGFloat = 48
    /// Finger travel per gear (points).
    static let detent: CGFloat = 14

    @State private var offset: CGFloat = 0
    @State private var stepped = 0
    @State private var dragging = false
    /// The gear right after the last detent (the model's change-only gear lags by ~0.1 s).
    @State private var live: Int?

    var body: some View {
        let _ = UIPerf.body("Throttle")
        let level = model.drivingLevel
        let gear = live ?? model.cruiseGear
        let half = height / 2 - 6
        ZStack {
            Capsule(style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(Capsule(style: .continuous).strokeBorder(.white.opacity(0.14)))
            // Ticks at gears 1, 5, 10, 15, 20, 26 on both sides, the zero notch in the middle.
            ForEach([1, 5, 10, 15, 20, 26], id: \.self) { g in
                ForEach([-1, 1], id: \.self) { s in
                    Capsule().fill(.white.opacity(0.28))
                        .frame(width: g % 5 == 0 || g == 26 ? 14 : 8, height: 1)
                        .offset(y: -CGFloat(s) * Self.position(g, half: half))
                }
            }
            Capsule().fill(.white.opacity(0.7)).frame(width: 26, height: 2)
            // The gear set: a bar from zero (in: up, out: down).
            if gear != 0 {
                let h = Self.position(abs(gear), half: half)
                Capsule()
                    .fill(gear > 0 ? Color.white.opacity(0.85) : Color.orange.opacity(0.9))
                    .frame(width: 4, height: h)
                    .offset(y: gear > 0 ? -h / 2 : h / 2)
            }
            Image(systemName: "chevron.up").font(.caption2.weight(.bold)).foregroundStyle(.secondary)
                .offset(y: -height / 2 + 12)
            Image(systemName: "chevron.down").font(.caption2.weight(.bold)).foregroundStyle(.secondary)
                .offset(y: height / 2 - 12)
            // The knob: follows the finger, springs back; shows the gear.
            Text(Self.label(gear: gear, level: level))
                .font(.caption.weight(.semibold).monospacedDigit())
                .foregroundStyle(.black)
                .frame(width: 40, height: 30)
                .background(Capsule(style: .continuous).fill(.white.opacity(dragging ? 1 : 0.88)))
                .shadow(color: .black.opacity(0.35), radius: 3, y: 1)
                .offset(y: min(max(offset, -half + 15), half - 15))
        }
        .frame(width: Self.width, height: height)
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 2)
                .onChanged { v in
                    dragging = true
                    offset = v.translation.height
                    // One detent per `detent` points; back the other way steps back.
                    let want = Int((-v.translation.height / Self.detent).rounded(.towardZero))
                    while stepped < want { step(up: true); stepped += 1 }
                    while stepped > want { step(up: false); stepped -= 1 }
                }
                .onEnded { _ in
                    model.throttleEnd()
                    dragging = false
                    stepped = 0
                    withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) { offset = 0 }
                    // The model's gear takes over once the stats caught up.
                    Task {
                        try? await Task.sleep(for: .seconds(0.4))
                        if !dragging { live = nil }
                    }
                }
        )
        .onTapGesture(count: 2) {
            model.stopMotion()
            live = nil
            Haptics.zeroDetent()
        }
        .accessibilityElement()
        .accessibilityIdentifier("throttle")
        .accessibilityLabel("Throttle")
        .accessibilityValue(Self.spoken(gear: gear, level: level))
        .accessibilityHint("Swipe up or down to change gear; double tap stops")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: step(up: true)
            case .decrement: step(up: false)
            @unknown default: return
            }
            model.throttleEnd()
        }
    }

    private func step(up: Bool) {
        let before = live ?? model.cruiseGear
        guard let s = model.throttle(up: up) else { return }
        let g = Int(s.gear)
        if g != before {
            if g == 0 { Haptics.zeroDetent() } else { Haptics.gear() }
        }
        live = g
    }

    /// Height of gear `g` (1...26) above the zero notch: square-root spaced, so the low gears
    /// (the ones used most) get the room.
    static func position(_ g: Int, half: CGFloat) -> CGFloat {
        let reach = max(0, half - 20)
        return reach * CGFloat((Double(min(g, 26)) / 26).squareRoot())
    }

    static func label(gear: Int, level: DrivingLevel) -> String {
        if gear == 0 { return level == .manual ? "M" : "0" }
        return gear > 0 ? "\(gear)" : "R\(-gear)"
    }

    static func spoken(gear: Int, level: DrivingLevel) -> String {
        if gear == 0 { return level == .manual ? "manual" : "stopped" }
        return gear > 0 ? "gear \(gear)" : "reverse \(-gear)"
    }
}
#endif
