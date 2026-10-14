import AlmondbreadCore
import SwiftUI

/// Settings side panel (a sheet on iPhone, where `.inspector` falls back to one).
struct InspectorView: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        Form {
            lookSection
            iterationsSection
            colourSection
            lightingSection
            qualitySection
            viewSection
            navigationSection
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

    // MARK: Look

    /// Curated looks: a palette with the colouring that suits it, applied in one step.
    private var lookSection: some View {
        Section {
            Picker("Look", selection: Binding(get: { model.lookID }, set: { model.applyLook($0) })) {
                ForEach(ExplorerModel.looks, id: \.id) { l in
                    HStack {
                        if let p = ExplorerModel.palettes.first(where: { $0.id == l.paletteId }) {
                            PaletteSwatch(palette: p).frame(width: 64, height: 12)
                        }
                        Text(l.name)
                    }
                    .tag(l.id)
                }
            }
            #if os(macOS)
            .pickerStyle(.menu)
            #else
            .pickerStyle(.navigationLink)
            #endif
            Toggle("Colour cycling", isOn: Binding(get: { model.cycleSpeed != 0 },
                                                   set: { model.cycleSpeed = $0 ? 0.05 : 0 }))
            if model.cycleSpeed != 0 {
                sliderRow("Cycle speed", value: logBinding($model.cycleSpeed, range: 0.005...0.5), in: 0...1,
                          text: String(format: "%.3g /s", model.cycleSpeed))
            }
            sliderRow("Depth tint", value: $model.depthTint, in: 0...0.2,
                      text: model.depthTint == 0 ? "off" : String(format: "%.3f / decade", model.depthTint))
        } header: {
            Label("Look", systemImage: "wand.and.stars")
        } footer: {
            Text("A look sets the palette, lighting, outline and interior together; adjust them below. Colours cycle only while the view rests. Depth tint shifts the palette as you zoom.")
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
            Toggle("Palette anti-aliasing", isOn: $model.paletteFilter)
            ColorPicker("Interior", selection: linearColorBinding(\.interiorColor), supportsOpacity: false)
        } header: {
            Label("Colour", systemImage: "paintpalette")
        }
    }

    /// The model keeps colours in linear RGB (what the kernel blends in).
    private func linearColorBinding(_ path: ReferenceWritableKeyPath<ExplorerModel, SIMD3<Float>>) -> Binding<Color> {
        Binding {
            let c = model[keyPath: path]
            return Color(.sRGBLinear, red: Double(c.x), green: Double(c.y), blue: Double(c.z))
        } set: { color in
            let r = color.resolve(in: EnvironmentValues())
            model[keyPath: path] = SIMD3(r.linearRed, r.linearGreen, r.linearBlue)
        }
    }

    // MARK: Lighting

    private var lightingSection: some View {
        Section {
            sliderRow("Shading", value: $model.shading, in: 0...1, text: String(format: "%.2f", model.shading))
            sliderRow("Light angle", value: $model.lightAngle, in: 0...360, text: String(format: "%.0f°", model.lightAngle))
                .disabled(model.shading == 0)
            sliderRow("Light height", value: $model.lightHeight, in: 0.2...4, text: String(format: "%.2f", model.lightHeight))
                .disabled(model.shading == 0)
            sliderRow("Highlight", value: $model.specular, in: 0...1, text: String(format: "%.2f", model.specular))
                .disabled(model.shading == 0)
            sliderRow("Outline", value: $model.outline, in: 0...1, text: String(format: "%.2f", model.outline))
            sliderRow("Outline width", value: $model.outlineWidth, in: 0.5...6, text: String(format: "%.1f px", model.outlineWidth))
                .disabled(model.outline == 0)
            ColorPicker("Outline colour", selection: linearColorBinding(\.edgeColor), supportsOpacity: false)
                .disabled(model.outline == 0)
            sliderRow("Glow", value: $model.glow, in: 0...0.5, text: String(format: "%.2f", model.glow))
            sliderRow("Glow width", value: $model.glowWidth, in: 1...40, text: String(format: "%.0f px", model.glowWidth))
                .disabled(model.glow == 0)
            ColorPicker("Glow colour", selection: linearColorBinding(\.glowColor), supportsOpacity: false)
                .disabled(model.glow == 0)
        } header: {
            Label("Lighting", systemImage: "light.max")
        } footer: {
            Text("Lighting, outline and glow use the distance estimate, which adds a little GPU work per iteration.")
        }
    }

    // MARK: Quality

    private var qualitySection: some View {
        Section {
            Picker("Anti-aliasing", selection: $model.supersampling) {
                ForEach(Supersampling.allCases) { Text($0.label).tag($0) }
            }
            Toggle("Interactive preview", isOn: $model.interactivePreview)
        } header: {
            Label("Quality", systemImage: "sparkles")
        } footer: {
            Text("While you move, each frame renders as much as fits in a refresh and builds on the frames before. When you stop, the view refines at full resolution and then gathers anti-aliasing samples near the boundary; any input interrupts that at once. Exports render the samples on a grid.")
        }
    }

    // MARK: View

    private var viewSection: some View {
        Section {
            let v = model.stats?.shown
            coordinateRow("Re", v?.re ?? "")
            coordinateRow("Im", v?.im ?? "")
            LabeledContent("Zoom") {
                Text(Format.zoom(log10: v?.zoomLog10 ?? 0)).monospacedDigit()
            }
            LabeledContent("Scale") {
                Text("\(v?.scaleText ?? "") / pt").monospacedDigit()
            }
            HStack {
                Button {
                    Clipboard.copy(model.coordinatesText)
                } label: {
                    Label("Copy", systemImage: "doc.on.doc")
                }
                Button {
                    model.pasteCoordinates()
                } label: {
                    Label("Go to copied", systemImage: "arrow.right.doc.on.clipboard")
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

    // MARK: Navigation

    private var navigationSection: some View {
        Section {
            Picker("Edge panning", selection: $model.edgePanning) {
                ForEach(EdgePanning.allCases) { Text($0.label).tag($0) }
            }
        } header: {
            Label("Navigation", systemImage: "move.3d")
        } footer: {
            Text("With the pointer near an edge of the image, the view also pans towards that edge, faster closer to it. "
                 + "\"While zooming\" pans only while Space is held, so reaching for the inspector or the Dock never moves the image.")
        }
    }

    // MARK: Help

    private var helpSection: some View {
        Section {
            #if os(macOS)
            helpRow("scope", "Move the mouse", "Pointer mode: zoom at the cursor")
            helpRow("keyboard", "Hold W A S D / arrows", "Pan; keyboard mode: zoom at the centre")
            helpRow("space", "Hold Space / ⇧Space", "Zoom in / out at the crosshair")
            helpRow("arrow.up.left.and.arrow.down.right", "Pinch, ⌘/⌥-scroll or wheel", "Zoom at the crosshair")
            helpRow("plus.magnifyingglass", "+  −", "Zoom step at the crosshair")
            helpRow("hand.draw", "Two-finger scroll or drag", "Pan")
            helpRow("cursorarrow.click.2", "Double-click / ⌥ double-click", "Zoom in / out there")
            helpRow("arrow.up.and.down.and.arrow.left.and.right", "Pointer at an edge", "Pan that way (while zooming)")
            helpRow("gauge.with.dots.needle.33percent", "Q / E", "Zoom speed − / + (\(Format.speed(model.zoomFactorPerSecond)))")
            helpRow("arrow.counterclockwise", "R or ⌘0", "Reset view")
            helpRow("sidebar.trailing", "⌘I", "Toggle inspector")
            #else
            helpRow("hand.draw", "Drag", "Pan")
            helpRow("arrow.up.left.and.arrow.down.right", "Pinch", "Zoom at the fingers, and pan")
            helpRow("hand.tap", "Double tap", "Zoom in")
            helpRow("hand.tap", "Two-finger tap", "Zoom out")
            helpRow("scope", "Trackpad or mouse pointer", "Zoom at the pointer")
            helpRow("keyboard", "Keyboard: W A S D / arrows", "Pan; zoom at the centre")
            helpRow("space", "Keyboard: Space / ⇧Space, Q / E", "Zoom, zoom speed")
            #endif
        } header: {
            Label("Controls", systemImage: "questionmark.circle")
        } footer: {
            #if os(macOS)
            Text("Every zoom goes towards the crosshair and keeps the point under it in place. Moving the mouse makes the cursor "
                 + "the crosshair; W A S D or the arrows hide the cursor and show a crosshair that glides to the centre. "
                 + "Q, E and Space alone do not switch.")
            #else
            Text("Every zoom keeps the point under the fingers, the pointer or the crosshair in place. W A S D or the arrows show "
                 + "a crosshair that glides to the centre; moving the pointer switches back.")
            #endif
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
