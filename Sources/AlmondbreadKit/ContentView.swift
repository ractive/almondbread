import AlmondbreadCore
import SwiftUI

struct ContentView: View {
    // One model per window; the platform view creates the Rust explorer (it compiles the
    // kernels) once, when SwiftUI makes the view.
    @State private var model = ExplorerModel()
    @State private var showInspector = UIDemo.enabled
    @State private var showFlyTo = false
    @State private var fullScreenSidebar = FullScreenSidebar()
    @Environment(\.horizontalSizeClass) private var sizeClass

    /// Width of the canvas in points (narrower than the window while the iPad sidebar shows).
    @State private var canvasWidth: CGFloat = 1000

    /// iPhone portrait, narrow iPad split views, and an iPad portrait canvas next to the
    /// control center: the HUD drops its statistics and the buttons move to the bottom.
    private var compactWidth: Bool {
        #if os(iOS)
        sizeClass == .compact || canvasWidth < 560
        #else
        false
        #endif
    }

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
        .ignoresSafeArea()
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { canvasWidth = $0 }
        .background(Color.black)
        .overlay(alignment: .topLeading) {
            if model.showHUD {
                HUDView(model: model, compact: compactWidth)
                    .padding(12)
            }
        }
        .onChange(of: model.showHUD) { if !model.showHUD { model.hudFrame = nil } }
        #if os(iOS)
        // iPhone portrait: the π panel sits left of the button column.
        .onChange(of: compactWidth, initial: true) {
            model.piMeterInsetRight = compactWidth ? 56 : 0
            // iPad and wide iPhone: the compass sits below the top-right buttons (44 + 12 pt).
            model.compassInsetTop = compactWidth ? 0 : 56
            // The cockpit (bottom centre) keeps left of the button column.
            model.cockpitInsetRight = compactWidth ? 56 : 0
        }
        #endif
        #if os(iOS)
        // iPhone portrait: the buttons sit at the bottom, in thumb reach and clear of the HUD;
        // wider screens: top trailing, next to the HUD.
        .overlay(alignment: compactWidth ? .bottomTrailing : .topTrailing) {
            let stack = compactWidth ? AnyLayout(VStackLayout(spacing: 10)) : AnyLayout(HStackLayout(spacing: 10))
            stack {
                // Autopilot: on, a finger steers, a tap pauses.
                Button {
                    model.toggleAutopilot()
                } label: {
                    Image(systemName: model.autopilotActive ? "airplane.circle.fill" : "airplane")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Autopilot")
                // A long press: the cruise pedals and lane assist (the keyboard's E, Q, Esc, L;
                // kb: decisions/controls "Driving levels").
                .contextMenu {
                    Button("Cruise In / Faster", systemImage: "plus.magnifyingglass") { model.pedalIn() }
                    Button("Cruise Out / Slower", systemImage: "minus.magnifyingglass") { model.pedalOut() }
                    Button("Stop", systemImage: "stop.circle") { model.stopMotion() }
                    Toggle("Lane Assist", systemImage: "road.lanes", isOn: $model.laneAssist)
                }
                // Journey: planned flights from landmark to landmark; a tap on the canvas pauses.
                // At a stop it flies on, and two more buttons go deeper here or elsewhere.
                Button {
                    model.toggleJourney()
                } label: {
                    Image(systemName: model.journeyAtStop ? "forward.fill"
                          : model.journeyActive ? "sparkles.rectangle.stack.fill" : "sparkles")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.journeyAtStop ? "Journey: next landmark" : "Journey")
                if model.journeyAtStop {
                    Button {
                        model.journeyDeeper()
                    } label: {
                        Image(systemName: "arrow.down.right.circle")
                            .font(.title3)
                            .frame(width: 44, height: 44)
                            .background(.ultraThinMaterial, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Journey: deeper here")
                    Button {
                        model.journeyElsewhere()
                    } label: {
                        Image(systemName: "arrow.triangle.branch")
                            .font(.title3)
                            .frame(width: 44, height: 44)
                            .background(.ultraThinMaterial, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Journey: somewhere else")
                }
                Button {
                    showInspector.toggle()
                } label: {
                    Image(systemName: "slider.horizontal.3")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Control center")
                .accessibilityValue(showInspector ? "shown" : "hidden")
            }
            .padding(12)
        }
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        #endif
        // A trailing sidebar on the Mac and iPad (the canvas narrows: nothing covers the image),
        // a sheet on iPhone.
        .inspector(isPresented: $showInspector) {
            ControlCenterView(model: model)
                .inspectorColumnWidth(min: 290, ideal: 330, max: 440)
                #if os(iOS)
                .presentationDetents(sizeClass == .compact ? [.medium, .large] : [.large])
                .presentationBackground(.regularMaterial)
                #endif
        }
        .sheet(isPresented: $showFlyTo) {
            FlyToSheet(model: model, isPresented: $showFlyTo)
        }
        #if os(iOS)
        .modifier(HelpSheet())
        #endif
        #if os(macOS)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Toggle(isOn: $model.orbitEnabled) {
                    Label("Orbit", systemImage: "point.topleft.down.to.point.bottomright.curvepath")
                }
                .help("Orbit of the point under the pointer (O)")
                Toggle(isOn: $model.mapEnabled) {
                    Label("Mini-map", systemImage: "map")
                }
                .help("Mini-map with nested levels; click it to jump there (M)")
                Toggle(isOn: $model.piMeterEnabled) {
                    Label("π Meter", systemImage: "pi")
                }
                .help("π meter (P): at the neck −3/4 + iε or the cusp 1/4 + ε the escape count times ε (√ε) closes in on π; View menu: guided flights")
                Toggle(isOn: $model.cosmicScale) {
                    Label("Size Scale", systemImage: "ruler")
                }
                .help("Cosmic size scale: how big the whole set would be if your window stayed its real size (C)")
                Button {
                    model.toggleAutopilot()
                } label: {
                    Label("Autopilot", systemImage: model.autopilotActive ? "airplane.circle.fill" : "airplane")
                }
                .help("Autopilot (Z): zoom on continuously towards interesting places; click pauses, Esc stops; steering hands over to cruise (E / Q)")
                Button {
                    model.toggleJourney()
                } label: {
                    Label("Journey", systemImage: model.journeyActive ? "sparkles.rectangle.stack.fill" : "sparkles")
                }
                .help("Journey (J): an endless planned flight from landmark to landmark; any input takes over, J resumes, Esc stops; ⇧J in the other mode (continuous / stop at each find)")
                Button {
                    showFlyTo = true
                } label: {
                    Label("Fly To", systemImage: "location.north.line")
                }
                .help("Fly to coordinates (⌘G); ⇧⌘G flies to copied ones")
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showInspector.toggle()
                } label: {
                    Label("Control Center", systemImage: "slider.horizontal.3")
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
            if let hint = motionHint(s) {
                Label(hint, systemImage: s.cruise.level == .cruise ? "gauge.with.dots.needle.33percent"
                      : s.flight.active ? "location.north.line" : "airplane")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.primary)
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
        .allowsHitTesting(false)
        // The canvas shows the arrow cursor over the HUD and treats it as off-canvas.
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { model.hudFrame = $0 }
        .accessibilityElement(children: .combine)
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
            Label(text, systemImage: "hourglass")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
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
        if j.suspended { return "\(name) paused  ·  J or click to resume" }
        // At a stop (kb: decisions/controls "Journey with stops"): the landmark and the choices.
        if j.atStop {
            let here = [j.title, Format.zoom(log10: s.shown.zoomLog10)].filter { !$0.isEmpty }.joined(separator: "  ·  ")
            if j.phase == "planning" { return here + "  ·  planning" }
            #if os(macOS)
            return here + "  —  J next  ·  Z deeper here  ·  X somewhere else"
            #else
            return here + "  —  next  ·  deeper  ·  elsewhere"
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
        if s.autopilot.paused { return "Autopilot paused  ·  click to resume" }
        let lock = s.autopilot.lockLabel.isEmpty ? nil : s.autopilot.lockLabel
        return ["Autopilot", lock, Self.hintText(s.autopilot.hint)].compactMap { $0 }.joined(separator: "  ·  ")
    }

    /// A coordinate cut to `n` characters (deep views have hundreds of digits; ⇧⌘C copies all).
    static func digits(_ x: String, _ n: Int) -> String {
        x.count <= n ? x : x.prefix(n - 1) + "…"
    }

    static func hintText(_ h: AutopilotHint) -> String? {
        switch h {
        case .none: nil
        case .rendering: "slower: keeping the detail sharp"
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
