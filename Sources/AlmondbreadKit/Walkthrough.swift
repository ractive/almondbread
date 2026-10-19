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
            TimelineView(.periodic(from: .now, by: 0.5)) { _ in
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
                }
                .ignoresSafeArea()
            }
            .onDisappear { model.tourCockpitFrame = nil }
        }
    }

    private func report(_ whole: CGRect?, _ ring: CGRect?, _ origin: CGPoint) {
        // The panel reaches beyond its icons (name, gauge, the (?) badge).
        let panel = whole.map { $0.insetBy(dx: -40, dy: -8).offsetBy(dx: origin.x, dy: origin.y) }
        if model.tourCockpitFrame != panel { model.tourCockpitFrame = panel }
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
    @State private var cardHeight: CGFloat = 220

    var body: some View {
        if model.tour != nil {
            GeometryReader { proxy in
                let size = proxy.size
                let g = proxy.frame(in: .global)
                let w = compact ? min(max(0, size.width - 24), 480) : 340
                // Clear of the ringed element and of the cockpit (a narrow Mac window, the
                // control center open).
                let ringed = model.tourHighlightFrame.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -10, dy: -10) }
                let cockpit = model.tourCockpitFrame.map { $0.offsetBy(dx: -g.minX, dy: -g.minY).insetBy(dx: -10, dy: -10) }
                let avoid = [ringed, cockpit].compactMap { $0 }
                let spots = Self.spots(size: size, card: CGSize(width: w, height: cardHeight), compact: compact,
                                       trailing: trailing, bottom: bottom, above: cockpit?.minY)
                let spot = spots.first { s in !avoid.contains { $0.intersects(s) } } ?? spots[0]
                TourCard(model: model, compact: compact)
                    .frame(width: w)
                    .fixedSize(horizontal: false, vertical: true)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { cardHeight = $0 }
                    .position(x: spot.midX, y: spot.midY)
                    .animation(.snappy(duration: 0.3), value: spot)
            }
        }
    }

    /// The card's places in order of preference (frames in the layer).
    /// `above`: the cockpit's top (the layer's coordinates): the card may sit just above it.
    static func spots(size: CGSize, card: CGSize, compact: Bool, trailing: CGFloat, bottom: CGFloat,
                      above: CGFloat? = nil) -> [CGRect] {
        let (w, h) = (card.width, card.height)
        let top = CGRect(x: (size.width - w) / 2, y: 12, width: w, height: h)
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
