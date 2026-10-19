import AlmondbreadCore
import SwiftUI
#if os(iOS)
import UIKit
#endif

// The walkthrough and the UI keys of guided tours (kb: decisions/controls "Guided tours",
// "Walkthrough"). The walkthrough is a hidden tour (`crates/almondbread-core/tours/welcome.md`);
// what it shows is data there. Here: the platform a tour plays on, the ring around a UI element a
// step names (`highlight:`), the tour card kept clear of it, and the first-launch offer.

/// The platform a tour plays on (`platform:` and `::: mac` blocks in the tour file).
@MainActor
enum TourPlatform {
    static var current: String {
        #if os(macOS)
        "mac"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "pad" : "phone"
        #endif
    }
}

// MARK: Highlight

/// The pulsing gold ring around what a tour step points at (gold like the tour's marker on the
/// canvas). Takes no touches.
struct TourHighlightRing: View {
    let id: String
    var cornerRadius: CGFloat = 12
    @State private var pulse = false

    var body: some View {
        let gold = Color(red: 1.0, green: 0.78, blue: 0.25)
        ZStack {
            RoundedRectangle(cornerRadius: cornerRadius + 4, style: .continuous)
                .strokeBorder(.black.opacity(0.45), lineWidth: 6)
            RoundedRectangle(cornerRadius: cornerRadius + 4, style: .continuous)
                .strokeBorder(gold, lineWidth: 3)
        }
        .padding(-6)
        .scaleEffect(pulse ? 1.06 : 0.98)
        .opacity(pulse ? 0.6 : 1)
        .animation(.easeInOut(duration: 0.75).repeatForever(autoreverses: true), value: pulse)
        .onAppear { pulse = true }
        .allowsHitTesting(false)
        .accessibilityElement()
        .accessibilityIdentifier("tour-highlight")
        .accessibilityLabel("Look here")
        .accessibilityValue(id)
    }
}

/// The ring over an element while the running step names it; reports where it is, so the tour
/// card keeps clear of it. Reads only the change-only `model.tour`.
struct TourTargetRing: View {
    let id: String
    let model: ExplorerModel
    var cornerRadius: CGFloat = 12

    var body: some View {
        if model.tour?.highlight == id {
            TourHighlightRing(id: id, cornerRadius: cornerRadius)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { f in
                    if model.tourHighlightFrame != f { model.tourHighlightFrame = f }
                }
        }
    }
}

extension View {
    /// Marks a UI element a tour step can ring by `id` (`highlight: id`; the ids are listed in
    /// `tour::HIGHLIGHT_TARGETS` in the core and in kb: decisions/controls "Guided tours").
    func tourTarget(_ id: String, model: ExplorerModel, cornerRadius: CGFloat = 12) -> some View {
        overlay { TourTargetRing(id: id, model: model, cornerRadius: cornerRadius) }
    }
}

/// The cockpit during a tour (Rust draws it; its cells come from `Explorer.cockpitControls`, view
/// points from the canvas centre, y up, read twice a second): where it is, so the tour card keeps
/// clear of it, and the ring when a step names it (`cockpit`, or one control: `cockpit-autopilot`, …).
struct TourCockpitLayer: View {
    let model: ExplorerModel

    var body: some View {
        if let t = model.tour {
            let id = t.highlight == "cockpit" || t.highlight.hasPrefix("cockpit-") ? t.highlight : nil
            TimelineView(.periodic(from: .now, by: 0.5)) { context in
                // The canvas's own frame (it runs under the Mac's toolbar): the cells are
                // relative to its centre.
                GeometryReader { proxy in
                    let origin = proxy.frame(in: .global).origin
                    let whole = rect("cockpit", canvas: proxy.size)
                    let ring = id.flatMap { rect($0, canvas: proxy.size) }
                    ZStack {
                        if let id, let local = ring {
                            TourHighlightRing(id: id, cornerRadius: id == "cockpit" ? 14 : 10)
                                .frame(width: local.width, height: local.height)
                                .position(x: local.midX, y: local.midY)
                        }
                    }
                    .onAppear { report(whole, ring, origin) }
                    .onChange(of: [whole, ring, CGRect(origin: origin, size: .zero)]) { report(whole, ring, origin) }
                    // (Every tick too: the Rust panel lags its cells by a frame after a turn.)
                    .onChange(of: context.date) { report(whole, ring, origin) }
                }
                .ignoresSafeArea()
            }
            .onDisappear {
                model.tourCockpitFrame = nil
                model.tourMapsFrame = nil
            }
        }
    }

    private func report(_ whole: CGRect?, _ ring: CGRect?, _ origin: CGPoint) {
        // The panel as Rust laid it out (points from the canvas's top left); else around its
        // icons (it reaches beyond them: name, gauge, the (?) badge). The cells measured from
        // this layer's centre were 17 points low on a phone on its side (2026-10-19), so the
        // card overlapped the cockpit's top.
        let panels = model.explorer?.overlayPanels() ?? []
        func frame(_ id: String) -> CGRect? {
            panels.first { $0.id == id }.map {
                CGRect(x: model.canvasOrigin.x + $0.x, y: model.canvasOrigin.y + $0.y, width: $0.width, height: $0.height)
            }
        }
        let panel = frame("cockpit") ?? whole.map { $0.insetBy(dx: -40, dy: -8).offsetBy(dx: origin.x, dy: origin.y) }
        if model.tourCockpitFrame != panel { model.tourCockpitFrame = panel }
        // The mini-map row (the card keeps clear of it too).
        let maps = frame("maps")
        if model.tourMapsFrame != maps { model.tourMapsFrame = maps }
        if let ring {
            let r = ring.offsetBy(dx: origin.x, dy: origin.y)
            if model.tourHighlightFrame != r { model.tourHighlightFrame = r }
        }
    }

    /// The cell(s) in the canvas's coordinates.
    private func rect(_ id: String, canvas canvasSize: CGSize) -> CGRect? {
        guard let cells = model.explorer?.cockpitControls(), !cells.isEmpty else { return nil }
        let want = id == "cockpit" ? nil : String(id.dropFirst("cockpit-".count))
        let rects = cells.filter { want == nil || $0.id == want }.map {
            CGRect(x: canvasSize.width / 2 + $0.x - $0.halfWidth, y: canvasSize.height / 2 - $0.y - $0.halfHeight,
                   width: 2 * $0.halfWidth, height: 2 * $0.halfHeight)
        }
        guard var u = rects.first else { return nil }
        for r in rects.dropFirst() { u = u.union(r) }
        return u.insetBy(dx: -2, dy: -2)
    }
}

// MARK: The tour card's place

/// The tour card where it covers neither the picture's middle nor what the step rings: its usual
/// place (bottom trailing; on a narrow canvas the top), else the next one free of the ring.
struct TourCardLayer: View {
    let model: ExplorerModel
    let compact: Bool
    let trailing: CGFloat
    let bottom: CGFloat
    /// The platform's controls over the canvas (global frames: the throttle, the round
    /// buttons): the narrow canvas's card at the top keeps clear of them.
    var controls: [CGRect] = []
    @State private var cardHeight: CGFloat = 220

    var body: some View {
        if model.tour != nil {
            GeometryReader { proxy in
                let size = proxy.size
                let g = proxy.frame(in: .global)
                // A phone on its side: narrower and at the leading side, so a π panel (316 points
                // and its margins) fits beside it (kb: decisions/controls "π meter on narrow canvases").
                let wide = compact && size.width > size.height
                let fullWidth = !compact ? 340 : wide ? max(280, min(480, size.width - 24 - 340)) : min(max(0, size.width - 24), 480)
                // Clear of the ringed element and of the cockpit (a narrow Mac window, the
                // control center open).
                let ringed = model.tourHighlightFrame.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -10, dy: -10) }
                let cockpit = model.tourCockpitFrame.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -10, dy: -10) }
                let maps = model.tourMapsFrame.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -8, dy: -8) }
                let avoid = [ringed, cockpit, maps].compactMap { $0 }
                // Narrow canvas: the top card's place, width and height clear of the controls,
                // the cockpit and the maps below it (kb: decisions/controls "Tour card on a phone").
                let below = controls.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -8, dy: -8) }
                    + [cockpit, maps].compactMap { $0 }
                let top = compact ? Self.topFit(size: size, width: fullWidth, leading: wide, obstacles: below) : nil
                let w = top?.width ?? fullWidth
                // (At the top the card shrinks to its limit: test the spots at that height.)
                let h = top.map { min(cardHeight, $0.maxHeight) } ?? cardHeight
                let spots = Self.spots(size: size, card: CGSize(width: w, height: h), compact: compact,
                                       trailing: trailing, bottom: bottom, above: cockpit?.minY,
                                       topX: top?.x)
                let spot = spots.first { s in s.minY >= Self.margin - 0.5 && !avoid.contains { $0.intersects(s) } } ?? spots[0]
                // (A card moved off the top for a ring: no taller than from its bottom to the top.)
                let limit = top.map { spot.minY == Self.margin ? $0.maxHeight : spot.maxY - Self.margin }
                TourCard(model: model, compact: compact, maxHeight: limit)
                    .accessibilityElement(children: .contain)
                    .accessibilityIdentifier("tour-card")
                    // (UI tests: the height limit and the obstacles' tops it came from.)
                    .accessibilityValue(TestLaunch.uiTest
                        ? "limit \(Int(limit ?? -1)) below \(below.map { "\(Int($0.minX)),\(Int($0.minY))" })" : "")
                    .frame(width: w)
                    .fixedSize(horizontal: false, vertical: true)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { cardHeight = $0 }
                    // The card's own frame in the space the canvas reports its frame in: the π
                    // panel keeps clear of it.
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { model.tourCardFrame = $0 }
                    .position(x: spot.midX, y: spot.midY)
                    .animation(.snappy(duration: 0.3), value: spot)
            }
            .onDisappear { model.tourCardFrame = nil }
        }
    }

    /// The card's places in order of preference (frames in the layer).
    /// `above`: the cockpit's top (the layer's coordinates): the card may sit just above it.
    /// `topX`: the top card's left edge (`topFit`), else centred.
    static func spots(size: CGSize, card: CGSize, compact: Bool, trailing: CGFloat, bottom: CGFloat,
                      above: CGFloat? = nil, topX: CGFloat? = nil) -> [CGRect] {
        let (w, h) = (card.width, card.height)
        let top = CGRect(x: topX ?? (size.width - w) / 2, y: margin, width: w, height: h)
        let bottomCentre = CGRect(x: (size.width - w) / 2, y: size.height - bottom - h, width: w, height: h)
        if compact { return [top, bottomCentre] }
        let bottomTrailing = CGRect(x: size.width - trailing - w, y: size.height - bottom - h, width: w, height: h)
        let bottomLeading = CGRect(x: 12, y: size.height - bottom - h, width: w, height: h)
        var spots = [bottomTrailing]
        if let above {
            spots.append(CGRect(x: size.width - trailing - w, y: min(size.height - bottom, above - 2) - h,
                                width: w, height: h))
        }
        return spots + [bottomLeading, top]
    }

    /// The card's distance from the canvas edges (points).
    static let margin: CGFloat = 12

    /// A card at the top of a narrow canvas: its left edge, width and the height it may take
    /// before it reaches `obstacles` below it (the throttle and round buttons, the cockpit, the
    /// maps; layer coordinates, margins included). The places in order: its usual one (centred,
    /// or leading on a phone on its side), narrowed to the leading side of the trailing
    /// obstacles that limit it (the phone's button column with the throttle on top; at least
    /// 280 points wide), and at the trailing side left of the controls reaching up there (the
    /// throttle of a phone on its side, when the maps or the cockpit are under the leading
    /// side). The first that leaves the card `TourCard.fullHeight`, else the tallest; the card's
    /// text scrolls in what is left.
    static func topFit(size: CGSize, width: CGFloat, leading: Bool,
                       obstacles: [CGRect]) -> (x: CGFloat, width: CGFloat, maxHeight: CGFloat) {
        func room(_ x0: CGFloat, _ w: CGFloat) -> CGFloat {
            let under = obstacles.filter { $0.maxX > x0 && $0.minX < x0 + w && $0.maxY > margin }
            return (under.map(\.minY).min() ?? size.height - margin) - margin
        }
        let band = margin + TourCard.fullHeight
        let x0 = leading ? margin : (size.width - width) / 2
        var places: [(x: CGFloat, width: CGFloat)] = [(x0, width)]
        // Narrowed: left of the trailing obstacles that limit the usual place.
        let limiting = obstacles.filter { $0.maxX > x0 && $0.minX < x0 + width && $0.minY < band }
        if let edge = limiting.map(\.minX).min(), edge > size.width / 2 {
            let narrow = min(width, edge - 2 * margin)
            if narrow >= 280, narrow < width { places.append((margin, narrow)) }
        }
        // Trailing: left of the controls in the trailing half that reach up into the card's band.
        let edge = obstacles.filter { $0.minX > size.width / 2 && $0.minY < band }.map(\.minX).min() ?? size.width
        let trailingX = edge - margin - width
        if trailingX >= margin, abs(trailingX - x0) > 1 { places.append((trailingX, width)) }
        let rooms = places.map { (x: $0.x, width: $0.width, maxHeight: room($0.x, $0.width)) }
        return rooms.first { $0.maxHeight >= TourCard.fullHeight }
            ?? rooms.max { $0.maxHeight < $1.maxHeight } ?? rooms[0]
    }
}

// MARK: First-launch offer

/// The walkthrough offered once, at the first launch: "Take a 2-minute walkthrough" or "Just
/// explore" (`walkthrough.offered` in user defaults). Never in window tests, `--ui-demo` or the
/// UI tests unless `--walkthrough-offer` forces it; `--no-walkthrough-offer` suppresses it.
@MainActor @Observable
final class WalkthroughOffer {
    static let shared = WalkthroughOffer()
    static let offeredKey = "walkthrough.offered"
    static let forced = CommandLine.arguments.contains("--walkthrough-offer") || UIDemo.scene == "walkthrough-offer"
    static let suppressed = CommandLine.arguments.contains("--no-walkthrough-offer")

    var shown = !suppressed && ExplorerModel.walkthrough != nil
        && (forced || (!TestLaunch.ephemeral && !UserDefaults.standard.bool(forKey: offeredKey)))
    /// Bumped to start the walkthrough from where no model is at hand (the iOS help sheet).
    private(set) var startRequests = 0

    func answer() {
        withAnimation(.easeOut(duration: 0.25)) { shown = false }
        if !TestLaunch.ephemeral { UserDefaults.standard.set(true, forKey: Self.offeredKey) }
    }

    func requestStart() { startRequests += 1 }
}

/// The offer: a small card in the middle of the picture.
struct WalkthroughOfferCard: View {
    let model: ExplorerModel
    private var offer = WalkthroughOffer.shared

    init(model: ExplorerModel) {
        self.model = model
    }

    var body: some View {
        if offer.shown {
            VStack(spacing: 14) {
                Image(systemName: "sparkles")
                    .font(.largeTitle)
                    .foregroundStyle(Color(red: 1.0, green: 0.78, blue: 0.25))
                Text("Welcome to Almondbread")
                    .font(.title2.weight(.bold))
                    .multilineTextAlignment(.center)
                Text("New here? A short walkthrough shows you how to fly around.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                VStack(spacing: 10) {
                    Button {
                        start()
                    } label: {
                        Label("Take a 2-minute walkthrough", systemImage: "signpost.right")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .accessibilityIdentifier("walkthrough-start")
                    Button {
                        skip()
                    } label: {
                        Text("Just explore").frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .keyboardShortcut(.cancelAction)
                    .accessibilityIdentifier("walkthrough-skip")
                }
                .controlSize(.large)
                .padding(.top, 4)
            }
            .padding(24)
            .frame(maxWidth: 340)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(.white.opacity(0.14)))
            .shadow(color: .black.opacity(0.4), radius: 16, y: 4)
            .padding(24)
            .transition(.opacity.combined(with: .scale(scale: 0.96)))
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("walkthrough-offer")
        }
    }

    private func start() {
        offer.answer()
        #if os(iOS)
        // The walkthrough says what the first-run touch hints say.
        if !TestLaunch.ephemeral { UserDefaults.standard.set(true, forKey: TouchHints.seenKey) }
        #endif
        model.startTour(ExplorerModel.walkthroughID)
    }

    private func skip() {
        offer.answer()
        #if os(iOS)
        // The first-run touch hints, which waited for the offer.
        if TouchHints.shared.unseen { TouchHints.shared.show() }
        #endif
    }
}
