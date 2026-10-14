import AlmondbreadCore
import SwiftUI

struct ContentView: View {
    // One model per window; the platform view creates the Rust explorer (it compiles the
    // kernels) once, when SwiftUI makes the view.
    @State private var model = ExplorerModel()
    @State private var showInspector = UIDemo.enabled
    @Environment(\.horizontalSizeClass) private var sizeClass

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
            if model.showHUD {
                HUDView(model: model)
                    .padding(12)
            }
        }
        .onChange(of: model.showHUD) { if !model.showHUD { model.hudFrame = nil } }
        #if os(iOS)
        .overlay(alignment: .topTrailing) {
            Button {
                showInspector.toggle()
            } label: {
                Image(systemName: "slider.horizontal.3")
                    .font(.title3)
                    .frame(width: 44, height: 44)
                    .background(.ultraThinMaterial, in: Circle())
            }
            .buttonStyle(.plain)
            .padding(12)
            .accessibilityLabel("Control center")
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
                Text("zoom speed \(Format.speed(model.zoomFactorPerSecond))")
                    .padding(.horizontal, speedHighlight ? 5 : 0)
                    .background(speedHighlight ? Color.orange.opacity(0.35) : .clear, in: Capsule())
                    .foregroundStyle(speedHighlight ? .primary : .secondary)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
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
