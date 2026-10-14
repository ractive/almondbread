import AlmondbreadCore
import SwiftUI

/// Settings side panel (a sheet on iPhone, where `.inspector` falls back to one).
struct InspectorView: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        Form {
            iterationsSection
            colourSection
            lightingSection
            qualitySection
            viewSection
            helpSection
        }
        .formStyle(.grouped)
        .controlSize(.small)
    }

    // MARK: Iterations

    private static let iterationCaps = [10_000, 50_000, 200_000, 1_000_000, 10_000_000]

    private var iterationsSection: some View {
        Section {
            Toggle("Automatic", isOn: $model.autoIterations)
            if model.autoIterations {
                LabeledContent("Current", value: (model.stats?.maxIterations ?? 0).formatted())
            } else {
                // Log scale: useful values span three orders of magnitude.
                Slider(value: logBinding(intBinding($model.manualIterations), range: 100...10_000_000), in: 0...1) {
                    Text("Maximum")
                }
                TextField("Maximum", value: $model.manualIterations, format: .number)
                    .multilineTextAlignment(.trailing)
                    .monospacedDigit()
            }
            Picker("Cap", selection: $model.maxIterationsCap) {
                ForEach(Self.iterationCaps, id: \.self) { Text($0.formatted()).tag($0) }
            }
            Toggle("Interior detection", isOn: $model.interiorDetection)
        } header: {
            Label("Iterations", systemImage: "repeat")
        } footer: {
            Text("The cap bounds automatic and manual values. High counts refine progressively. Interior detection stops points that settle into a cycle, so the black inside of the set stays cheap at any count.")
        }
    }

    // MARK: Colour

    private var colourSection: some View {
        Section {
            Picker("Palette", selection: $model.paletteID) {
                ForEach(ExplorerModel.palettes, id: \.id) { p in
                    HStack {
                        PaletteSwatch(palette: p).frame(width: 64, height: 12)
                        Text(p.name)
                    }
                    .tag(p.id)
                }
            }
            #if os(macOS)
            .pickerStyle(.menu)
            #else
            .pickerStyle(.navigationLink)
            #endif
            PaletteSwatch(palette: model.palette)
                .frame(height: 14)
            sliderRow("Density", value: logBinding($model.colorDensity, range: 0.001...1), in: 0...1,
                      text: String(format: "%.4g", model.colorDensity))
            sliderRow("Offset", value: $model.colorOffset, in: 0...1,
                      text: String(format: "%.2f", model.colorOffset))
            Toggle("Logarithmic", isOn: $model.logColoring)
            Toggle("Smooth", isOn: $model.smoothColoring)
            ColorPicker("Interior", selection: interiorColorBinding, supportsOpacity: false)
        } header: {
            Label("Colour", systemImage: "paintpalette")
        }
    }

    /// The model keeps the interior colour in linear RGB (what the kernel blends in).
    private var interiorColorBinding: Binding<Color> {
        Binding {
            let c = model.interiorColor
            return Color(.sRGBLinear, red: Double(c.x), green: Double(c.y), blue: Double(c.z))
        } set: { color in
            let r = color.resolve(in: EnvironmentValues())
            model.interiorColor = SIMD3(r.linearRed, r.linearGreen, r.linearBlue)
        }
    }

    // MARK: Lighting

    private var lightingSection: some View {
        Section {
            sliderRow("Shading", value: $model.shading, in: 0...1, text: String(format: "%.2f", model.shading))
            sliderRow("Light angle", value: $model.lightAngle, in: 0...360, text: String(format: "%.0f°", model.lightAngle))
                .disabled(model.shading == 0)
            sliderRow("Outline", value: $model.outline, in: 0...1, text: String(format: "%.2f", model.outline))
            sliderRow("Outline width", value: $model.outlineWidth, in: 0.5...6, text: String(format: "%.1f px", model.outlineWidth))
                .disabled(model.outline == 0)
        } header: {
            Label("Lighting", systemImage: "light.max")
        } footer: {
            Text("Both use the distance estimate, which adds a little GPU work per iteration.")
        }
    }

    // MARK: Quality

    private var qualitySection: some View {
        Section {
            Picker("Supersampling", selection: $model.supersampling) {
                ForEach(Supersampling.allCases) { Text($0.label).tag($0) }
            }
            Toggle("Interactive preview", isOn: $model.interactivePreview)
        } header: {
            Label("Quality", systemImage: "sparkles")
        } footer: {
            Text("A half- or quarter-resolution preview renders first when full frames are slow; the full image then refines over it.")
        }
    }

    // MARK: View

    private var viewSection: some View {
        Section {
            let v = model.stats?.shown
            coordinateRow("Re", v?.re ?? "")
            coordinateRow("Im", v?.im ?? "")
            LabeledContent("Zoom") {
                Text(Format.zoom(v?.zoom ?? 1)).monospacedDigit()
            }
            LabeledContent("Scale") {
                Text(String(format: "%.4g / pt", v?.scale ?? 0)).monospacedDigit()
            }
            HStack {
                Button {
                    Clipboard.copy(model.coordinatesText)
                } label: {
                    Label("Copy", systemImage: "doc.on.doc")
                }
                Spacer()
                Button {
                    model.reset()
                } label: {
                    Label("Reset", systemImage: "arrow.counterclockwise")
                }
            }
            #if os(iOS)
            .buttonStyle(.borderless)
            #endif
        } header: {
            Label("View", systemImage: "scope")
        }
    }

    private func coordinateRow(_ label: String, _ value: String) -> some View {
        LabeledContent(label) {
            Text(value)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
    }

    // MARK: Help

    private var helpSection: some View {
        Section {
            #if os(macOS)
            helpRow("hand.draw", "Two-finger scroll or drag", "Pan")
            helpRow("arrow.up.left.and.arrow.down.right", "Pinch, ⌘/⌥-scroll or wheel", "Zoom at cursor")
            helpRow("cursorarrow.click.2", "Double-click / ⌥ double-click", "Zoom in / out")
            helpRow("keyboard", "Hold W A S D / arrows", "Pan (diagonals too)")
            helpRow("space", "Hold Space / ⇧Space", "Zoom in at cursor / out")
            helpRow("gauge.with.dots.needle.33percent", "Q / E", "Zoom speed − / + (\(Format.speed(model.zoomFactorPerSecond)))")
            helpRow("plus.magnifyingglass", "+  −", "Zoom step")
            helpRow("arrow.counterclockwise", "R or ⌘0", "Reset view")
            helpRow("sidebar.trailing", "⌘I", "Toggle inspector")
            #else
            helpRow("hand.draw", "Drag", "Pan")
            helpRow("arrow.up.left.and.arrow.down.right", "Pinch", "Zoom and pan")
            helpRow("hand.tap", "Double tap", "Zoom in")
            helpRow("hand.tap", "Two-finger tap", "Zoom out")
            helpRow("keyboard", "Keyboard: W A S D / arrows", "Pan")
            helpRow("space", "Keyboard: Space / ⇧Space, Q / E", "Zoom, zoom speed")
            #endif
        } header: {
            Label("Controls", systemImage: "questionmark.circle")
        }
    }

    private func helpRow(_ symbol: String, _ input: String, _ action: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .frame(width: 20)
                .foregroundStyle(.secondary)
            Text(input)
            Spacer()
            Text(action).foregroundStyle(.secondary)
        }
        .font(.caption)
    }

    // MARK: Helpers

    private func sliderRow(_ title: String, value: Binding<Double>, in range: ClosedRange<Double>, text: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(title)
                Spacer()
                Text(text).monospacedDigit().foregroundStyle(.secondary)
            }
            Slider(value: value, in: range)
                .labelsHidden()
        }
    }

    private func intBinding(_ b: Binding<Int>) -> Binding<Double> {
        Binding { Double(b.wrappedValue) } set: { b.wrappedValue = Int($0.rounded()) }
    }

    /// Maps 0...1 onto `range` logarithmically.
    private func logBinding(_ b: Binding<Double>, range: ClosedRange<Double>) -> Binding<Double> {
        let lo = log(range.lowerBound), hi = log(range.upperBound)
        return Binding {
            (log(min(max(b.wrappedValue, range.lowerBound), range.upperBound)) - lo) / (hi - lo)
        } set: {
            b.wrappedValue = exp(lo + $0 * (hi - lo))
        }
    }
}

/// The palette's gradient as a small horizontal bar.
struct PaletteSwatch: View {
    let palette: PaletteInfo

    var body: some View {
        RoundedRectangle(cornerRadius: 3, style: .continuous)
            .fill(LinearGradient(gradient: palette.swatchGradient, startPoint: .leading, endPoint: .trailing))
            .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(.white.opacity(0.15)))
    }
}

extension PaletteInfo {
    /// One full cycle, closed back to the first stop like the GPU table.
    var swatchGradient: Gradient {
        func color(_ hex: UInt32) -> Color {
            Color(.sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
        }
        var stops = self.stops.map { Gradient.Stop(color: color($0.rgb), location: $0.position) }
        if let first = self.stops.first {
            stops.append(Gradient.Stop(color: color(first.rgb), location: 1))
        }
        return Gradient(stops: stops)
    }
}
