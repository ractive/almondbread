import AlmondbreadCore
import SwiftUI

struct ContentView: View {
    // One model per window; the platform view creates the Rust explorer (it compiles the
    // kernels) once, when SwiftUI makes the view.
    @State private var model = ExplorerModel()
    @State private var showInspector = UIDemo.enabled
    @State private var showFlyTo = false
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
                // Journey: planned flights from landmark to landmark; a tap on the canvas pauses.
                Button {
                    model.toggleJourney()
                } label: {
                    Image(systemName: model.journeyActive ? "sparkles.rectangle.stack.fill" : "sparkles")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Journey")
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
                Toggle(isOn: $model.cosmicScale) {
                    Label("Size Scale", systemImage: "ruler")
                }
                .help("Cosmic size scale: the view compared with real things, from the universe to the Planck length (C)")
                Button {
                    model.toggleAutopilot()
                } label: {
                    Label("Autopilot", systemImage: model.autopilotActive ? "airplane.circle.fill" : "airplane")
                }
                .help("Autopilot (Z): zoom on continuously, steer with the mouse; click pauses, Esc stops")
                Button {
                    model.toggleJourney()
                } label: {
                    Label("Journey", systemImage: model.journeyActive ? "sparkles.rectangle.stack.fill" : "sparkles")
                }
                .help("Journey (J): an endless planned flight from landmark to landmark; any input takes over, J resumes, Esc stops")
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
        #endif
        .focusedSceneValue(\.explorerModel, model)
        .focusedSceneValue(\.inspectorVisible, $showInspector)
        .focusedSceneValue(\.flyToVisible, $showFlyTo)
    }
}

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
                Text("iter \(s.maxIterations.formatted())\(model.autoIterations ? " auto" : "")")
                if !compact {
                    Text("GPU \(Format.ms(s.gpuTime))  ·  \(s.chunks) chunks  ·  \(s.renderWidth)×\(s.renderHeight)\(s.isRendering ? "  ·  refining" : "")")
                    Text("ref \(s.referenceLength.formatted()) pts\(s.referencePeriod > 0 ? " (period \(s.referencePeriod))" : "")  ·  \(s.referenceBits) bit  ·  CPU \(Format.ms(s.referenceTime))\(s.referencePending ? "  ·  computing" : "")")
                }
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
                Label(hint, systemImage: s.flight.active ? "location.north.line" : "airplane")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.primary)
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

extension HUDView {
    /// The speed Q / E change right now: the flight's, the autopilot's or Space's.
    func speedLine(_ s: ExplorerStats) -> String {
        if s.journey.active && !s.journey.suspended {
            return "journey speed \(Format.speed(exp(s.journey.speed)))"
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
        var parts = [name, "leg \(j.leg)"]
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
        if s.flight.active {
            let to = "Flying to \(Format.zoom(log10: s.flight.targetZoomLog10))  ·  \(Int(s.flight.progress * 100)) %"
            if s.flight.stopping { return to + "  ·  stopping" }
            return [to, Self.hintText(s.flight.hint)].compactMap { $0 }.joined(separator: "  ·  ")
        }
        guard s.autopilot.active else { return nil }
        if s.autopilot.paused { return "Autopilot paused  ·  click to resume" }
        return ["Autopilot", Self.hintText(s.autopilot.hint)].compactMap { $0 }.joined(separator: "  ·  ")
    }

    static func hintText(_ h: AutopilotHint) -> String? {
        switch h {
        case .none: nil
        case .rendering: "slower: keeping the detail sharp"
        case .reference: "waiting for the deep reference"
        case .interior: "slower: heading into the interior"
        case .featureless: "slower: featureless ahead"
        case .limit: "depth limit"
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

    static func ms(_ seconds: Double) -> String {
        let ms = seconds * 1000
        return ms < 10 ? String(format: "%.2f ms", ms) : String(format: "%.0f ms", ms)
    }
}
