#if os(iOS)
import AlmondbreadCore
import SwiftUI
import UIKit

// Touch controls (kb: decisions/controls "Touch"): the throttle lever, the long-press actions
// and the haptics. Swift only decodes gestures; what a gear, a steering finger or a probe
// means is the Rust navigator's (`InputEvent::SetGear`, `TouchSteer`, `Probe`, `SeekAt`,
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
    /// The throttle's knob put on signed gear `gear` (0: the middle, stop): the position lever
    /// (Rust `SetGear`). Returns the cockpit's state right after it.
    @discardableResult
    func setGear(_ gear: Int) -> CruiseStatus? {
        send(.setGear(gear: Int32(max(-26, min(26, gear)))))
        return explorer?.cruiseStatus()
    }

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

/// The throttle (kb: decisions/controls "Touch"): a position lever at the trailing edge. The
/// knob's place is the gear: the middle (M) stops, up are the forward gears (zoom in), down the
/// reverse ones (zoom out). Each tick is one gear; a drag moves the knob from tick to tick (a
/// selection tick each, a firmer one at M) and it stays where it is left. A tap on a tick goes
/// to that gear, the chevrons at the ends one gear up / down, a double tap stops (Esc). When
/// fewer ticks fit than the 26 gears a side, they stand for gear 1 to 26 geometrically spaced
/// (Rust `throttle_ticks`) and the chevrons reach every gear between. The knob always shows
/// the navigator's gear (the cockpit's set speed), so it moves when keys, the cockpit, the
/// autopilot or a journey change the speed; in A and J the gear nearest their speed.
struct ThrottleLever: View {
    let model: ExplorerModel
    var height: CGFloat = 200

    static let width: CGFloat = TouchLayout.throttleWidth
    /// The chevrons' cells at both ends (points).
    static let cap: CGFloat = 26
    /// The knob (points).
    static let knob = CGSize(width: 44, height: 26)
    /// Closest tick spacing (points): below this fewer ticks stand for the gears.
    static let minPitch: CGFloat = 10

    /// The gear right after this lever set it (the model's change-only gear lags by ~0.1 s).
    @State private var live: Int?
    /// The tick under the finger while dragging (signed tick index: 0 the middle).
    @State private var dragTick: Int?

    /// The ticks on one side (their gears, from the middle out) and their spacing.
    struct Layout: Equatable {
        let ticks: [Int]
        let pitch: CGFloat

        @MainActor init(height: CGFloat) {
            let reach = max(ThrottleLever.minPitch, height / 2 - ThrottleLever.cap - ThrottleLever.knob.height / 2)
            let n = max(1, min(26, Int((reach / ThrottleLever.minPitch).rounded(.down))))
            ticks = Self.ticks(n)
            pitch = reach / CGFloat(n)
        }

        @MainActor private static var memo: [Int: [Int]] = [:]
        @MainActor private static func ticks(_ n: Int) -> [Int] {
            if let t = memo[n] { return t }
            let t = throttleTicks(perSide: UInt32(n)).map(Int.init)
            memo[n] = t
            return t
        }

        /// The gear of signed tick `k` (0: the middle).
        func gear(tick k: Int) -> Int {
            guard k != 0, !ticks.isEmpty else { return 0 }
            let g = ticks[min(abs(k), ticks.count) - 1]
            return k > 0 ? g : -g
        }

        /// The signed tick nearest a point `dy` above the middle (points).
        func tick(at dy: CGFloat) -> Int {
            let k = Int((dy / pitch).rounded())
            return max(-ticks.count, min(ticks.count, k))
        }

        /// Height above the middle of gear `g`: on its tick, or between the two ticks around it
        /// (log-spaced), for gears the chevrons or the keys set.
        func offset(gear g: Int) -> CGFloat {
            guard g != 0, let first = ticks.first else { return 0 }
            let a = Double(min(abs(g), 26))
            var pos = 1.0
            if a <= Double(first) {
                pos = a / Double(first)
            } else if let i = ticks.lastIndex(where: { Double($0) <= a }) {
                if i == ticks.count - 1 {
                    pos = Double(ticks.count)
                } else {
                    let lo = Double(ticks[i]), hi = Double(ticks[i + 1])
                    pos = Double(i + 1) + log(a / lo) / log(hi / lo)
                }
            }
            return CGFloat(pos) * pitch * (g > 0 ? 1 : -1)
        }
    }

    var body: some View {
        let _ = UIPerf.body("Throttle")
        let level = model.drivingLevel
        let gear = live ?? model.cruiseGear
        let layout = Layout(height: height)
        let knobY = -(dragTick.map { layout.offset(gear: layout.gear(tick: $0)) } ?? layout.offset(gear: gear))
        ZStack {
            Capsule(style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(Capsule(style: .continuous).strokeBorder(.white.opacity(0.14)))
            // One tick per gear position on both sides; the middle (M) a longer notch.
            ForEach(1...max(1, layout.ticks.count), id: \.self) { k in
                ForEach([-1, 1], id: \.self) { s in
                    Capsule().fill(.white.opacity(0.4))
                        .frame(width: 12, height: 1.5)
                        .offset(y: -CGFloat(s * k) * layout.pitch)
                }
            }
            Capsule().fill(.white.opacity(0.75)).frame(width: 28, height: 2.5)
            // The gear set: a bar from the middle to the knob (in: white, out: orange).
            if gear != 0 {
                let h = abs(layout.offset(gear: gear))
                Capsule()
                    .fill(gear > 0 ? Color.white.opacity(0.85) : Color.orange.opacity(0.9))
                    .frame(width: 4, height: h)
                    .offset(y: gear > 0 ? -h / 2 : h / 2)
            }
            chevron(up: true)
                .offset(y: -height / 2 + Self.cap / 2)
            chevron(up: false)
                .offset(y: height / 2 - Self.cap / 2)
            // The knob: on the gear's place; the gear in large type ("M", "+3", "−2").
            Text(Self.label(gear: gear))
                .font(.system(size: 17, weight: .bold, design: .rounded).monospacedDigit())
                .minimumScaleFactor(0.7)
                .lineLimit(1)
                .foregroundStyle(gear < 0 ? Color.white : Color.black)
                .frame(width: Self.knob.width, height: Self.knob.height)
                .background(Capsule(style: .continuous)
                    .fill(gear < 0 ? Color.orange : Color.white.opacity(dragTick != nil ? 1 : 0.92)))
                .shadow(color: .black.opacity(0.35), radius: 3, y: 1)
                .offset(y: knobY)
                .animation(dragTick == nil ? .spring(response: 0.25, dampingFraction: 0.8) : nil, value: knobY)
                .allowsHitTesting(false)
        }
        .frame(width: Self.width, height: height)
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 3, coordinateSpace: .local)
                .onChanged { v in
                    let k = layout.tick(at: height / 2 - v.location.y)
                    guard k != dragTick else { return }
                    dragTick = k
                    set(layout.gear(tick: k))
                }
                .onEnded { _ in
                    dragTick = nil
                    settleLive()
                }
                .exclusively(before: SpatialTapGesture(count: 2).onEnded { v in
                    // Two quick taps on a chevron are two gears, elsewhere a stop (Esc).
                    if let s = chevronStep(at: v.location.y) {
                        set(max(-26, min(26, gear + 2 * s)))
                    } else {
                        model.stopMotion()
                        live = 0
                        Haptics.zeroDetent()
                    }
                    settleLive()
                })
                .exclusively(before: SpatialTapGesture(count: 1).onEnded { v in
                    // A tap on a chevron: one gear on; on a tick: that gear.
                    if let s = chevronStep(at: v.location.y) {
                        set(max(-26, min(26, gear + s)))
                    } else {
                        set(layout.gear(tick: layout.tick(at: height / 2 - v.location.y)))
                    }
                    settleLive()
                })
        )
        .onChange(of: model.cruiseGear) { if live == model.cruiseGear && dragTick == nil { live = nil } }
        .accessibilityElement()
        .accessibilityIdentifier("throttle")
        .accessibilityLabel("Throttle")
        .accessibilityValue(Self.spoken(gear: gear, level: level) + Self.testInfo(layout))
        .accessibilityHint("Slide the knob to a gear: up flies in, down flies out, the middle stops; double tap stops")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: set(min(26, gear + 1))
            case .decrement: set(max(-26, gear - 1))
            @unknown default: return
            }
            settleLive()
        }
    }

    /// A chevron's cell: one gear up / down per tap (the lever's taps, by where they land).
    private func chevron(up: Bool) -> some View {
        Image(systemName: up ? "chevron.up" : "chevron.down")
            .font(.footnote.weight(.heavy))
            .foregroundStyle(.white.opacity(0.85))
            .frame(width: Self.width, height: Self.cap)
    }

    /// The chevron a tap at `y` (lever points from the top) is on: +1 up, −1 down, nil a tick.
    private func chevronStep(at y: CGFloat) -> Int? {
        if y < Self.cap { return 1 }
        if y > height - Self.cap { return -1 }
        return nil
    }

    /// Puts the level driving on gear `g` (Rust `SetGear`); the knob shows what Rust made of it
    /// (the autopilot's range stops at gear 20, home refuses reverse), with a haptic per change.
    private func set(_ g: Int) {
        let before = live ?? model.cruiseGear
        guard let s = model.setGear(g) else { return }
        let now = Int(s.gear)
        if now != before {
            if now == 0 { Haptics.zeroDetent() } else { Haptics.gear() }
        }
        live = now
    }

    /// The model's gear takes over once the stats caught up (or after 0.6 s, whatever it says).
    private func settleLive() {
        Task {
            try? await Task.sleep(for: .seconds(0.6))
            if dragTick == nil { live = nil }
        }
    }

    static func label(gear: Int) -> String {
        if gear == 0 { return "M" }
        return gear > 0 ? "+\(gear)" : "\u{2212}\(-gear)"
    }

    static func spoken(gear: Int, level: DrivingLevel) -> String {
        let what = gear == 0 ? "stopped" : gear > 0 ? "gear \(gear) in" : "gear \(-gear) out"
        switch level {
        case .autopilot: return "autopilot, " + what
        case .journey: return "journey, " + what
        default: return what
        }
    }

    /// UI tests (`ALMONDBREAD_UITEST=1`): the ticks' gears and spacing, so taps can aim.
    @MainActor static func testInfo(_ layout: Layout) -> String {
        guard TestLaunch.uiTest else { return "" }
        let pitch = String(format: "%.2f", Double(layout.pitch))
        return "; ticks \(layout.ticks.map(String.init).joined(separator: ",")) pitch \(pitch) cap \(Int(cap))"
    }
}
#endif
