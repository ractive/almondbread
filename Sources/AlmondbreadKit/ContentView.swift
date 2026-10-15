import AlmondbreadCore
import SwiftUI

struct ContentView: View {
    // One model per window; the platform view creates the Rust explorer (it compiles the
    // kernels) once, when SwiftUI makes the view.
    @State private var model = ExplorerModel()
    @State private var showInspector = false
    @State private var showFlyTo = false

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
        .background(Color.black)
        .overlay(alignment: .topLeading) {
            HUDView(model: model)
                .padding(12)
        }
        #if os(iOS)
        .overlay(alignment: .topTrailing) {
            HStack(spacing: 10) {
                // Autopilot: on, a finger steers, a tap pauses.
                Button {
                    model.toggleAutopilot()
                } label: {
                    Image(systemName: model.stats?.autopilot.active == true ? "airplane.circle.fill" : "airplane")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Autopilot")
                Button {
                    showInspector.toggle()
                } label: {
                    Image(systemName: "slider.horizontal.3")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Inspector")
            }
            .padding(12)
        }
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        #endif
        .inspector(isPresented: $showInspector) {
            InspectorView(model: model)
                .inspectorColumnWidth(min: 280, ideal: 320, max: 420)
                #if os(iOS)
                .presentationDetents([.medium, .large])
                #endif
        }
        .sheet(isPresented: $showFlyTo) {
            FlyToSheet(model: model, isPresented: $showFlyTo)
        }
        #if os(macOS)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    model.toggleAutopilot()
                } label: {
                    Label("Autopilot", systemImage: model.stats?.autopilot.active == true ? "airplane.circle.fill" : "airplane")
                }
                .help("Autopilot (Z): zoom on continuously, steer with the mouse; click pauses, Esc stops")
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showInspector.toggle()
                } label: {
                    Label("Inspector", systemImage: "sidebar.trailing")
                }
                .help("Show or hide the inspector (⌘I)")
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
                Text("GPU \(Format.ms(s.gpuTime))  ·  \(s.chunks) chunks  ·  \(s.renderWidth)×\(s.renderHeight)\(s.isRendering ? "  ·  refining" : "")")
                Text("ref \(s.referenceLength.formatted()) pts\(s.referencePeriod > 0 ? " (period \(s.referencePeriod))" : "")  ·  \(s.referenceBits) bit  ·  CPU \(Format.ms(s.referenceTime))\(s.referencePending ? "  ·  computing" : "")")
                Text(speedLine(s))
                    .padding(.horizontal, speedHighlight ? 5 : 0)
                    .background(speedHighlight ? Color.orange.opacity(0.35) : .clear, in: Capsule())
                    .foregroundStyle(speedHighlight ? .primary : .secondary)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
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
        if s.flight.active {
            return "fly speed \(Format.speed(exp(s.flight.speed)))"
        }
        if s.autopilot.active {
            return "autopilot \(Format.speed(exp(s.autopilot.speed)))  ·  now \(Format.speed(exp(max(0, s.autopilot.rate))))"
        }
        return "zoom speed \(Format.speed(model.zoomFactorPerSecond))"
    }

    /// Autopilot / flight state and why it is slower than set (kb: decisions/controls).
    func motionHint(_ s: ExplorerStats) -> String? {
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
