import AlmondbreadCore
import SwiftUI

struct ContentView: View {
    // One model per window; the platform view creates the Rust explorer (it compiles the
    // kernels) once, when SwiftUI makes the view.
    @State private var model = ExplorerModel()
    @State private var showInspector = UIDemo.enabled && !UIDemo.hidesSidebar && !UIPerf.openScene
    @State private var showFlyTo = false
    @State private var fullScreenSidebar = FullScreenSidebar()
    /// Whether the control center showed before a tour step opened or closed it (it comes back
    /// so at the tour's end).
    @State private var inspectorBeforeTour: Bool?
    private var walkthroughOffer = WalkthroughOffer.shared
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.verticalSizeClass) private var verticalSizeClass

    /// Size of the canvas in points (narrower than the window while the iPad sidebar shows).
    @State private var canvasWidth: CGFloat = 1000
    @State private var canvasHeight: CGFloat = 800
    /// The touch controls' global frames ("throttle", "buttons"): the tour card keeps clear of them.
    @State private var controlFrames: [String: CGRect] = [:]

    /// Where the touch controls go (kb: decisions/controls "Touch", "Round buttons" ▸ Fit).
    private var touchLayout: TouchLayout {
        #if os(iOS)
        TouchLayout(canvas: CGSize(width: canvasWidth, height: canvasHeight),
                    horizontalCompact: sizeClass == .compact, verticalCompact: verticalSizeClass == .compact)
        #else
        TouchLayout(canvas: CGSize(width: canvasWidth, height: canvasHeight), horizontalCompact: false, verticalCompact: false)
        #endif
    }

    /// iPhone portrait, narrow iPad split views, and an iPad portrait canvas next to the
    /// control center: the HUD drops its statistics and the buttons move to the bottom.
    private var compactWidth: Bool {
        #if os(iOS)
        touchLayout.compactWidth
        #else
        false
        #endif
    }

    /// A phone on its side (kb: decisions/controls "Touch"): the buttons go in a row at the
    /// bottom trailing corner and the throttle stands at the trailing edge above them.
    private var landscapePhone: Bool {
        #if os(iOS)
        touchLayout.landscapePhone
        #else
        false
        #endif
    }

    /// The buttons stack in a column at the bottom trailing corner (phone portrait, narrow
    /// split views), with the throttle on top of them.
    private var buttonColumn: Bool { compactWidth && !landscapePhone }

    var body: some View {
        explorer
            .overlay {
                if let failure = model.failure {
                    ContentUnavailableView(
                        "Metal Unavailable",
                        systemImage: "exclamationmark.triangle",
                        description: Text("Almondbread needs a Metal GPU. The renderer could not be created: \(failure)"))
                    .background(Color.black)
                }
            }
    }

    @ViewBuilder
    private var explorer: some View {
        FractalCanvas(model: model)
        // Popovers of the help badges Rust draws on its panels (kb: decisions/controls "Help").
        // Inside `ignoresSafeArea`: the badge rects are in the canvas's full frame (under the
        // toolbar), and an anchor in the safe-area frame sat a toolbar lower, below the window
        // for the cockpit's badge at the bottom edge (no popover at all).
        .modifier(CanvasHelpPopover(model: model))
        #if os(iOS)
        .modifier(CanvasMenuPopover(model: model))
        #endif
        .ignoresSafeArea()
        .onGeometryChange(for: CGSize.self) { $0.size } action: {
            canvasWidth = $0.width
            canvasHeight = $0.height
        }
        // The canvas's origin in global coordinates, measured outside the safe area as the
        // canvas lies (under the Mac's toolbar): the tour card's frame is converted with it.
        .overlay {
            Color.clear
                .onGeometryChange(for: CGPoint.self) { $0.frame(in: .global).origin } action: { model.canvasOrigin = $0 }
                .ignoresSafeArea()
                .allowsHitTesting(false)
        }
        .background(Color.black)
        .overlay(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 8) {
                // On a narrow canvas a tour's card takes the HUD's place.
                if model.showHUD && !(compactWidth && model.tour != nil) {
                    HUDView(model: model, compact: compactWidth)
                        .accessibilityElement(children: .contain)
                        .accessibilityIdentifier("hud")
                        .tourTarget("hud", model: model, cornerRadius: 10)
                }
                #if os(iOS)
                // At a journey stop: its three choices, under the HUD line that names the
                // landmark (the Mac's J / Z / X; kb: decisions/controls "Touch").
                if model.journeyAtStop {
                    JourneyStopButtons(model: model)
                }
                #endif
            }
            .padding(12)
        }
        .onChange(of: model.showHUD) { if !model.showHUD { model.hudFrame = nil } }
        // A guided tour's card (kb: decisions/controls "Guided tours"): bottom trailing, clear of the
        // HUD (top left), the maps (bottom left) and the cockpit (bottom centre). On a narrow canvas
        // (iPhone portrait) at the top in place of the HUD, its text folding and scrolling: at the
        // bottom it covered the maps, the cockpit and part of the picture (kb: "Touch").
        // A step that rings something moves the card out of its way (kb: "Guided tours").
        .overlay {
            TourCardLayer(model: model, compact: compactWidth,
                          trailing: compactWidth ? 12 : Self.tourTrailing(compact: false),
                          bottom: Self.tourBottom(compact: compactWidth, cockpit: model.showHUD),
                          controls: Array(controlFrames.values))
        }
        // The ring around the cockpit (Rust draws it) when a tour step points at it.
        .overlay {
            TourCockpitLayer(model: model)
        }
        // The first-launch walkthrough offer (kb: decisions/controls "Walkthrough").
        .overlay {
            if model.failure == nil {
                WalkthroughOfferCard(model: model)
            }
        }
        .onChange(of: model.tour) { old, new in tourControlCenter(old: old, new: new) }
        .onChange(of: walkthroughOffer.startRequests) { model.startTour(ExplorerModel.walkthroughID) }
        #if os(macOS)
        // The canvas runs on under the control center: the π panel, the cosmic scale's right
        // edge and the cockpit keep left of it while it shows (it reports a width while hidden
        // too: the π panel sat 330 points left of its corner during the tours).
        .onChange(of: showInspector ? model.controlCenterWidth : 0, initial: true) { _, width in
            model.piMeterInsetRight = width
            model.cosmicInsetRight = width
            model.cockpitInsetRight = width
        }
        #endif
        // (The π panel keeps clear of the tour card by itself: the card layer reports its
        // frame, `model.tourCardFrame`; kb: decisions/controls "π meter on narrow canvases".)
        #if os(iOS)
        // The Rust panels keep clear of the touch controls (kb: decisions/controls "Touch").
        .onChange(of: touchLayout, initial: true) {
            let l = touchLayout
            // Left of the button column, or of the throttle at the trailing edge; above the
            // button row of a phone on its side; below the iPad's top-right buttons.
            model.piMeterInsetRight = l.trailingInset
            model.piMeterInsetBottom = l.bottomInset
            model.piMeterInsetTop = l.topInset
            model.cosmicInsetRight = l.trailingInset
            // iPad: the compass sits below the top-right buttons (44 + 12 pt).
            model.compassInsetTop = l.topInset
            // The cockpit (bottom centre) keeps left of the button column, or of the button row
            // of a phone on its side (five buttons).
            model.cockpitInsetRight = l.cockpitInsetRight
        }
        // The throttle (kb: decisions/controls "Touch") at the trailing edge, in thumb reach:
        // on top of the button column on a phone in portrait (below), else at mid height.
        .overlay(alignment: .trailing) {
            if model.showHUD && !buttonColumn {
                // A phone on its side: short enough to clear the compass at the top trailing
                // corner (they overlapped by a few points; QA 2026-10-18).
                // iPad: taller (360), so more of the position lever's gears get a tick.
                ThrottleLever(model: model, height: min(landscapePhone ? 240 : 360,
                                                        max(120, canvasHeight - (landscapePhone ? 200 : 260))))
                    .touchHintAnchor("throttle")
                    .tourTarget("throttle", model: model, cornerRadius: 22)
                    .reportFrame("throttle", into: $controlFrames)
                    .padding(.trailing, 12)
                    .padding(.bottom, landscapePhone ? 56 : 0)
            }
        }
        #endif
        #if os(iOS)
        // iPhone portrait: the buttons sit at the bottom, in thumb reach and clear of the HUD;
        // wider screens: top trailing, next to the HUD.
        // A phone on its side: a row at the bottom trailing corner, whatever its width class.
        .overlay(alignment: touchLayout.buttonCorner == .bottomTrailing ? .bottomTrailing : .topTrailing) {
            let stack = buttonColumn ? AnyLayout(VStackLayout(spacing: 10)) : AnyLayout(HStackLayout(spacing: 10))
            stack {
                if buttonColumn && model.showHUD {
                    // Five buttons below it (two more than before 2026-10-18: 108 points).
                    ThrottleLever(model: model, height: min(220, max(120, canvasHeight - 528)))
                        .touchHintAnchor("throttle")
                        .tourTarget("throttle", model: model, cornerRadius: 22)
                        .reportFrame("throttle", into: $controlFrames)
                        .padding(.bottom, 6)
                }
                // The Mac toolbar's items (kb: decisions/controls "Touch", "Round buttons"): the
                // overlays and Fly To. The autopilot and journeys are the cockpit's icons.
                // (The tour rings: `highlight: toolbar` all five, or one by its name.)
                stack {
                    RoundToggle("Orbit", symbol: InlineIcons.symbol("orbit"),
                                isOn: $model.orbitEnabled)
                        .tourTarget("orbit", model: model, cornerRadius: 22)
                    RoundToggle("Mini-map", symbol: InlineIcons.symbol("mini-map"), isOn: $model.mapEnabled)
                        .tourTarget("mini-map", model: model, cornerRadius: 22)
                    // The Julia panel: only in the iPad's top row (the phone's column and row
                    // have no room for a sixth button; kb: decisions/controls "Julia panel"), and
                    // not in Multibrot views (`hasJuliaPanelSwitch`: Rust's capabilities; a Julia
                    // view keeps it).
                    if touchLayout.buttonCorner == .topTrailing && model.hasJuliaPanelSwitch {
                        RoundToggle("Julia set", symbol: InlineIcons.symbol("julia"), isOn: $model.juliaPanelToggle)
                            .tourTarget("julia", model: model, cornerRadius: 22)
                    }
                    RoundToggle("Size scale", symbol: InlineIcons.symbol("size-scale"), isOn: $model.cosmicScale)
                        .tourTarget("size-scale", model: model, cornerRadius: 22)
                    RoundButton("Fly to", symbol: InlineIcons.symbol("fly-to")) { showFlyTo = true }
                        .tourTarget("fly-to", model: model, cornerRadius: 22)
                    RoundButton("Control center", symbol: InlineIcons.symbol("control-center"), isSelected: showInspector) {
                        showInspector.toggle()
                    }
                    .accessibilityValue(showInspector ? "shown" : "hidden")
                    .tourTarget("control-center", model: model, cornerRadius: 22)
                }
                .tourTarget("toolbar", model: model, cornerRadius: 26)
                .reportFrame("buttons", into: $controlFrames)
            }
            .padding(12)
        }
        // First-run touch hints over everything above, pointing at the throttle and the
        // cockpit's plane.
        .touchHints(cockpitPlane: cockpitPlane)
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        #endif
        // A trailing sidebar on the Mac and iPad (the canvas narrows: nothing covers the image),
        // a sheet on iPhone.
        .inspector(isPresented: inspectorPresented) {
            ControlCenterView(model: model, close: closeControlCenter)
                .inspectorColumnWidth(min: ControlCenterView.minimumWidth, ideal: 330, max: 440)
                #if os(iOS)
                .presentationDetents(sizeClass == .compact ? [.medium, .large] : [.large])
                .presentationBackground(.regularMaterial)
                // During a tour the phone's half-height sheet leaves the tour card (at the top)
                // usable (kb: decisions/controls "Walkthrough").
                .presentationBackgroundInteraction(model.tour != nil ? .enabled(upThrough: .medium) : .automatic)
                #endif
        }
        .sheet(isPresented: $showFlyTo) {
            FlyToSheet(model: model, isPresented: $showFlyTo)
        }
        .onReceive(UIPerf.actions) { action in
            if case .inspector(let shown) = action { showInspector = shown }
        }
        #if os(iOS)
        .modifier(HelpSheet())
        #endif
        #if os(macOS)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Toggle(isOn: $model.orbitEnabled) {
                    Label("Orbit", systemImage: InlineIcons.symbol("orbit"))
                        .tourTarget("orbit", model: model, cornerRadius: 6)
                        .tourTarget("toolbar", model: model, cornerRadius: 6)
                }
                .help("Orbit of the point under the pointer (O)")
                Toggle(isOn: $model.mapEnabled) {
                    Label("Mini-map", systemImage: InlineIcons.symbol("mini-map"))
                        .tourTarget("mini-map", model: model, cornerRadius: 6)
                        .tourTarget("toolbar", model: model, cornerRadius: 6)
                }
                .help("Mini-map with nested levels; click it to jump there (M)")
                // (Not in Multibrot views, `hasJuliaPanelSwitch`: Rust's capabilities; a Julia
                // view keeps it; kb: decisions/controls "Fractal picker and Julia views".)
                if model.hasJuliaPanelSwitch {
                    Toggle(isOn: $model.juliaPanelToggle) {
                        Label("Julia Set", systemImage: InlineIcons.symbol("julia"))
                            .tourTarget("julia", model: model, cornerRadius: 6)
                            .tourTarget("toolbar", model: model, cornerRadius: 6)
                    }
                    .help("The Julia set of the point under the pointer, next to the mini-map (U)")
                }
                // The π meter is not in the toolbar: the tour "π in the Mandelbrot set" drives it
                // (user, 2026-10-18).
                Toggle(isOn: $model.cosmicScale) {
                    Label("Size Scale", systemImage: InlineIcons.symbol("size-scale"))
                        .tourTarget("size-scale", model: model, cornerRadius: 6)
                        .tourTarget("toolbar", model: model, cornerRadius: 6)
                }
                .help("Cosmic size scale: how big the whole set would be if your window stayed its real size (C)")
                // Autopilot and journey: the cockpit's icons (clickable) and Z / J, not here.
                Button {
                    showFlyTo = true
                } label: {
                    Label("Fly To", systemImage: InlineIcons.symbol("fly-to"))
                        .tourTarget("fly-to", model: model, cornerRadius: 6)
                        .tourTarget("toolbar", model: model, cornerRadius: 6)
                }
                .help("Fly to coordinates (⌘G); ⇧⌘G flies to copied ones")
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showInspector.toggle()
                } label: {
                    Label("Control Center", systemImage: InlineIcons.symbol("control-center"))
                        .tourTarget("control-center", model: model, cornerRadius: 6)
                        .tourTarget("toolbar", model: model, cornerRadius: 6)
                }
                .help("Show or hide the control center (⌘I)")
            }
        }
        .navigationTitle("Almondbread")
        // Full screen (kb: decisions/controls "Full screen"): a clean view. The control center
        // closes and comes back as it was; the toolbar hides and slides in when the pointer
        // goes to the top edge (macOS 15+; macOS 14 keeps AppKit's visible toolbar).
        .onChange(of: model.fullScreen) {
            fullScreenSidebar.update(fullScreen: model.fullScreen, shown: &showInspector)
        }
        .modifier(FullScreenToolbarOnHover())
        #endif
        .focusedSceneValue(\.explorerModel, model)
        .focusedSceneValue(\.inspectorVisible, $showInspector)
        .focusedSceneValue(\.flyToVisible, $showFlyTo)
    }
}

extension ContentView {
    /// A tour step's `control-center: open | closed`; at the tour's end the control center is
    /// back as it was before the first step that changed it.
    func tourControlCenter(old: TourStepInfo?, new: TourStepInfo?) {
        guard let new else {
            if old != nil, let before = inspectorBeforeTour { showInspector = before }
            inspectorBeforeTour = nil
            return
        }
        if old?.tourId != new.tourId { inspectorBeforeTour = nil }
        guard old?.index != new.index || old?.tourId != new.tourId, !new.controlCenter.isEmpty else { return }
        if inspectorBeforeTour == nil { inspectorBeforeTour = showInspector }
        showInspector = new.controlCenter == "open"
    }

    /// The control center's presentation. iOS restores an inspector that was open when the app
    /// was last ended (scene state restoration writes `true` into this binding ~15 ms after the
    /// first body, and keeps doing so even after the pane was closed); the pane then sometimes
    /// never came up: the round button read "shown", lit, with no pane, and its first tap did
    /// nothing visible. The control center starts closed and only the app opens it (round
    /// button, tours, UIPerf all set `showInspector` directly), so on iOS the binding takes only
    /// `false` (the phone sheet's swipe down); a refused restore collapses the pane again
    /// (kb: platform/ios-simulator "Restored control center").
    var inspectorPresented: Binding<Bool> {
        #if os(iOS)
        Binding(get: { showInspector }, set: { shown in if !shown { showInspector = false } })
        #else
        $showInspector
        #endif
    }

    /// The control center's ✕ (iPhone and iPad); the Mac closes it from the toolbar and ⌘I.
    var closeControlCenter: (() -> Void)? {
        #if os(iOS)
        { showInspector = false }
        #else
        nil
        #endif
    }

    #if os(iOS)
    /// The cockpit's plane icon (Rust draws it; view points from the canvas centre, y up) in
    /// global coordinates: the canvas fills the window from its top-left corner.
    func cockpitPlane() -> CGRect? {
        guard model.showHUD,
              let c = model.explorer?.cockpitControls().first(where: { $0.id == "autopilot" }) else { return nil }
        return CGRect(x: canvasWidth / 2 + c.x - c.halfWidth, y: canvasHeight / 2 - c.y - c.halfHeight,
                      width: 2 * c.halfWidth, height: 2 * c.halfHeight)
    }
    #endif

    /// The tour card's trailing margin: clear of the button column (compact) or the throttle.
    static func tourTrailing(compact: Bool) -> CGFloat {
        #if os(iOS)
        compact ? 68 : ThrottleLever.width + 24
        #else
        12
        #endif
    }

    /// The tour card's bottom margin: on iOS above the touch cockpit (44 points, two rows of 66
    /// on a narrow canvas), whose icons it would cover.
    static func tourBottom(compact: Bool, cockpit: Bool) -> CGFloat {
        #if os(iOS)
        cockpit ? (compact ? 66 : 44) + 24 : 12
        #else
        12
        #endif
    }
}

extension View {
    /// Keeps `frames[id]` at this view's global frame while it shows (the tour card's obstacles).
    func reportFrame(_ id: String, into frames: Binding<[String: CGRect]>) -> some View {
        onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frames.wrappedValue[id] = $0 }
            .onDisappear { frames.wrappedValue[id] = nil }
    }
}

/// The control center across full screen: closed on entering, back to how it was on leaving
/// (opened or closed in full screen does not count).
struct FullScreenSidebar: Equatable {
    private(set) var saved: Bool?

    mutating func update(fullScreen: Bool, shown: inout Bool) {
        if fullScreen {
            if saved == nil { saved = shown }
            shown = false
        } else if let s = saved {
            shown = s
            saved = nil
        }
    }
}

#if os(macOS)
/// Full screen hides the toolbar until the pointer reaches the top edge (the standard
/// auto-hide of Safari and Xcode). Without it AppKit keeps the toolbar showing.
struct FullScreenToolbarOnHover: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content.windowToolbarFullScreenVisibility(.onHover)
        } else {
            content
        }
    }
}
#endif

/// Translucent status overlay. Does not take input, so gestures reach the canvas below.
struct HUDView: View {
    let model: ExplorerModel
    /// Narrow screens: no renderer statistics, lines wrap at a fixed width.
    var compact = false

    /// Long lines (journey, hints) wrap instead of running under the iOS buttons at the top
    /// trailing corner (iPad portrait).
    private var maxWidth: CGFloat? {
        #if os(iOS)
        compact ? 230 : 380
        #else
        nil
        #endif
    }
    @State private var speedHighlight = false

    var body: some View {
        let _ = UIPerf.body("HUD")
        if let s = model.stats {
            content(s)
        }
    }

    private func content(_ s: ExplorerStats) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Image(systemName: "plus.magnifyingglass")
                Text(Format.zoom(log10: s.shown.zoomLog10)).fontWeight(.semibold)
                if s.deep {
                    Text("deep")
                        .font(.caption2.weight(.semibold))
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(.purple.opacity(0.35), in: Capsule())
                }
                if s.isPreview {
                    Text("preview")
                        .font(.caption2.weight(.semibold))
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(.orange.opacity(0.35), in: Capsule())
                }
            }
            .font(.callout)
            Group {
                // The centre (the control center shows no live values; ⇧⌘C copies it in full).
                Group {
                    Text("re " + Self.digits(s.shown.re, compact ? 22 : 34))
                    Text("im " + Self.digits(s.shown.im, compact ? 22 : 34))
                }
                .font(.system(.caption, design: .monospaced))
                .lineLimit(1)
                if s.shown.angleDegrees != 0 {
                    Text(String(format: "turned %.1f°", s.shown.angleDegrees))
                }
                // F4: a Julia set is the main view: its k, and the way back.
                if !s.juliaK.isEmpty {
                    HStack(spacing: 6) {
                        Label("Julia set, k = " + s.juliaK, systemImage: InlineIcons.symbol("julia"))
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.primary)
                            .lineLimit(2)
                        Button {
                            model.backFromJulia()
                        } label: {
                            Image(systemName: "arrow.uturn.backward.circle")
                        }
                        .buttonStyle(.plain)
                        .help("Back to the Mandelbrot set (⇧U)")
                        .accessibilityLabel("Back to the Mandelbrot set")
                        .accessibilityIdentifier("julia-back-hud")
                    }
                }
                Text(speedLine(s))
                    .padding(.horizontal, speedHighlight ? 5 : 0)
                    .background(speedHighlight ? Color.orange.opacity(0.35) : .clear, in: Capsule())
                    .foregroundStyle(speedHighlight ? .primary : .secondary)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            if let journey = journeyLine(s) {
                Label(journey, systemImage: "sparkles")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.primary)
            }
            let hint = motionHint(s)
            if let hint {
                Label(hint, systemImage: s.cruise.level == .cruise ? "gauge.with.dots.needle.33percent"
                      : s.flight.active ? "location.north.line" : "airplane")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.primary)
            }
            // "Minibrot, period 34" (seek, a journey's stop or leg, the autopilot's target): the
            // tour on periods is one click away (kb: decisions/controls "Finding the tours").
            if ExplorerModel.tour(forTopic: "period") != nil,
               [journeyLine(s), hint].contains(where: { $0?.contains("period ") == true }) {
                TourTopicLink(model: model, topic: "period", label: "What's a period?")
            }
            HUDProgressLine(model: model)
            if model.hudDetails {
                HUDDeveloperDetails(stats: s, autoIterations: model.autoIterations)
            }
            if s.shown.atPrecisionLimit {
                Label("Depth limit: zoom stops here (10^3000; reference orbits need 10 000 bits).",
                      systemImage: "exclamationmark.triangle.fill")
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(.black)
                    .padding(6)
                    .background(.yellow, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: 260, alignment: .leading)
            }
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: maxWidth, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        // Hit-testable for the progress line's "Continue" button; the canvas already treats the
        // HUD as off-canvas (arrow cursor, no edge pan), so a click there is not lost for zooms.
        // The canvas shows the arrow cursor over the HUD and treats it as off-canvas.
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { model.hudFrame = $0 }
        // `.contain`, not `.combine`: the Continue button and the tour links stay buttons.
        .accessibilityElement(children: .contain)
        .onChange(of: model.zoomSpeedChanged) {
            speedHighlight = true
            Task {
                try? await Task.sleep(for: .seconds(1.5))
                if Date().timeIntervalSince(model.zoomSpeedChanged) >= 1.4 { speedHighlight = false }
            }
        }
    }
}

/// The render progress (kb: decisions/controls "Render progress"): stage, steps, pixels left
/// and ETA while a settled view takes long; Rust draws the line at the top of the canvas. A
/// leaf reading only the change-only `progressText`.
struct HUDProgressLine: View {
    let model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("HUDProgress")
        let text = model.progressText
        if !text.isEmpty {
            HStack(spacing: 8) {
                Label(text, systemImage: "hourglass")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                // At the time limit: one more round of compute further (kb: techniques/
                // auto-iterations "Compute further").
                if model.canContinue {
                    Button("Continue") { model.continueComputing() }
                        .font(.caption)
                        #if os(iOS)
                        // A finger-sized target (kb: decisions/controls "Touch").
                        .buttonStyle(.bordered)
                        .controlSize(.regular)
                        #else
                        .controlSize(.small)
                        #endif
                        .help("Compute another minute on this view (G)")
                }
            }
            // Settled with fog left: the tour that explains it (topic `fog`), if bundled.
            if model.progressUnfinished, ExplorerModel.tour(forTopic: "fog") != nil {
                TourTopicLink(model: model, topic: "fog", label: "What's the fog?")
            }
        }
    }
}

/// The HUD's developer details (Overlays ▸ "Developer details in the HUD", View menu): what
/// the control center's "Performance & diagnostics" section showed.
struct HUDDeveloperDetails: View {
    let stats: ExplorerStats
    let autoIterations: Bool

    var body: some View {
        let s = stats
        VStack(alignment: .leading, spacing: 2) {
            Text("iterations \(s.maxIterations.formatted())\(autoIterations ? " (auto)" : "")")
            Text("GPU \(Format.ms(s.gpuTime))  ·  \(s.chunks) chunks")
            Text("render \(s.renderWidth)×\(s.renderHeight)\(s.isPreview ? " preview" : "")")
            Text("ref \(s.referenceLength.formatted()) pts  ·  \(s.referenceBits) bit\(s.referencePeriod > 0 ? "  ·  period \(s.referencePeriod)" : "")")
            Text("ref CPU \(Format.ms(s.referenceTime))\(s.referencePending ? "  ·  computing" : "")")
            Text("kernel \(s.deep ? "deep (extended range)" : "plain float32")  ·  \(s.settled ? "settled" : (s.isRendering ? "refining" : "idle"))")
            if s.failedFrames > 0 {
                Text("failed frames \(s.failedFrames)").foregroundStyle(.red)
            }
        }
        // Set off from the lines above by space (a `Divider` would stretch the HUD's width).
        .padding(.top, 4)
        .font(.caption)
        .foregroundStyle(.secondary)
    }
}

extension HUDView {
    /// The speed Q / E change right now: the flight's, the autopilot's or Space's.
    func speedLine(_ s: ExplorerStats) -> String {
        if s.journey.active && !s.journey.suspended && !s.journey.atStop {
            return "journey speed \(Format.speed(exp(s.journey.speed)))"
        }
        if s.cruise.level == .cruise {
            return "cruise \(Format.cruise(s.cruise.set))"
        }
        if s.flight.active {
            return "fly speed \(Format.speed(exp(s.flight.speed)))"
        }
        if s.autopilot.active {
            return "autopilot \(Format.speed(exp(s.autopilot.speed)))  ·  now \(Format.speed(exp(max(0, s.autopilot.rate))))"
        }
        return "zoom speed \(Format.speed(model.zoomFactorPerSecond))"
    }

    /// Autopilot / flight state and why it is slower than set (kb: decisions/controls).
    /// "Journey #4711 · leg 3 · spiral M(36,2) · 1.2e8× · resting" (kb: decisions/controls).
    func journeyLine(_ s: ExplorerStats) -> String? {
        let j = s.journey
        guard j.active else { return nil }
        let name = j.daily ? "Daily journey #\(j.seed)" : "Journey #\(j.seed)"
        if j.suspended { return "\(name) paused  ·  \(Format.resumeJourney)" }
        // At a stop (kb: decisions/controls "Journey with stops"): the landmark and the choices.
        if j.atStop {
            let here = [j.title, Format.zoom(log10: s.shown.zoomLog10)].filter { !$0.isEmpty }.joined(separator: "  ·  ")
            if j.phase == "planning" { return here + "  ·  planning" }
            #if os(macOS)
            return here + "  —  J next  ·  Z deeper here  ·  X somewhere else"
            #else
            // The choices are buttons right under the HUD (JourneyStopButtons).
            return here
            #endif
        }
        var parts = [name, "leg \(j.leg)"]
        if !j.note.isEmpty { parts.append(j.note) }
        if !j.label.isEmpty { parts.append(j.label) }
        if j.leg > 0 { parts.append(Format.zoom(log10: j.targetZoomLog10)) }
        parts.append(j.phase)
        return parts.joined(separator: "  ·  ")
    }

    func motionHint(_ s: ExplorerStats) -> String? {
        if let seek = Self.seekLine(s) { return seek }
        if s.journey.active {
            // The journey line says it all; only a back-off is worth a second line.
            return s.flight.active && !s.journey.suspended ? Self.hintText(s.flight.hint) : nil
        }
        // Cruise (kb: decisions/controls "Driving levels"): its landing on the whole set, lane
        // assist steering away from the featureless, or nothing (the cockpit shows the rest).
        if s.cruise.level == .cruise {
            if s.cruise.landing {
                return s.flight.stopping ? "Cruise  ·  stopping" : "Cruise  ·  landing on the whole set"
            }
            if s.cruise.nudging { return "The Nothing ahead – steering away" }
            return s.cruise.set == 0 && abs(s.cruise.rate) > 0.02 ? "Cruise  ·  stopping" : nil
        }
        if s.flight.active {
            let to = "Flying to \(Format.zoom(log10: s.flight.targetZoomLog10))  ·  \(String(format: "%.0f", s.flight.progress * 100)) %"
            if s.flight.stopping { return to + "  ·  stopping" }
            return [to, Self.hintText(s.flight.hint)].compactMap { $0 }.joined(separator: "  ·  ")
        }
        guard s.autopilot.active else { return nil }
        if s.autopilot.paused { return "Autopilot paused  ·  \(Format.resumeAutopilot)" }
        let lock = s.autopilot.lockLabel.isEmpty ? nil : s.autopilot.lockLabel
        return ["Autopilot", lock, Self.hintText(s.autopilot.hint)].compactMap { $0 }.joined(separator: "  ·  ")
    }

    /// Seek minibrot (N; kb: decisions/controls "Seek minibrot"): searching, the minibrot it
    /// flies to or landed on ("Minibrot, period 4640 · 1.0e69×"), or that none was found.
    static func seekLine(_ s: ExplorerStats) -> String? {
        let k = s.seek
        switch k.phase {
        case "searching": return "Seeking a minibrot…"
        case "not found": return "No minibrot found nearby"
        case "flying", "arrived":
            let name = "Minibrot, period \(k.period)  ·  \(Format.zoom(log10: k.targetZoomLog10))"
            guard k.phase == "flying", s.flight.active else { return name }
            if s.flight.stopping { return name + "  ·  stopping" }
            let to = name + "  ·  \(String(format: "%.0f", s.flight.progress * 100)) %"
            return [to, hintText(s.flight.hint)].compactMap { $0 }.joined(separator: "  ·  ")
        default: return nil
        }
    }

    /// A coordinate cut to `n` characters (deep views have hundreds of digits; ⇧⌘C copies all).
    static func digits(_ x: String, _ n: Int) -> String {
        x.count <= n ? x : x.prefix(n - 1) + "…"
    }

    static func hintText(_ h: AutopilotHint) -> String? {
        switch h {
        case .none: nil
        case .rendering: "slower: the renderer is behind"
        case .reference: "waiting for the deep reference"
        // The one playful wink (Michael Ende's The NeverEnding Story): a uniform region ahead,
        // fully interior or featureless exterior.
        case .interior: "The Nothing ahead – slowing down"
        case .featureless: "The Nothing ahead – slowing down"
        case .limit: "depth limit"
        case .rescue: "The Nothing ahead – turning back"
        }
    }
}

enum Format {
    /// How a paused autopilot or journey resumes: a click (or J) on the Mac, a tap on touch
    /// (a child read "click" on the iPad and had no mouse; QA 2026-10-18).
    #if os(iOS)
    static let resumeJourney = "tap the picture to go on"
    static let resumeAutopilot = "tap the picture to go on"
    #else
    static let resumeJourney = "J or click to resume"
    static let resumeAutopilot = "click to resume"
    #endif

    /// "850×", "12.5k×" style up to 1e4, then "1.2e9×" (any depth: from log10 of the zoom).
    static func zoom(log10 l: Double) -> String {
        if Preferences.zoomUnits == .powerOfTen { return String(format: "10^%.1f", l) }
        if l < 1 { return String(format: "%.2f×", pow(10, l)) }
        if l < 4 { return String(format: "%.0f×", pow(10, l)) }
        var e = floor(l)
        var m = pow(10, l - e)
        if m >= 9.95 { m /= 10; e += 1 }
        return String(format: "%.1fe%.0f×", m, e)
    }

    /// Zoom speed as magnification per second: "×4.95/s".
    static func speed(_ factor: Double) -> String {
        factor < 10 ? String(format: "×%.2f/s", factor) : String(format: "×%.0f/s", factor)
    }

    /// A signed cruise speed (e-folds per second): "↓ ×2.00/s" out, "↑ ×1.50/s" in, "stopped".
    static func cruise(_ efolds: Double) -> String {
        if abs(efolds) < 1e-9 { return "stopped" }
        return (efolds < 0 ? "↓ " : "↑ ") + speed(exp(abs(efolds)))
    }

    static func ms(_ seconds: Double) -> String {
        let ms = seconds * 1000
        return ms < 10 ? String(format: "%.2f ms", ms) : String(format: "%.0f ms", ms)
    }
}

#if os(iOS)
/// A round 44-point button over the canvas (iPhone and iPad), in the material of the HUD.
/// Selected (the control center's while it is open) it looks like a `RoundToggle` that is on.
struct RoundButton: View {
    let title: String
    let symbol: String
    let isSelected: Bool
    let action: () -> Void

    init(_ title: String, symbol: String, isSelected: Bool = false, action: @escaping () -> Void) {
        self.title = title
        self.symbol = symbol
        self.isSelected = isSelected
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.title3)
                .foregroundStyle(isSelected ? Color.white : Color.primary)
                .frame(width: 44, height: 44)
                .background {
                    if isSelected {
                        Circle().fill(Color.accentColor)
                    } else {
                        Circle().fill(.ultraThinMaterial)
                    }
                }
                .overlay(Circle().strokeBorder(.white.opacity(isSelected ? 0.5 : 0), lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}

/// A round 44-point toggle (the Mac toolbar's Orbit, Mini-map and Size Scale): on, the circle
/// is filled with the accent colour and the symbol is white; off, the plain material.
struct RoundToggle: View {
    let title: String
    let symbol: String
    @Binding var isOn: Bool

    init(_ title: String, symbol: String, isOn: Binding<Bool>) {
        self.title = title
        self.symbol = symbol
        _isOn = isOn
    }

    var body: some View {
        Button {
            isOn.toggle()
        } label: {
            Image(systemName: symbol)
                .font(.title3)
                .foregroundStyle(isOn ? Color.white : Color.primary)
                .frame(width: 44, height: 44)
                .background {
                    if isOn {
                        Circle().fill(Color.accentColor)
                    } else {
                        Circle().fill(.ultraThinMaterial)
                    }
                }
                .overlay(Circle().strokeBorder(.white.opacity(isOn ? 0.5 : 0), lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityValue(isOn ? "on" : "off")
        .accessibilityAddTraits(isOn ? [.isToggle, .isSelected] : .isToggle)
    }
}

/// A journey stop's choices (kb: decisions/controls "Journey with stops"): next as planned (J),
/// deeper here (Z), somewhere else (X). Labelled capsules under the HUD; a column when a row
/// does not fit (phone portrait).
struct JourneyStopButtons: View {
    let model: ExplorerModel

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { buttons }
            VStack(alignment: .leading, spacing: 8) { buttons }
        }
    }

    @ViewBuilder private var buttons: some View {
        choice("Next", "forward.fill", "Journey: next landmark") { model.toggleJourney() }
        choice("Deeper here", "arrow.down.right.circle", "Journey: deeper here") { model.journeyDeeper() }
        choice("Elsewhere", "arrow.triangle.branch", "Journey: somewhere else") { model.journeyElsewhere() }
    }

    private func choice(_ title: String, _ symbol: String, _ label: String,
                        action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(title, systemImage: symbol)
                .font(.subheadline.weight(.semibold))
                .lineLimit(1)
                .fixedSize()
                .padding(.horizontal, 14)
                .frame(minHeight: 44)
                .background(.ultraThinMaterial, in: Capsule(style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}
#endif
